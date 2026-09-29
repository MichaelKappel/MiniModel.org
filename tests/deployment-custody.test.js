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
  ["publication-bindings", "separates predecessor and candidate discovery and requires real publication dates"],
  ["repository-identity", "rejects a wrong checkout, mirror, branch state, or remote head"],
  ["transport-authority", "rejects an unbound or wrong FTPS host, port, user, profile, or root"],
  ["mutation-order", "forbids local and public mutation before read-only admission"],
  ["public-predecessor", "rejects stale 1.1.1 public bytes and rewritten ledger history"],
  ["remote-overlay", "rejects missing and overlay entries in the publication root"],
  ["remote-observation", "treats FTP 550 and contradictory SIZE/LIST evidence as ambiguous failure"],
  ["rollback-order", "bounds the transaction journal and rolls paths back in exact reverse order"],
  ["lease-ownership", "binds deployment lease cleanup to one exact operation owner"],
  ["published-materialization", "materializes published records without rewriting prepared source facts"],
]) {
  test(description, () => runHarness(name));
}

test("exercises bounded input, transfer and rollback using canonical functions without source copies", () => {
  assert.equal(repositoryRoot.toLowerCase(), "e:\\source\\rust\\minimodel.org");
  function assertPlainAncestors(target) {
    let current = target;
    for (;;) {
      assert.equal(fs.lstatSync(current).isSymbolicLink(), false, `reparse ancestor: ${current}`);
      const parent = path.dirname(current);
      if (parent === current) break;
      current = parent;
    }
  }
  assertPlainAncestors(repositoryRoot);
  const parent = path.join(repositoryRoot, "artifacts", "tests");
  assertPlainAncestors(path.dirname(parent));
  fs.mkdirSync(parent, { recursive: true });
  assertPlainAncestors(parent);
  const operation = crypto.randomUUID();
  const owned = path.join(parent, `publication-custody-${operation}`);
  fs.mkdirSync(owned);
  const marker = path.join(owned, "owner.json");
  const markerBytes = Buffer.from(JSON.stringify({ operation, suite: "publication-custody" }) + "\n");
  fs.writeFileSync(marker, markerBytes, { flag: "wx" });
  try {
    const command = [
      "$ErrorActionPreference='Stop'",
      "$result=Invoke-Pester -Script @('./tests/MiniModel.DeploymentCustody_test.ps1','./tests/MiniModel.StaticSeed_test.ps1') -CodeCoverage @('./tools/MiniModel.DeploymentCustody.psm1','./tools/MiniModel.StaticSeed.psm1') -PassThru -Quiet",
      "$coverage=$result.CodeCoverage",
      "$changedCoverage=@('ConvertFrom-CustodyDirectoryListing','Assert-CustodyBootstrapManifest') | ForEach-Object { $name=$_; $hit=@($coverage.HitCommands | Where-Object Function -eq $name).Count; $miss=@($coverage.MissedCommands | Where-Object Function -eq $name).Count; [pscustomobject]@{name=$name;hit=$hit;analyzed=($hit+$miss)} }",
      "$result.TestResult | Where-Object { -not $_.Passed } | Select-Object Name,FailureMessage | ConvertTo-Json -Compress | Write-Output",
      "[pscustomobject]@{total=$result.TotalCount;passed=$result.PassedCount;failed=$result.FailedCount;hit=$coverage.NumberOfCommandsExecuted;analyzed=$coverage.NumberOfCommandsAnalyzed;changedFunctions=@($changedCoverage)} | ConvertTo-Json -Compress | Write-Output",
      "if ($result.FailedCount -ne 0) { exit 1 }",
    ].join("\n");
    const result = spawnSync("powershell.exe", ["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", command], {
      cwd: repositoryRoot, encoding: "utf8", windowsHide: true, timeout: 120_000,
      maxBuffer: 2 * 1024 * 1024, env: { ...process.env, TEMP: owned, TMP: owned },
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stdout + result.stderr);
    const summary = JSON.parse(result.stdout.trim().split(/\r?\n/).at(-1));
    assert.equal(summary.failed, 0);
    assert.ok(summary.total > 0);
    assert.equal(summary.passed, summary.total);
    assert.ok(summary.hit > 0);
    console.log(`Publication custody Pester: ${JSON.stringify(summary)}`);
  } finally {
    assertPlainAncestors(owned);
    assert.deepEqual(fs.readFileSync(marker), markerBytes, "test output ownership changed");
    let entries = 0;
    function verifyEmptyOutput(directory, depth = 0) {
      assert.ok(depth <= 4, "test output depth exceeded");
      for (const item of fs.readdirSync(directory, { withFileTypes: true })) {
        assert.ok(++entries <= 128, "test output entry bound exceeded");
        const full = path.join(directory, item.name);
        assert.equal(fs.lstatSync(full).isSymbolicLink(), false, "test output contains a reparse point");
        if (full === marker) continue;
        assert.ok(item.isDirectory(), "unexpected test output file; preserving it for review");
        verifyEmptyOutput(full, depth + 1);
      }
    }
    verifyEmptyOutput(owned);
    fs.rmSync(owned, { recursive: true });
    assert.equal(fs.existsSync(owned), false);
  }
});

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

test("binds the observed transport tuple without claiming sentinel or website publication", () => {
  const authority = readJson("deploy/publication-authority.v1.json");
  assert.equal(authority.repository.root_policy, "script-parent-equals-git-toplevel");
  assert.equal(Object.hasOwn(authority.repository, "canonical_root"), false);
  assert.equal(authority.transport.status, "bound");
  assert.equal(authority.transport.host, "server41.shared.spaceship.host");
  assert.equal(authority.transport.port, 21);
  assert.equal(authority.transport.username_sha256, "efbabe420071a41d5b24570f14e8e9b5a72db8cde22dcb2437d03dc6baeba3b1");
  assert.equal(authority.transport.remote_root, ".");
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

test("performs exact public admission before input creation or credential access", () => {
  const publisher = read("tools/deploy-site.ps1");
  const publicAdmission = publisher.indexOf("Assert-CustodyPublicPredecessor -Authority $authority");
  const phaseAdvance = publisher.indexOf('$script:phase = "public-admitted"');
  const transportBinding = publisher.indexOf('if ($authority.transport.status -cne "bound")', phaseAdvance);
  const input = publisher.indexOf("$script:publicationInput = New-CustodyPublicationInput", transportBinding);
  const credentialAccess = publisher.indexOf("$profile = Get-CredentialProfile", input);

  assert.ok(publicAdmission >= 0);
  assert.ok(publicAdmission < phaseAdvance);
  assert.ok(phaseAdvance < transportBinding);
  assert.ok(transportBinding < input);
  assert.ok(input < credentialAccess);
  const credentialImport = publisher.indexOf("    Import-MiniModelCredentialModule", input);
  assert.ok(input < credentialImport && credentialImport < credentialAccess);
  assert.doesNotMatch(publisher, /FileZilla|XmlReader|sitemanager\.xml/);
  assert.match(publisher, /Get-DeploymentCredentialProfile -ProfilePath \$Path -SourceRoot \$repositoryRoot -ExpectedProfileName \$ProfileName/);
  const moduleBytes = fs.readFileSync(path.join(repositoryRoot, "../TinyRustLM.com/tinyrustlm/tools/Deployment.Credentials.psm1"));
  assert.ok(publisher.includes(`$Stream.Length -ne ${moduleBytes.length}`));
  assert.ok(publisher.includes(`$actual -cne '${sha256(moduleBytes)}'`));
  assert.match(publisher, /Assert-MiniModelCredentialModuleIdentity -Stream \$lease/);
  assert.match(publisher, /Import-Module -Name \$path -Global -Force/);
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
  const input = publisher.indexOf("$script:publicationInput = New-CustodyPublicationInput");
  const credential = publisher.indexOf("$profile = Get-CredentialProfile");
  const ftpUse = publisher.indexOf("$script:lease = New-RemoteDeploymentLease");

  assert.ok(gateCall >= 0);
  assert.ok(moduleImport > gateCall, "editable custody module import preceded source-authority gate");
  for (const later of [remoteHead, publicRead, input, credential, ftpUse]) {
    assert.ok(later > gateCall, `transport-capable action at ${later} preceded source-authority gate`);
  }
  assert.match(publisher, /Invoke-VerifiedNode -Tool source-authority -Arguments \$gateArguments/);
  assert.match(publisher, /\$gateArguments \+= @\("--require-clean", "--require-remote-exact"\)/);
  assert.match(publisher, /Invoke-VerifiedNode -Tool manifest -Arguments/);
  assert.doesNotMatch(publisher, /& node\b/i);
  assert.match(publisher, /TinyRustLM\.com\\tinyrustlm/);
  assert.match(publisher, /New-FtpRequest[\s\S]*if \(-not \$script:sourceAuthorityAdmitted\)/);
  assert.doesNotMatch(publisher, /E:\\Source\\Rust\\MiniModel\.org/);
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

test("rejects the stale manifest without reconstructing mutable repo-local staging", () => {
  const builder = read("tools/build-site-deployment-manifest.js");
  const publisher = read("tools/deploy-site.ps1");
  const custody = read("tools/MiniModel.DeploymentCustody.psm1");
  const ignore = read(".gitignore");
  assert.doesNotMatch(builder, /stagingRoot|deploy[\\/]staging/);
  assert.doesNotMatch(publisher, /deploy[\\/]staging/);
  assert.doesNotMatch(ignore, /deploy[\\/]staging/);
  assert.equal(fs.existsSync(path.join(repositoryRoot, "deploy/staging")), false);
  assert.match(builder, /--verify/);
  assert.match(publisher, /Get-CustodyGitBlobBytes/);
  assert.match(publisher, /New-CustodyPublicationInput/);
  assert.doesNotMatch(publisher + custody, /SealedSnapshot|snapshotRoot|snapshotParent|GetTempPath|BlobReader|Custody\.NativeMethods/);
  assert.doesNotMatch(custody, /WriteAllBytes|WriteAllText|Copy-Item|CreateDirectory/);
  assert.match(publisher, /publication_input_sha256 = \$script:publicationInputSha256/);

  const manifestPath = path.join(repositoryRoot, "deploy/site-deployment-manifest.v1.json");
  const before = fs.statSync(manifestPath);
  const beforeHash = sha256(fs.readFileSync(manifestPath));
  const result = spawnSync(
    process.execPath,
    [
      path.join(repositoryRoot, "tools/build-site-deployment-manifest.js"),
      "7e0a6eb6bf292ff69e3030adf1d229c811606ce9",
      "--verify",
    ],
    { cwd: repositoryRoot, encoding: "utf8", timeout: 90_000, windowsHide: true },
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Canonical public source differs/);
  assert.equal(result.stdout, "");
  assert.equal(sha256(fs.readFileSync(manifestPath)), beforeHash);
  assert.equal(fs.statSync(manifestPath).mtimeMs, before.mtimeMs);
});

test("actual publisher checks both Node children despite hostile startup settings and stops before public admission", () => {
  const receipts = path.join(repositoryRoot, "deploy/receipts");
  const before = fs.readdirSync(receipts).sort();
  const manifestPath = path.join(repositoryRoot, "deploy/site-deployment-manifest.v1.json");
  const beforeManifest = fs.readFileSync(manifestPath);
  const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", path.join(repositoryRoot, "tools/deploy-site.ps1"), "-NodeExecutablePath", process.execPath], {
    cwd: repositoryRoot, encoding: "utf8", windowsHide: true, timeout: 120_000, maxBuffer: 65536,
    env: { ...process.env, NODE_OPTIONS: "--require definitely-not-loaded", NODE_PATH: "definitely-not-searched" },
  });
  assert.ifError(result.error);
  assert.notEqual(result.status, 0);
  assert.match(result.stdout + result.stderr, /Deployment manifest or current catalog admission failed: catalog is stale/);
  assert.doesNotMatch(result.stdout + result.stderr, /Cannot find module|READ-ONLY-ADMISSION-PASS|Get-CredentialProfile/);
  assert.deepEqual(fs.readdirSync(receipts).sort(), before);
  assert.deepEqual(fs.readFileSync(manifestPath), beforeManifest);
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
