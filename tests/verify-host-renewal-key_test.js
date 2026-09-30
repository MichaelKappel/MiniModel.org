"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const path = require("node:path");
const { Readable } = require("node:stream");
const { spawnSync } = require("node:child_process");
const test = require("node:test");
const { verifyKeyBytes, run } = require("../tools/verify-host-renewal-key.js");

// Public RFC 8032 section 7.1 TEST 1 vector, never a deployed/account key.
// https://www.rfc-editor.org/rfc/rfc8032#section-7.1
const TEST_SEED = Buffer.from("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60", "hex");
const TEST_PUBLIC = Buffer.from("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", "hex");
const PASS = "host-renewal-key-match\n", FAIL = "host-renewal-key-rejected\n";

/** Allocate public fixture bytes separately so mutation tests cannot contaminate another case. */
function fixture() { return Buffer.concat([TEST_SEED, TEST_PUBLIC]); }

/** Capture only the helper's public fixed protocol, never echoing supplied input. */
async function capture(chunks, args = []) {
  let stdout = "", stderr = "";
  const code = await run(args, Readable.from(chunks), { write: value => { stdout += value; } },
    { write: value => { stderr += value; } });
  return { code, stdout, stderr };
}

test("known public Ed25519 vector derives the exact public key without mutating caller-owned bytes", () => {
  const input = fixture(), before = Buffer.from(input);
  assert.equal(verifyKeyBytes(input), true); assert.deepEqual(input, before);
  for (const offset of [0, 31, 32, 63]) {
    const changed = fixture(); changed[offset] ^= 1;
    assert.equal(verifyKeyBytes(changed), false);
  }
  for (const value of [null, undefined, "", new Uint8Array(64), Buffer.alloc(0), Buffer.alloc(31),
    Buffer.alloc(32), Buffer.alloc(63), Buffer.alloc(65), Buffer.alloc(65536)]) assert.equal(verifyKeyBytes(value), false);
});

test("derivation failure or wrong exported algorithm framing yields only false and clears temporary DER seed bytes", t => {
  const original = crypto.createPrivateKey;
  let borrowed;
  t.mock.method(crypto, "createPrivateKey", options => { borrowed = options.key; return original(options); });
  assert.equal(verifyKeyBytes(fixture()), true); assert.equal(borrowed.every(value => value === 0), true);
  t.mock.method(crypto, "createPublicKey", () => { throw new Error("synthetic private diagnostic must not escape"); });
  assert.equal(verifyKeyBytes(fixture()), false); assert.equal(borrowed.every(value => value === 0), true);
  t.mock.restoreAll();
  t.mock.method(crypto, "createPrivateKey", () => ({ asymmetricKeyType: "rsa" }));
  assert.equal(verifyKeyBytes(fixture()), false);
  t.mock.restoreAll();
  for (const output of ["not a buffer", Buffer.alloc(43), Buffer.alloc(44)]) {
    t.mock.method(crypto, "createPublicKey", () => ({ export: () => output }));
    assert.equal(verifyKeyBytes(fixture()), false); t.mock.restoreAll();
  }
});

test("binary stdin handles arbitrary chunk boundaries and exact EOF with fixed pass/fail output", async () => {
  const input = fixture();
  for (const chunks of [[input], [input.subarray(0, 1), input.subarray(1, 33), input.subarray(33)],
    Array.from(input, value => Buffer.from([value])), [Buffer.alloc(0), input, Buffer.alloc(0)]]) {
    assert.deepEqual(await capture(chunks), { code: 0, stdout: PASS, stderr: "" });
  }
  for (const chunks of [[], [input.subarray(0, 63)], [input, Buffer.from([0])], [Buffer.alloc(65536)],
    ["not binary"], [Buffer.concat([TEST_SEED, Buffer.alloc(32)])]]) {
    assert.deepEqual(await capture(chunks), { code: 1, stdout: "", stderr: FAIL });
  }
  assert.deepEqual(await capture([input], ["--key"]), { code: 1, stdout: "", stderr: FAIL });
  assert.deepEqual(await capture([input], null), { code: 1, stdout: "", stderr: FAIL });
});

test("an oversized frame stops reading immediately, and stream errors cannot reveal input or exception text", async () => {
  let consumed = 0, closed = false, stdout = "", stderr = "";
  async function* oversized() {
    try { consumed += 1; yield Buffer.alloc(65); consumed += 1; yield fixture(); }
    finally { closed = true; }
  }
  assert.equal(await run([], oversized(), { write: value => { stdout += value; } }, { write: value => { stderr += value; } }), 1);
  assert.equal(consumed, 1); assert.equal(closed, true); assert.equal(stdout, ""); assert.equal(stderr, FAIL);
  async function* broken() { yield fixture().subarray(0, 32); throw new Error("synthetic seed detail must not escape"); }
  stderr = "";
  assert.equal(await run([], broken(), { write: value => { stdout += value; } }, { write: value => { stderr += value; } }), 1);
  assert.equal(stdout, ""); assert.equal(stderr, FAIL);
});

test("actual CLI reads only raw stdin and emits exactly the fixed protocol for match, mismatch and malformed invocation", () => {
  const script = path.resolve(__dirname, "../tools/verify-host-renewal-key.js");
  const environment = { ...process.env }; delete environment.NODE_OPTIONS;
  for (const [args, input, success] of [[[], fixture(), true], [[], Buffer.alloc(64), false], [[], Buffer.alloc(63), false],
    [[], Buffer.alloc(65), false], [["--file", "not-read"], fixture(), false]]) {
    const result = spawnSync(process.execPath, [script, ...args], { input, encoding: "utf8", timeout: 5000,
      windowsHide: true, shell: false, maxBuffer: 4096, env: environment });
    assert.equal(result.error, undefined); assert.equal(result.status, success ? 0 : 1);
    assert.equal(result.stdout, success ? PASS : ""); assert.equal(result.stderr, success ? "" : FAIL);
  }
});
