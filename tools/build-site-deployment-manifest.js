#!/usr/bin/env node
"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");

const repositoryRoot = path.resolve(__dirname, "..");
const canonicalRoot = "E:\\Source\\Rust\\MiniModel.org";
const manifestPath = path.join(repositoryRoot, "deploy/site-deployment-manifest.v1.json");
const maxFileBytes = 16 * 1024 * 1024;
const maxTotalBytes = 64 * 1024 * 1024;
const gateSha256 = "f99083edfe7eae96279acafcb624eb88e1a26e43d5622d498711fe280691647d";
const publicFiles = Object.freeze([
  ".htaccess", ".well-known/site-release-history.json", ".well-known/uaix-talisman.txt",
  "catalog.js", "catalog/.htaccess", "catalog/compositions.v1.json", "catalog/peer-announcements.v1.json",
  "docs/acquisition-contract.md", "docs/catalog-boundary.md", "docs/memory-endpoints.md",
  "docs/peer-companion.md", "docs/slm2-composition.md", "index.html", "LICENSE.txt", "llms-full.txt",
  "llms.txt", "ROADMAP.md", "robots.txt", "sitemap.xml", "styles.css", "updates/index.html",
  ".well-known/site-release.json",
]);
const publicSet = new Set(publicFiles);
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

/** Strict CLI: metadata goes to stdout; verification reads the canonical manifest only. */
function parseArguments(args) {
  if (args.length < 1 || args.length > 2 || typeof args[0] !== "string" || args[0].length !== 40 || !/^[0-9a-f]{40}$/.test(args[0]) ||
      (args.length === 2 && args[1] !== "--verify")) {
    throw new Error("Usage: node tools/build-site-deployment-manifest.js <40-character-content-commit> [--verify]");
  }
  return { contentCommit: args[0], verify: args.length === 2 };
}

/** Reject reparse ancestors before reading source or loading the canonical shared gate. */
function assertPlainAncestors(target) {
  let current = target;
  for (;;) {
    if (fs.lstatSync(current).isSymbolicLink()) throw new Error("Publication path contains a reparse point");
    const parent = path.dirname(current);
    if (parent === current) return;
    current = parent;
  }
}

/** Bound allocation and actual bytes independently; never read a growing file without a limit. */
function readBoundedFile(target, maximumBytes) {
  assertPlainAncestors(target);
  const fd = fs.openSync(target, "r");
  try {
    const info = fs.fstatSync(fd);
    if (!info.isFile() || info.size > maximumBytes) throw new Error("Publication file exceeds its regular-file byte bound");
    const bytes = Buffer.alloc(info.size);
    let offset = 0;
    while (offset < bytes.length) {
      const count = fs.readSync(fd, bytes, offset, bytes.length - offset, null);
      if (count === 0) throw new Error("Publication file changed during its bounded read");
      offset += count;
    }
    if (fs.readSync(fd, Buffer.alloc(1), 0, 1, null) !== 0) throw new Error("Publication file grew beyond its byte bound");
    return bytes;
  } finally { fs.closeSync(fd); }
}

/** Run read-only Git commands with a fixed root, output budget and deadline. */
function git(args, maximumBytes = 4 * 1024 * 1024, encoding = "utf8") {
  try {
    return execFileSync("git", ["--no-optional-locks", "-c", `safe.directory=${repositoryRoot}`, "-C", repositoryRoot, ...args], {
      cwd: repositoryRoot, encoding, windowsHide: true, timeout: 30000, maxBuffer: maximumBytes,
      env: { ...process.env, GIT_OPTIONAL_LOCKS: "0", GIT_TERMINAL_PROMPT: "0" },
    });
  } catch { throw new Error("Bounded publication Git command failed"); }
}

/** The builder can read public source only from the one direct canonical repository. */
function assertCanonicalRepository() {
  if (process.platform !== "win32" || repositoryRoot.toLowerCase() !== canonicalRoot.toLowerCase()) {
    throw new Error("Publication metadata requires the canonical MiniModel repository");
  }
  assertPlainAncestors(repositoryRoot);
  if (path.resolve(git(["rev-parse", "--show-toplevel"], 4096).trim()).toLowerCase() !== repositoryRoot.toLowerCase()) {
    throw new Error("Publication repository top-level differs from its canonical path");
  }
  const gate = path.join(path.dirname(repositoryRoot), "TinyRustLM.com/tinyrustlm/tools/verify-single-source-authority.js");
  if (sha256(readBoundedFile(gate, 141430)) !== gateSha256) throw new Error("Reviewed ecosystem source gate identity drifted");
  const verdict = require(gate).verifySingleSourceAuthority();
  if (verdict.verdict !== "pass" || verdict.canonicalLayoutAvailable !== true || verdict.localScanPerformed !== true ||
      verdict.coverageClass !== "registered-governed-roots-only") throw new Error("Ecosystem source authority was not proved");
}

/** Public directories are explicit release boundaries, never catch-all source inclusion rules. */
function isPublicCandidate(name) {
  return publicSet.has(name) || /^(?:\.well-known|catalog|docs|examples|updates)\//i.test(name) ||
    (!name.includes("/") && /\.(?:html?|css|js|json|xml)$/i.test(name));
}

/** Parse NUL-delimited tree identities and reject unknown public files, links and path aliases. */
function parsePublicTree(bytes) {
  if (!Buffer.isBuffer(bytes) || bytes.length > 4 * 1024 * 1024 || !bytes.length || bytes.at(-1) !== 0) {
    throw new Error("Publication Git tree is not bounded NUL-delimited data");
  }
  const rows = new TextDecoder("utf-8", { fatal: true }).decode(bytes).slice(0, -1).split("\0");
  if (rows.length > 32768) throw new Error("Publication Git tree exceeds its entry budget");
  const found = new Map();
  const seen = new Set();
  for (const row of rows) {
    const match = /^([0-7]{6}) (blob|commit) ([0-9a-f]{40})\t(.+)$/.exec(row);
    if (!match) throw new Error("Publication Git tree entry is malformed");
    const [, mode, type, objectId, name] = match;
    if (!isPublicCandidate(name)) continue;
    if (!publicSet.has(name) || seen.has(name.toLowerCase()) || !["100644", "100755"].includes(mode) || type !== "blob") {
      throw new Error("Publication tree contains an unreviewed public path, alias or non-regular entry");
    }
    seen.add(name.toLowerCase());
    found.set(name, { path: name, objectId });
  }
  if (found.size !== publicFiles.length) throw new Error("Publication tree is missing a reviewed public file");
  return publicFiles.map((name) => found.get(name));
}

/** Authenticate payload bytes independently of the Git command's successful exit status. */
function assertGitBlobIdentity(record, bytes) {
  if (!Buffer.isBuffer(bytes) || bytes.length > maxFileBytes || !/^[0-9a-f]{40}$/.test(record.objectId)) {
    throw new Error("Publication Git blob exceeds its identity or byte bound");
  }
  const identity = crypto.createHash("sha1").update(`blob ${bytes.length}\0`).update(bytes).digest("hex");
  if (identity !== record.objectId) throw new Error("Publication Git blob content does not match its object identity");
}

/** Include ignored and untracked entries when checking the actual public directory closure. */
function assertWorkingPublicClosure() {
  const found = new Set();
  let count = 0;
  function visit(directory, prefix = "", depth = 0) {
    if (depth > 16) throw new Error("Public directory depth exceeds its bound");
    for (const item of fs.readdirSync(directory, { withFileTypes: true })) {
      if (++count > 8192) throw new Error("Public directory entry budget exceeded");
      const name = `${prefix}${item.name}`;
      if (!isPublicCandidate(name) && !/^(?:\.well-known|catalog|docs|examples|updates)$/i.test(name)) continue;
      const full = path.join(directory, item.name);
      if (fs.lstatSync(full).isSymbolicLink()) throw new Error("Public source contains a reparse point");
      if (item.isDirectory()) { visit(full, `${name}/`, depth + 1); continue; }
      if (!item.isFile() || !publicSet.has(name) || found.has(name)) throw new Error("Canonical public source contains an unreviewed file");
      found.add(name);
    }
  }
  visit(repositoryRoot);
  if (found.size !== publicFiles.length) throw new Error("Canonical public source closure is incomplete");
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

/** Build current publication metadata from verified Git blobs and matching canonical files. */
function buildManifest(contentCommit) {
  if (typeof contentCommit !== "string" || contentCommit.length !== 40 || !/^[0-9a-f]{40}$/.test(contentCommit)) throw new Error("Publication source commit must be an exact identity");
  if (git(["cat-file", "-t", contentCommit], 4096).trim() !== "commit") throw new Error("Publication source identity is not a commit");
  git(["merge-base", "--is-ancestor", contentCommit, "HEAD"], 4096);
  const sourceTree = git(["rev-parse", `${contentCommit}^{tree}`], 4096).trim();
  if (!/^[0-9a-f]{40}$/.test(sourceTree)) throw new Error("Publication source tree identity is invalid");
  const records = parsePublicTree(git(["ls-tree", "-rz", "--full-tree", contentCommit], 4 * 1024 * 1024, null));
  assertWorkingPublicClosure();
  let totalBytes = 0;
  const materializations = [];
  const files = records.map((record) => {
    const relativePath = record.path;
    const committed = git(["cat-file", "blob", record.objectId], maxFileBytes, null);
    assertGitBlobIdentity(record, committed);
    const working = readBoundedFile(path.join(repositoryRoot, relativePath), maxFileBytes);
    if (!working.equals(committed)) throw new Error(`Canonical public source differs from the content commit: ${relativePath}`);
    const materialized = materializePublishedArtifact(relativePath, committed);
    const publicationBytes = materialized ? materialized.bytes : committed;
    totalBytes += publicationBytes.length;
    if (publicationBytes.length > maxFileBytes || totalBytes > maxTotalBytes) throw new Error("Publication payload exceeds its byte budget");
    if (materialized) materializations.push({ path: relativePath, kind: materialized.kind, source_bytes: committed.length, source_sha256: sha256(committed) });
    return { path: relativePath, bytes: publicationBytes.length, sha256: sha256(publicationBytes), https_readback_path: relativePath.endsWith(".htaccess") ? null : `/${relativePath}` };
  });
  if (materializations.length !== 2 || materializations[0].path !== historyPath || materializations[1].path !== releasePath) {
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

  return manifest;
}

/** Validate all inputs before emitting bounded metadata or a read-only verification result. */
function main(args = process.argv.slice(2)) {
  const options = parseArguments(args);
  assertCanonicalRepository();
  const manifest = buildManifest(options.contentCommit);
  const bytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  if (bytes.length > 2 * 1024 * 1024) throw new Error("Publication manifest exceeds its byte budget");
  if (options.verify) {
    if (!readBoundedFile(manifestPath, 2 * 1024 * 1024).equals(bytes)) throw new Error("Canonical deployment manifest is stale or differs from current verified source");
    process.stdout.write(`VERIFIED ${manifestPath}\n${manifest.files.length} files\n`);
  } else {
    process.stdout.write(bytes);
  }
}

module.exports = Object.freeze({ publicFiles, parseArguments, parsePublicTree, assertGitBlobIdentity, materializePublishedArtifact, buildManifest });
if (require.main === module) {
  try { main(); } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
