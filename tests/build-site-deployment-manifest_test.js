"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { execFileSync, spawnSync } = require("node:child_process");
const { test, mock } = require("node:test");
const builder = require("../tools/build-site-deployment-manifest.js");
const root = path.resolve(__dirname, "..");
const tool = path.join(root, "tools/build-site-deployment-manifest.js");
const head = execFileSync("git", ["-c", `safe.directory=${root}`, "-C", root, "rev-parse", "HEAD"], { encoding: "utf8", timeout: 30000, windowsHide: true }).trim();
const sha = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
const blobId = (bytes) => crypto.createHash("sha1").update(`blob ${bytes.length}\0`).update(bytes).digest("hex");
const tree = (rows = builder.publicFiles.map((name) => `100644 blob ${"a".repeat(40)}\t${name}`)) => Buffer.from(rows.join("\0") + "\0");

test("accepts only an exact source commit and optional canonical verification", () => {
  assert.deepEqual(builder.parseArguments([head]), { contentCommit: head, verify: false });
  assert.deepEqual(builder.parseArguments([head, "--verify"]), { contentCommit: head, verify: true });
  for (const args of [[], ["HEAD"], [head + "\n"], [head, "E:\\unexpected.json"], [head, "--verify", "E:\\unexpected.json"], [head, "--output"], [head, "--verify", "--verify"]]) {
    assert.throws(() => builder.parseArguments(args), /Usage:/);
  }
});

test("requires every exact public file and places the current marker last", () => {
  const records = builder.parsePublicTree(tree());
  assert.deepEqual(records.map((row) => row.path), builder.publicFiles);
  assert.equal(records.length, 22);
  assert.equal(records.at(-1).path, ".well-known/site-release.json");
  assert.throws(() => builder.parsePublicTree(tree(builder.publicFiles.slice(1).map((name) => `100644 blob ${"a".repeat(40)}\t${name}`))), /missing/);
});

test("rejects unknown public files, old model payloads, case aliases, links and submodules", () => {
  const rows = builder.publicFiles.map((name) => `100644 blob ${"a".repeat(40)}\t${name}`);
  for (const name of ["docs/extra.md", "catalog/model.slm", "catalog/model.safetensors", "DOCS/hidden.txt", "unexpected.js", "docs/../escape.txt", "docs/.secrets"]) {
    assert.throws(() => builder.parsePublicTree(tree([...rows, `100644 blob ${"a".repeat(40)}\t${name}`])), /unreviewed public path/);
  }
  assert.throws(() => builder.parsePublicTree(tree([...rows, rows[0]])), /alias/);
  for (const prefix of ["120000 blob", "160000 commit"]) {
    assert.throws(() => builder.parsePublicTree(tree([`${prefix} ${"a".repeat(40)}\t.htaccess`, ...rows.slice(1)])), /non-regular/);
  }
});

test("rejects malformed, truncated and oversized Git tree data", () => {
  for (const bytes of [Buffer.alloc(0), tree().subarray(0, -1), Buffer.from("garbage\0"), Buffer.alloc(4 * 1024 * 1024 + 1), Buffer.from([255, 0])]) {
    assert.throws(() => builder.parsePublicTree(bytes));
  }
});

test("verifies payload hashes rather than trusting Git output labels", () => {
  for (const bytes of [Buffer.alloc(0), Buffer.from("exact payload")]) {
    builder.assertGitBlobIdentity({ objectId: blobId(bytes) }, bytes);
    assert.throws(() => builder.assertGitBlobIdentity({ objectId: "0".repeat(40) }, bytes), /object identity/);
  }
  const original = Buffer.from("abc");
  assert.throws(() => builder.assertGitBlobIdentity({ objectId: blobId(original) }, Buffer.from("abd")), /object identity/);
  assert.throws(() => builder.assertGitBlobIdentity({ objectId: "a".repeat(40) }, Buffer.alloc(16 * 1024 * 1024 + 1)), /byte bound/);
});

test("builds the entire current canonical closure without mutating source or its manifest", () => {
  const manifestPath = path.join(root, "deploy/site-deployment-manifest.v1.json");
  const before = fs.readFileSync(manifestPath);
  const identities = new Map(builder.publicFiles.map((name) => [name, sha(fs.readFileSync(path.join(root, name)))]));
  const manifest = builder.buildManifest(head);
  assert.equal(manifest.source_commit, head);
  assert.equal(manifest.files.length, 22);
  assert.equal(manifest.materializations.length, 2);
  assert.ok(manifest.files.reduce((sum, file) => sum + file.bytes, 0) < 64 * 1024 * 1024);
  for (const name of builder.publicFiles) assert.equal(sha(fs.readFileSync(path.join(root, name))), identities.get(name), name);
  assert.deepEqual(fs.readFileSync(manifestPath), before);
  assert.throws(() => builder.buildManifest("7e0a6eb6bf292ff69e3030adf1d229c811606ce9"), /Canonical public source differs/);
});

test("includes ignored public extras in its refusal without creating a filesystem fixture", () => {
  const docs = path.join(root, "docs");
  const extra = path.join(docs, "untracked-ignored.tmp");
  const readdir = fs.readdirSync.bind(fs);
  const lstat = fs.lstatSync.bind(fs);
  const directoryMock = mock.method(fs, "readdirSync", (target, options) => {
    const entries = readdir(target, options);
    return path.resolve(target) === docs ? [...entries, { name: "untracked-ignored.tmp", isDirectory: () => false, isFile: () => true }] : entries;
  });
  const statMock = mock.method(fs, "lstatSync", (target, ...args) => path.resolve(target) === extra ? { isSymbolicLink: () => false } : lstat(target, ...args));
  try { assert.throws(() => builder.buildManifest(head), /unreviewed file/); }
  finally { directoryMock.mock.restore(); statMock.mock.restore(); }
  assert.equal(fs.existsSync(extra), false);
});

test("current JS metadata materializes identically through the actual PowerShell custody module", () => {
  const manifest = Buffer.from(JSON.stringify(builder.buildManifest(head)));
  const command = [
    "$ErrorActionPreference='Stop'",
    "Import-Module './tools/MiniModel.DeploymentCustody.psm1' -Force",
    "$bytes=[Convert]::FromBase64String($env:MINIMODEL_TEST_MANIFEST)",
    "$manifest=[Text.Encoding]::UTF8.GetString($bytes) | ConvertFrom-Json",
    "$authority=Get-Content -LiteralPath './deploy/publication-authority.v1.json' -Raw | ConvertFrom-Json",
    "$hash=Get-CustodyByteSha256 -Bytes $bytes",
    "$inputRecord=New-CustodyPublicationInput -Phase public-admitted -RepositoryRoot $PWD -Manifest $manifest -ManifestSha256 $hash -OperationId ('1'*32) -Authority $authority",
    "Assert-CustodyPublicationInput -InputRecord $inputRecord -InputSha256 $inputRecord.BindingSha256 -ManifestSha256 $hash -OperationId ('1'*32)",
    "[pscustomobject]@{files=$inputRecord.Payloads.Count;input_sha256=$inputRecord.BindingSha256} | ConvertTo-Json -Compress",
  ].join("\n");
  const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", command], {
    cwd: root, encoding: "utf8", windowsHide: true, timeout: 30000, maxBuffer: 65536,
    env: { ...process.env, MINIMODEL_TEST_MANIFEST: manifest.toString("base64") },
  });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stdout + result.stderr);
  const observed = JSON.parse(result.stdout);
  assert.equal(observed.files, 22);
  assert.match(observed.input_sha256, /^[0-9a-f]{64}$/);
});

test("actual CLI refuses removed output-path arguments without creating files", () => {
  const absent = path.join(root, "artifacts/manifest-must-not-be-created.json");
  assert.equal(fs.existsSync(absent), false);
  const result = spawnSync(process.execPath, [tool, head, absent], { cwd: root, encoding: "utf8", windowsHide: true, timeout: 5000, maxBuffer: 65536 });
  assert.ifError(result.error);
  assert.notEqual(result.status, 0);
  assert.equal(result.stdout, "");
  assert.match(result.stderr, /Usage:/);
  assert.equal(fs.existsSync(absent), false);
});

test("actual CLI emits only the current verified manifest to stdout", () => {
  const before = fs.readFileSync(path.join(root, "deploy/site-deployment-manifest.v1.json"));
  const result = spawnSync(process.execPath, [tool, head], { cwd: root, encoding: "utf8", windowsHide: true, timeout: 90000, maxBuffer: 2 * 1024 * 1024 });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stderr, "");
  assert.deepEqual(JSON.parse(result.stdout), builder.buildManifest(head));
  assert.deepEqual(fs.readFileSync(path.join(root, "deploy/site-deployment-manifest.v1.json")), before);
});
