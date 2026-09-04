"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");

const repositoryRoot = path.resolve(__dirname, "..");
const harnessPath = path.join(__dirname, "deployment-custody.harness.ps1");

function read(relativePath) {
  return fs.readFileSync(path.join(repositoryRoot, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

function sha256(buffer) {
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

function runHarness(name) {
  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", harnessPath, name],
    {
      cwd: repositoryRoot,
      encoding: "utf8",
      maxBuffer: 8 * 1024 * 1024,
      timeout: 60_000,
      windowsHide: true,
    },
  );
  assert.equal(
    result.status,
    0,
    `custody harness ${name} failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  assert.match(result.stdout, new RegExp(`PASS ${name}`));
}

for (const [name, description] of [
  ["repository-identity", "rejects a wrong checkout, mirror, branch state, or remote head"],
  ["transport-authority", "rejects an unbound or wrong FTPS host, port, user, profile, or root"],
  ["mutation-order", "forbids local and public mutation before read-only admission"],
  ["public-predecessor", "rejects stale 1.1.1 public bytes and rewritten ledger history"],
  ["remote-overlay", "rejects missing and overlay entries in the publication root"],
  ["remote-observation", "treats FTP 550 and contradictory SIZE/LIST evidence as ambiguous failure"],
  ["rollback-order", "bounds the transaction journal and rolls paths back in exact reverse order"],
  ["lease-ownership", "binds deployment lease cleanup to one exact operation owner"],
  ["published-materialization", "materializes published records without rewriting prepared source facts"],
  ["sealed-snapshot", "keeps uploads bound after mutable source changes and cleans the snapshot"],
]) {
  test(description, () => runHarness(name));
}

test("binds the immutable 1.1.1 receipt without editing it", () => {
  const receipt = fs.readFileSync(
    path.join(repositoryRoot, "deploy/receipts/site-deployment-20260831T061709Z.json"),
  );
  assert.equal(sha256(receipt), "e38bae2fa85fbdfb9688994cc200e2718e0a9a14751974731b59a9387c315014");

  const authority = readJson("deploy/publication-authority.v1.json");
  assert.equal(authority.predecessor.version, "1.1.1");
  assert.equal(authority.predecessor.receipt_sha256, sha256(receipt));
  assert.deepEqual(authority.predecessor.ledger_release_ids, [
    "minimodel.org@1.1.1",
    "minimodel.org@1.1.0",
    "minimodel.org@1.0.0",
  ]);
});

test("keeps production transport blocked until an authorized secret-free tuple is recorded", () => {
  const authority = readJson("deploy/publication-authority.v1.json");
  assert.equal(authority.repository.root_policy, "script-parent-equals-git-toplevel");
  assert.equal(Object.hasOwn(authority.repository, "canonical_root"), false);
  assert.equal(authority.transport.status, "unbound");
  assert.equal(authority.transport.host, null);
  assert.equal(authority.transport.port, null);
  assert.equal(authority.transport.username_sha256, null);
  assert.equal(authority.transport.remote_root, null);
  assert.equal(authority.transport.profile_name, "MiniModel.org");
  assert.equal(authority.transport.protocol, "ftps-explicit");
  assert.equal(authority.transport.lease_directory_path, ".minimodel-deployment-lease.v1");

  const sentinel = fs.readFileSync(path.join(repositoryRoot, authority.transport.root_sentinel_source_path));
  assert.equal(sentinel.length, authority.transport.root_sentinel_bytes);
  assert.equal(sha256(sentinel), authority.transport.root_sentinel_sha256);
});

test("keeps workstation paths out of public-safe authority and deployment docs", () => {
  const publicSafe = [
    read("deploy/publication-authority.v1.json"),
    read("deploy/publication-root-sentinel.v1.json"),
    read("deploy/README.md"),
  ].join("\n");
  assert.doesNotMatch(publicSafe, /[A-Z]:\\Source\\Rust\\MiniModel\.org/i);
});

test("performs exact public admission before snapshot creation or credential access", () => {
  const publisher = read("tools/deploy-site.ps1");
  const publicAdmission = publisher.indexOf("Assert-CustodyPublicPredecessor -Authority $authority");
  const phaseAdvance = publisher.indexOf('$script:phase = "public-admitted"');
  const transportBinding = publisher.indexOf('if ($authority.transport.status -cne "bound")', phaseAdvance);
  const snapshot = publisher.indexOf("$snapshot = New-CustodySealedSnapshot", transportBinding);
  const credentialAccess = publisher.indexOf("$profile = Get-CredentialProfile", snapshot);

  assert.ok(publicAdmission >= 0);
  assert.ok(publicAdmission < phaseAdvance);
  assert.ok(phaseAdvance < transportBinding);
  assert.ok(transportBinding < snapshot);
  assert.ok(snapshot < credentialAccess);
  assert.match(publisher, /New-FtpRequest[\s\S]*Assert-CustodyMutationAllowed -Phase \$script:phase -MutationKind remote/);
});

test("bounds HTTP content lengths without assuming a nullable Value wrapper", () => {
  const publisher = read("tools/deploy-site.ps1");
  assert.doesNotMatch(publisher, /ContentLength\.Value/);
  assert.equal(
    (publisher.match(/\$contentLength = \$response\.Content\.Headers\.ContentLength/g) ?? []).length,
    2,
  );
  assert.equal(
    (publisher.match(/\$null -ne \$contentLength -and \[int64\]\$contentLength -gt/g) ?? []).length,
    2,
  );
});

test("runs the fixed-root source-authority gate before every transport-capable action", () => {
  const publisher = read("tools/deploy-site.ps1");
  const gateCall = publisher.search(/Assert-EcosystemSingleSourceAuthority\r?\n/);
  const moduleImport = publisher.indexOf("Import-Module -Name $modulePath -Force");
  const remoteHead = publisher.indexOf("$repositoryObservation = Get-RepositoryObservation");
  const publicRead = publisher.indexOf("$publicReleaseBytes = Get-PublicBytes");
  const snapshot = publisher.indexOf("$snapshot = New-CustodySealedSnapshot");
  const credential = publisher.indexOf("$profile = Get-CredentialProfile");
  const ftpUse = publisher.indexOf("$script:lease = New-RemoteDeploymentLease");

  assert.ok(gateCall >= 0);
  assert.ok(moduleImport > gateCall, "editable custody module import preceded source-authority gate");
  for (const later of [remoteHead, publicRead, snapshot, credential, ftpUse]) {
    assert.ok(later > gateCall, `transport-capable action at ${later} preceded source-authority gate`);
  }
  assert.match(
    publisher,
    /\$gateArguments = @\(\$authorityGate\)\r?\n\s*if \(\$Publish\) \{\r?\n\s*\$gateArguments \+= @\("--require-clean", "--require-remote-exact"\)\r?\n\s*\}\r?\n\s*\$gateOutput = @\(& node @gateArguments 2>&1\)/,
  );
  assert.doesNotMatch(publisher, /& node \$authorityGate/);
  assert.match(publisher, /TinyRustLM\.com\\tinyrustlm/);
  assert.match(publisher, /New-FtpRequest[\s\S]*if \(-not \$script:sourceAuthorityAdmitted\)/);
  assert.doesNotMatch(publisher, /E:\\Source\\Rust\\MiniModel\.org/);
});

test("fails a hostile source-authority gate before profile, snapshot, receipt, or FTP effects", () => {
  const fixtureRoot = fs.mkdtempSync(path.join(require("node:os").tmpdir(), "minimodel-gate-order-"));
  const gateMarker = path.join(fixtureRoot, "gate-invoked.txt");
  const gateArguments = path.join(fixtureRoot, "gate-arguments.txt");
  const fakeNode = path.join(fixtureRoot, "node.cmd");
  const reportPath = path.join(
    repositoryRoot,
    "deploy",
    "receipts",
    "site-deployment-20991231T235959Z.json",
  );
  try {
    fs.writeFileSync(
      fakeNode,
      "@echo off\r\necho invoked>\"%MINIMODEL_GATE_MARKER%\"\r\necho %*>\"%MINIMODEL_GATE_ARGUMENTS%\"\r\nexit /b 23\r\n",
      "ascii",
    );
    assert.equal(fs.existsSync(reportPath), false, "hostile test receipt path must begin absent");
    const result = spawnSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        path.join(repositoryRoot, "tools", "deploy-site.ps1"),
        "-Publish",
        "-FileZillaProfilePath",
        path.join(fixtureRoot, "must-not-be-read.xml"),
        "-ReportPath",
        reportPath,
      ],
      {
        cwd: repositoryRoot,
        encoding: "utf8",
        timeout: 30_000,
        windowsHide: true,
        env: {
          ...process.env,
          PATH: `${fixtureRoot};${process.env.PATH}`,
          MINIMODEL_GATE_ARGUMENTS: gateArguments,
          MINIMODEL_GATE_MARKER: gateMarker,
        },
      },
    );
    assert.notEqual(result.status, 0);
    assert.match(`${result.stdout}\n${result.stderr}`, /source-authority gate failed closed/i);
    assert.equal(fs.readFileSync(gateMarker, "utf8").trim(), "invoked");
    assert.match(
      fs.readFileSync(gateArguments, "utf8").trim(),
      /verify-single-source-authority\.js --require-clean --require-remote-exact$/i,
    );
    assert.equal(fs.existsSync(reportPath), false);
    assert.equal(fs.existsSync(path.join(fixtureRoot, "must-not-be-read.xml")), false);
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("holds an exact lease across predecessor proof and keeps backups through final gates", () => {
  const publisher = read("tools/deploy-site.ps1");
  const lease = publisher.indexOf("$script:lease = New-RemoteDeploymentLease");
  const predecessor = publisher.indexOf("$predecessorTree = Get-ObservedRemoteTree", lease);
  const marker = publisher.indexOf("$markerResult = Publish-File", predecessor);
  const finalGates = publisher.indexOf("$finalGatesPassed = $true", marker);
  const cleanup = publisher.indexOf("Complete-RemoteTransaction", finalGates);
  const release = publisher.indexOf("Remove-RemoteDeploymentLease", cleanup);
  const publishFunction = publisher.slice(
    publisher.indexOf("function Publish-File"),
    publisher.indexOf("function Retire-RemoteFile"),
  );

  assert.ok(lease >= 0 && lease < predecessor);
  assert.ok(predecessor < marker && marker < finalGates);
  assert.ok(finalGates < cleanup && cleanup < release);
  assert.doesNotMatch(publishFunction, /Remove-RemoteExactFile[\s\S]*BackupPath|Remove-RemoteFileIfPresent[\s\S]*backup/i);
  assert.match(publisher, /function Invoke-RemoteExactRollback[\s\S]*Get-CustodyBoundedRollbackOrder -Journal/);
  assert.match(publisher, /failed-closed-lease-retained/);
});

test("never converts FTP 550 or SIZE failure into remote absence", () => {
  const publisher = read("tools/deploy-site.ps1");
  const sizeFunction = publisher.slice(
    publisher.indexOf("function Get-RemoteSize"),
    publisher.indexOf("function Get-RemoteBytes"),
  );
  assert.match(sizeFunction, /FTP 550; accessibility and existence are ambiguous/);
  assert.doesNotMatch(sizeFunction, /return \[int64\]-1|return -1/);
  assert.match(publisher, /Get-RemoteFileObservation[\s\S]*Remote SIZE and LIST observations contradict/);
});

test("verifies the manifest without reconstructing mutable repo-local staging", () => {
  const builder = read("tools/build-site-deployment-manifest.js");
  const publisher = read("tools/deploy-site.ps1");
  const ignore = read(".gitignore");
  assert.doesNotMatch(builder, /stagingRoot|deploy[\\/]staging/);
  assert.doesNotMatch(publisher, /deploy[\\/]staging/);
  assert.doesNotMatch(ignore, /deploy[\\/]staging/);
  assert.equal(fs.existsSync(path.join(repositoryRoot, "deploy/staging")), false);
  assert.match(builder, /--verify/);
  assert.match(publisher, /Get-CustodyGitBlobBytes/);
  assert.match(publisher, /New-CustodySealedSnapshot/);

  const manifestPath = path.join(repositoryRoot, "deploy/site-deployment-manifest.v1.json");
  const before = fs.statSync(manifestPath);
  const beforeHash = sha256(fs.readFileSync(manifestPath));
  const result = spawnSync(
    process.execPath,
    [
      path.join(repositoryRoot, "tools/build-site-deployment-manifest.js"),
      "7e0a6eb6bf292ff69e3030adf1d229c811606ce9",
      "--verify",
      manifestPath,
    ],
    { cwd: repositoryRoot, encoding: "utf8", timeout: 30_000 },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^VERIFIED /m);
  assert.equal(sha256(fs.readFileSync(manifestPath)), beforeHash);
  assert.equal(fs.statSync(manifestPath).mtimeMs, before.mtimeMs);
});

test("preserves a truthfully empty prepared catalog and peer feed", () => {
  const catalog = readJson("catalog/compositions.v1.json");
  const peers = readJson("catalog/peer-announcements.v1.json");
  const release = readJson(".well-known/site-release.json");
  assert.deepEqual(catalog.entries, []);
  assert.deepEqual(peers.announcements, []);
  assert.equal(release.version, "1.1.2");
  assert.equal(release.status, "prepared");
});

test("pins byte-authoritative deployment records to LF without normalizing historical receipts", () => {
  const attributes = read(".gitattributes");
  assert.match(attributes, /^deploy\/publication-authority\.v1\.json text eol=lf$/m);
  assert.match(attributes, /^deploy\/publication-root-sentinel\.v1\.json text eol=lf$/m);
  assert.match(attributes, /^deploy\/site-deployment-manifest\.v1\.json text eol=lf$/m);
  assert.match(attributes, /^deploy\/receipts\/\*\.json -text -diff$/m);
});
