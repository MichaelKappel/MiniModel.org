"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");
const { buildStaticSeedPlan } = require("../tools/build-static-seed-plan.js");
const { descriptorBytes } = require("./helpers/static-seed-fixture.js");

const tinyRoot = path.resolve(__dirname, "../../TinyRustLM.com");
const tool = path.resolve(__dirname, "../tools/build-static-seed-plan.js");
const canonical = (value) => Buffer.from(`${JSON.stringify(value)}\n`, "utf8");
const hash = (value) => crypto.createHash("sha256").update(value).digest("hex");

/** Mutate one field of synthetic metadata while keeping its JSON encoding canonical. */
function changed(mutate) {
  const value = JSON.parse(descriptorBytes());
  mutate(value);
  return canonical(value);
}

/** Exercise the real stdout-only entry point with bounded time and output. */
function cli(input, args = []) {
  return spawnSync(process.execPath, [tool, ...args], {
    input, encoding: "utf8", timeout: 10000, maxBuffer: 128 * 1024,
    windowsHide: true, env: { ...process.env, NODE_V8_COVERAGE: "" },
  });
}

test("companion canonical bytes produce the native descriptor identity and six exact URLs", () => {
  const bytes = descriptorBytes();
  const value = JSON.parse(bytes);
  const plan = buildStaticSeedPlan(bytes);
  assert.deepEqual(buildStaticSeedPlan(bytes), plan);
  assert.equal(plan.publicationAllowed, false);
  assert.deepEqual(plan.requiredServerConfiguration, { context: "server-or-virtual-host",
    directives: ["TraceEnable Off"], verified: false });
  assert.equal(plan.status, "prepared-not-verified-or-published");
  assert.equal(plan.pieceSetId, `sha256:${hash(bytes)}`);
  assert.equal(plan.descriptorSha256, hash(bytes));
  assert.equal(plan.compositionId, value.compositionId);
  assert.equal(plan.files.length, 6);
  const profile = fs.readFileSync(path.join(tinyRoot, "contracts/vnext/acg2-format-profile.v1.json"));
  assert.equal(hash(profile), value.compositionProfileSha256);
  for (const [index, file] of plan.files.entries()) {
    const member = value.members[index];
    assert.equal(file.url, `https://minimodel.org/tinyrustlm/direct-composition/v1/compositions/${value.compositionId}/members/${member.name}`);
    assert.equal(file.remotePath, `${plan.remoteDirectory}/${member.name}`);
    assert.equal(file.bytes, member.bytes);
    assert.equal(file.sha256, member.sha256);
    assert.deepEqual(file.expectedHeaders, {
      "content-type": "application/octet-stream", "content-length": String(member.bytes),
      "content-encoding": null, "access-control-allow-origin": "https://tinyrustlm.com",
      "x-tinyrustlm-composition-id": value.compositionId, "x-tinyrustlm-piece-set-id": plan.pieceSetId,
      "x-tinyrustlm-member-name": member.name, "x-tinyrustlm-member-id": `sha256:${member.sha256}`,
    });
  }
});

test("configuration binds every member without forcing length, hiding encoding or copying payloads", () => {
  const plan = buildStaticSeedPlan(descriptorBytes());
  const config = plan.configuration;
  assert.equal(config.bytes, Buffer.byteLength(config.text));
  assert.equal(config.sha256, hash(config.text));
  assert.equal(config.remotePath, `${plan.remoteDirectory}/.htaccess`);
  assert.ok(config.text.endsWith("\n") && !config.text.includes("\r"));
  assert.equal((config.text.match(/^<Files /gm) || []).length, 6);
  assert.match(config.text, /^Options -Indexes -MultiViews$/m);
  assert.match(config.text, /^# Requires TraceEnable Off in server\/vhost config/m);
  assert.doesNotMatch(config.text, /^TraceEnable /m);
  assert.match(config.text, /^RewriteCond %\{QUERY_STRING\} !\^\$$/m);
  assert.match(config.text, /^RewriteCond %\{REQUEST_METHOD\} !\^\(GET\|HEAD\)\$$/m);
  assert.match(config.text, /^SetEnv no-gzip 1$/m);
  assert.match(config.text, /^SetEnv no-brotli 1$/m);
  assert.match(config.text, /^ForceType application\/octet-stream$/m);
  assert.match(config.text, /^Header always set Cache-Control "no-store, no-transform"$/m);
  assert.match(config.text, /^Header always set Allow "GET, HEAD"$/m);
  assert.doesNotMatch(config.text, /Header .* (?:set|unset) Content-(?:Length|Encoding)/);
  assert.doesNotMatch(config.text, /Allow-Credentials "true"|Allow-Origin "\*"|<IfModule/);
  assert.ok(config.text.includes(`Header always set X-TinyRustLM-Piece-Set-Id "${plan.pieceSetId}"`));
  const exposed = config.text.match(/^Header always set Access-Control-Expose-Headers "([^"]+)"$/m)[1].split(", ");
  for (const name of ["Content-Length", "Content-Type", "Content-Encoding", "X-TinyRustLM-Composition-Id",
    "X-TinyRustLM-Piece-Set-Id", "X-TinyRustLM-Member-Name", "X-TinyRustLM-Member-Id"]) {
    assert.ok(exposed.includes(name));
  }
  for (const file of plan.files) {
    const section = config.text.split(`<Files "${file.name}">`)[1].split("</Files>")[0];
    assert.ok(section.includes(`Header always set X-TinyRustLM-Member-Id "sha256:${file.sha256}"`));
    assert.ok(section.includes(`Header always set X-TinyRustLM-Member-Name "${file.name}"`));
  }
  assert.doesNotMatch(JSON.stringify(plan), /[EDX]:\\|"access_token"|"password"|"private_key"/);
  const allowed = new RegExp(config.text.match(/^RewriteRule !(.*) - \[R=404,L\]$/m)[1]);
  for (const file of plan.files) assert.equal(allowed.test(file.name), true);
  for (const name of [".htaccess", "seed.json", "model.slm", "modelXslm2", "MODEL.SLM2",
    "../model.slm2", "model.slm2/", "model.slm2.bak", "folder/model.slm2"]) {
    assert.equal(allowed.test(name), false, name);
  }
});

test("metadata changes produce new content identities and bounded native-compatible sizes", () => {
  const original = buildStaticSeedPlan(descriptorBytes());
  for (const mutate of [
    (v) => { v.compositionId = `sha256:${"a".repeat(64)}`; },
    (v) => { v.members[0].sha256 = "b".repeat(64); },
    (v) => { v.members[0].bytes = 68_719_476_736; },
  ]) {
    const bytes = changed(mutate);
    const result = buildStaticSeedPlan(bytes);
    assert.notEqual(result.pieceSetId, original.pieceSetId);
    assert.equal(result.pieceSetId, `sha256:${hash(bytes)}`);
  }
  // Bind every inclusive role ceiling to the owning profile instead of redistributing its budget.
  const profile = JSON.parse(fs.readFileSync(path.join(tinyRoot, "contracts/vnext/acg2-format-profile.v1.json")));
  const maximum = changed((v) => {
    for (const member of profile.memberProfiles) v.members[member.ordinal].bytes = member.maximumArtifactBytes;
    v.members[5].bytes = profile.artifactFormat.fileBytes;
  });
  assert.equal(buildStaticSeedPlan(maximum).files.reduce((sum, file) => sum + file.bytes, 0),
    profile.limits.maximumResolvedMemberBytes + profile.artifactFormat.fileBytes);
  const over = JSON.parse(maximum); over.members[1].bytes += 1;
  assert.throws(() => buildStaticSeedPlan(canonical(over)), /static-seed-descriptor-member/);
});

test("seed preparation rejects every native role ceiling overflow before generating a plan", async (t) => {
  const profile = JSON.parse(fs.readFileSync(path.join(tinyRoot, "contracts/vnext/acg2-format-profile.v1.json")));
  const limits = [...profile.memberProfiles.map((member) => member.maximumArtifactBytes), profile.artifactFormat.fileBytes];
  for (const [index, maximum] of limits.entries()) {
    await t.test(JSON.parse(descriptorBytes()).members[index].name, () => {
      assert.throws(() => buildStaticSeedPlan(changed((value) => { value.members[index].bytes = maximum + 1; })),
        /static-seed-descriptor-member/);
    });
  }
});

test("rejects unknown, injected, reordered and unsupported metadata before generating configuration", async (t) => {
  const mutations = {
    schema: (v) => { v.schema = "old"; },
    protocol: (v) => { v.protocol = "other"; },
    profile: (v) => { v.compositionProfile = "other"; },
    profileHash: (v) => { v.compositionProfileSha256 = "0".repeat(64); },
    idType: (v) => { v.compositionId = null; },
    idInjection: (v) => { v.compositionId += '\nHeader set Evil "1"'; },
    idUppercase: (v) => { v.compositionId = v.compositionId.toUpperCase(); },
    membersType: (v) => { v.members = {}; },
    missingMember: (v) => { v.members.pop(); },
    extraMember: (v) => { v.members.push(v.members[0]); },
    order: (v) => { v.members.reverse(); },
    nullMember: (v) => { v.members[0] = null; },
    arrayMember: (v) => { v.members[0] = []; },
    pathTraversal: (v) => { v.members[0].name = "../../index.html"; },
    duplicateName: (v) => { v.members[0].name = v.members[1].name; },
    zeroBytes: (v) => { v.members[0].bytes = 0; },
    negativeBytes: (v) => { v.members[0].bytes = -1; },
    stringBytes: (v) => { v.members[0].bytes = "25"; },
    fractionalBytes: (v) => { v.members[0].bytes = 25.5; },
    overflowBytes: (v) => { v.members[0].bytes = Number.MAX_SAFE_INTEGER + 1; },
    memberCeiling: (v) => { v.members[0].bytes = 68_719_476_737; },
    graphSize: (v) => { v.members[5].bytes = 895; },
    hashType: (v) => { v.members[0].sha256 = null; },
    hashPrefix: (v) => { v.members[0].sha256 = `sha256:${v.members[0].sha256}`; },
    hashUppercase: (v) => { v.members[0].sha256 = v.members[0].sha256.toUpperCase(); },
    memberExtraField: (v) => { v.members[0].path = "D:\\private"; },
    topExtraField: (v) => { v.pieceSetId = `sha256:${"0".repeat(64)}`; },
    unknownFirstField: (v) => { const entries = Object.entries(v).reverse();
      for (const key of Object.keys(v)) delete v[key]; Object.assign(v, Object.fromEntries(entries)); },
  };
  for (const [name, mutate] of Object.entries(mutations)) {
    await t.test(name, () => assert.throws(() => buildStaticSeedPlan(changed(mutate)), /static-seed-descriptor-/));
  }
});

test("rejects alternate encodings, duplicate fields, malformed UTF-8 and oversized input", () => {
  const good = descriptorBytes();
  for (const bytes of [null, good.toString(), Buffer.alloc(0), Buffer.alloc(8193),
    Buffer.from([0xff]), Buffer.from("not-json"), canonical(null), canonical([]), canonical(42),
    good.subarray(0, good.length - 1), Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), good]),
    Buffer.from(good.toString().replace(/\n$/, "\r\n")),
    Buffer.from(`${JSON.stringify(JSON.parse(good), null, 2)}\n`),
    Buffer.from(good.toString().replace('"bytes":25', '"bytes":25,"bytes":25')),
    Buffer.from(good.toString().replace('"bytes":25', '"bytes":2.5e1')),
    Buffer.from(good.toString().replace("model.slm2", "model\\u002eslm2")),
  ]) assert.throws(() => buildStaticSeedPlan(bytes), /static-seed-descriptor-/);
});

test("CLI emits one preparation record and refuses invalid input without partial output", () => {
  const good = cli(descriptorBytes());
  assert.equal(good.error, undefined);
  assert.equal(good.status, 0, good.stderr);
  assert.equal(good.stderr, "");
  assert.deepEqual(JSON.parse(good.stdout), buildStaticSeedPlan(descriptorBytes()));
  for (const [bytes, args] of [[Buffer.alloc(0), []], [Buffer.alloc(8193), []],
    [Buffer.from("private-invalid-input"), []], [descriptorBytes(), ["--publish"]]]) {
    const result = cli(bytes, args);
    assert.equal(result.error, undefined);
    assert.equal(result.status, 1);
    assert.equal(result.stdout, "");
    assert.doesNotMatch(result.stderr, /private-invalid-input/);
    assert.match(result.stderr, /static-seed-descriptor-|usage:/);
  }
});
