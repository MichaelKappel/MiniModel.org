#!/usr/bin/env node
"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");

const repositoryRoot = path.resolve(__dirname, "..");
const contentCommit = process.argv[2];
const verifyIndex = process.argv.indexOf("--verify");
const verifyPath = verifyIndex >= 0
  ? path.resolve(repositoryRoot, process.argv[verifyIndex + 1] || "deploy/site-deployment-manifest.v1.json")
  : null;
const outputArgument = process.argv[3] && process.argv[3] !== "--verify"
  ? process.argv[3]
  : "deploy/site-deployment-manifest.v1.json";
const outputPath = path.resolve(repositoryRoot, outputArgument);
const version = "1.1.2";
const releaseDate = "2026-08-31";
const releasePath = ".well-known/site-release.json";
const historyPath = ".well-known/site-release-history.json";
const retiredPaths = [
  "catalog/models.v0.json",
  "catalog/peer-source-announcements.v0.json",
  "catalog/qwen2.5-1.5b-instruct-q4.catalog-entry.v0.json",
  "catalog/qwen2.5-1.5b-instruct-q4.conversation-evidence.v1",
  "catalog/qwen2.5-1.5b-instruct-q4.minimodel.manifest.v0",
  "catalog/qwen2.5-1.5b-instruct-q4.minimodel-peer-offer",
  "catalog/qwen2.5-1.5b-instruct-q4.modification-notice.md",
  "catalog/qwen2.5-1.5b-instruct-q4.quality-evaluation.v1",
  "catalog/qwen2.5-1.5b-instruct-q4.source-license.txt",
  "docs/manifest-v0.md",
  "docs/verifier-mvp.md",
  "examples/catalog-entry.v0.json",
  "examples/minimodel.manifest.v0.example",
  "examples/share-metadata.v0.json",
  "examples/tiny-test-model-q8.minimodel.manifest.v0",
  "examples/tiny-test-model-q8.minimodel-peer-import-request.json",
  "examples/tiny-test-model-q8.minimodel-peer-offer",
];

if (!/^[0-9a-f]{40}$/.test(contentCommit || "")) {
  throw new Error("Usage: node tools/build-site-deployment-manifest.js <40-character-content-commit> [output]");
}

function git(args, encoding = "utf8") {
  return execFileSync("git", ["--no-optional-locks", ...args], {
    cwd: repositoryRoot,
    encoding,
    env: { ...process.env, GIT_OPTIONAL_LOCKS: "0", GIT_TERMINAL_PROMPT: "0" },
    maxBuffer: 128 * 1024 * 1024,
  });
}

function isPublicPath(relativePath) {
  const rootFiles = new Set([
    ".htaccess",
    "LICENSE.txt",
    "ROADMAP.md",
    "catalog.js",
    "index.html",
    "llms-full.txt",
    "llms.txt",
    "robots.txt",
    "sitemap.xml",
    "styles.css",
  ]);
  if (relativePath.endsWith(".slm")) return false;
  return rootFiles.has(relativePath) ||
    relativePath.startsWith(".well-known/") ||
    relativePath.startsWith("catalog/") ||
    relativePath.startsWith("docs/") ||
    relativePath.startsWith("examples/") ||
    relativePath.startsWith("updates/");
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function parseJsonBlob(bytes, label) {
  if (bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    throw new Error(`${label} must not contain a UTF-8 byte-order mark`);
  }
  const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(`${label} is not strict UTF-8 JSON`);
  }
}

function compactJsonBytes(value) {
  return Buffer.from(`${JSON.stringify(value)}\n`, "utf8");
}

function materializePublishedArtifact(relativePath, sourceBytes) {
  if (relativePath === releasePath) {
    const release = parseJsonBlob(sourceBytes, "prepared release marker");
    if (release.schema !== "uaix.site_release.v1" ||
        release.release_id !== "minimodel.org@1.1.2" ||
        release.version !== version ||
        release.release_date !== releaseDate ||
        release.last_publish_date !== "2026-08-29" ||
        release.status !== "prepared") {
      throw new Error("Prepared release marker does not preserve the truthful pre-publication state");
    }
    release.last_publish_date = releaseDate;
    release.status = "published";
    return {
      kind: "prepared-release-to-published-v1",
      bytes: compactJsonBytes(release),
    };
  }

  if (relativePath === historyPath) {
    const history = parseJsonBlob(sourceBytes, "prepared release history");
    const releases = Array.isArray(history.releases) ? history.releases : [];
    if (history.schema !== "uaix.site_release_history.v1" ||
        history.current_release_id !== "minimodel.org@1.1.2" ||
        releases.length !== 4 ||
        releases[0]?.release_id !== "minimodel.org@1.1.2" ||
        releases[0]?.version !== version ||
        releases[0]?.release_date !== releaseDate ||
        releases[0]?.last_publish_date !== null ||
        releases[0]?.status !== "prepared" ||
        releases[1]?.release_id !== "minimodel.org@1.1.1" ||
        releases[1]?.last_publish_date !== "2026-08-29") {
      throw new Error("Prepared release history does not preserve the truthful pre-publication ledger");
    }
    releases[0].last_publish_date = releaseDate;
    releases[0].status = "published";
    return {
      kind: "prepared-history-to-published-v1",
      bytes: compactJsonBytes(history),
    };
  }

  return null;
}

git(["cat-file", "-e", `${contentCommit}^{commit}`]);
const sourceTree = git(["rev-parse", `${contentCommit}^{tree}`]).trim();
const tracked = git(["ls-tree", "-r", "--name-only", contentCommit])
  .split(/\r?\n/)
  .filter(Boolean)
  .filter(isPublicPath)
  .sort((left, right) => left.localeCompare(right));
const marker = releasePath;
const ordered = tracked.filter((item) => item !== marker);
if (tracked.includes(marker)) ordered.push(marker);

const materializations = [];
const files = ordered.map((relativePath) => {
  const committed = git(["show", `${contentCommit}:${relativePath}`], null);
  const materialized = materializePublishedArtifact(relativePath, committed);
  const publicationBytes = materialized ? materialized.bytes : committed;
  if (materialized) {
    materializations.push({
      path: relativePath,
      kind: materialized.kind,
      source_bytes: committed.length,
      source_sha256: sha256(committed),
    });
  }
  return {
    path: relativePath,
    bytes: publicationBytes.length,
    sha256: sha256(publicationBytes),
    https_readback_path: relativePath.endsWith(".htaccess") ? null : `/${relativePath}`,
  };
});

if (files.length < 20 || files.length > 40 || files.at(-1)?.path !== marker) {
  throw new Error(`Unexpected public deployment closure: ${files.length} files`);
}
if (files.some((file) => file.path.endsWith(".slm"))) {
  throw new Error("Model bytes entered the public deployment closure");
}
if (materializations.length !== 2 ||
    materializations[0].path !== historyPath ||
    materializations[1].path !== releasePath) {
  throw new Error("The two governed publication records were not materialized exactly");
}

const manifest = {
  schema: "uaix.static_site_deployment_manifest.v1",
  site: "minimodel.org",
  origin: "https://minimodel.org",
  version,
  release_date: "2026-08-31",
  source_commit: contentCommit,
  source_tree: sourceTree,
  transport_profile: "MiniModel.org",
  materializations,
  files,
  retired_paths: retiredPaths,
  route_smokes: [
    { path: "/", contains: "Version 1.1.2" },
    { path: "/updates/", contains: "1.1.2 - 2026-08-31" },
    { path: "/catalog/compositions.v1.json", contains: "minimodel.slm2_catalog.v1" },
    { path: "/docs/catalog-boundary.md", contains: "Public peer URLs require HTTPS and HTTP is loopback-only." },
    { path: "/docs/memory-endpoints.md", contains: "exactly two" },
    { path: "/.well-known/site-release.json", contains: "minimodel.org@1.1.2" },
  ],
};

const manifestBytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`, "utf8");
if (verifyPath) {
  const actual = fs.readFileSync(verifyPath);
  if (!actual.equals(manifestBytes)) {
    throw new Error("Deployment manifest is stale or was not generated byte-for-byte from the bound content commit");
  }
  process.stdout.write(`VERIFIED ${verifyPath}\n${files.length} files\n`);
} else {
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, manifestBytes);
  process.stdout.write(`${outputPath}\n${files.length} files\n`);
}
