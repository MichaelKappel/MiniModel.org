"use strict";

const crypto = require("node:crypto");

const INPUT_BYTES = 64;
const PRIVATE_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const PUBLIC_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const PASS = "host-renewal-key-match\n";
const FAIL = "host-renewal-key-rejected\n";

/** Compare a supplied Ed25519 seed with its expected raw public key without exporting private material. */
function verifyKeyBytes(input) {
  if (!Buffer.isBuffer(input) || input.length !== INPUT_BYTES) return false;
  const encoded = Buffer.alloc(PRIVATE_PREFIX.length + 32);
  try {
    PRIVATE_PREFIX.copy(encoded);
    input.copy(encoded, PRIVATE_PREFIX.length, 0, 32);
    const privateKey = crypto.createPrivateKey({ key: encoded, format: "der", type: "pkcs8" });
    if (privateKey.asymmetricKeyType !== "ed25519") return false;
    const publicKey = crypto.createPublicKey(privateKey).export({ format: "der", type: "spki" });
    if (!Buffer.isBuffer(publicKey) || publicKey.length !== PUBLIC_PREFIX.length + 32 ||
        !publicKey.subarray(0, PUBLIC_PREFIX.length).equals(PUBLIC_PREFIX)) return false;
    return crypto.timingSafeEqual(publicKey.subarray(PUBLIC_PREFIX.length), input.subarray(32));
  } catch (_) {
    return false;
  } finally {
    // Clear the temporary DER seed copy. Node/OpenSSL KeyObject memory lifetime
    // remains owned by the runtime; this is not a claim of whole-process erasure.
    encoded.fill(0);
  }
}

/** Consume exactly one bounded binary frame through EOF; the invoking publisher owns the process deadline. */
async function run(args, input, stdout, stderr) {
  const bytes = Buffer.alloc(INPUT_BYTES);
  let length = 0, accepted = false;
  try {
    if (!Array.isArray(args) || args.length !== 0) throw new Error("invalid invocation");
    for await (const chunk of input) {
      if (!Buffer.isBuffer(chunk) || chunk.length > INPUT_BYTES - length) throw new Error("invalid frame");
      chunk.copy(bytes, length); length += chunk.length;
    }
    accepted = length === INPUT_BYTES && verifyKeyBytes(bytes);
  } catch (_) {
    accepted = false;
  } finally {
    bytes.fill(0);
  }
  // No input, key, exception, digest, or derived public key can reach either pipe.
  if (accepted) stdout.write(PASS);
  else stderr.write(FAIL);
  return accepted ? 0 : 1;
}

module.exports = Object.freeze({ verifyKeyBytes, run });
if (require.main === module) {
  run(process.argv.slice(2), process.stdin, process.stdout, process.stderr)
    .then(code => { process.exitCode = code; })
    .catch(() => { process.stderr.write(FAIL); process.exitCode = 1; });
}
