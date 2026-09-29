# MiniModel.org Catalog Boundary

MiniModel.org is the metadata catalog and discovery surface for qualified tiny
model compositions. The operator has authorized MiniModel.org to host the
initial model seeds. It is not an inference service, conversion service, or
model-quality authority.

## Model-Byte Boundary

MiniModel.org supports direct P2P discovery, but it does not proxy model bytes.
The HTTPS MiniModel.org server may be a consenting initial seed, serving its
own verified files directly from `D:\LLMs\TinyRustLM`. If an
exact qualified peer composition is unavailable, an installed local companion
may acquire a pinned public source and perform an explicit local conversion.
TinyRustLM.com does not serve model bytes, and neither site receives prompts.
`project_server_serves_model_bytes` declares actual seed hosting as a boolean;
proxying and server inference must remain false. The empty catalog still
declares false until the seed is actually deployed.

The current TinyRustLM composition format is SLM2. Catalog metadata must bind
the exact composition identity, member sizes and digests, source revision,
license and provenance, qualification claims, runtime requirements, and the
cryptographic piece/manifests needed for verification. An alias must not point
to different bytes.

Static seed preparation rejects lengths above each role's existing ACG2 profile
ceiling: model 68,719,476,736 bytes; tokenizer 67,108,864; template 1,048,576;
sampling 65,800; and prompt 512. The composition graph is exactly 896 bytes.
Unused capacity in one role cannot authorize a larger member in another role.
These checks only reject impossible descriptor sizes before configuration is
generated; actual file parsing, qualification, browser resource admission and
public seed verification remain separate requirements.

Every catalog document fixes `browser_admission_schema` to
`tinyrustlm.browser-composition-admission/1`. Every qualified entry must carry
one exact `browser_admission` descriptor containing only a safe same-origin
metadata route, its positive byte count of at most 64 KiB, and the lowercase
prefixed SHA-256 digest of its canonical bytes. TinyRustLM fetches that metadata
separately and must verify the declared length, digest, canonical serialization,
and schema before browser activation. The admission is metadata, not a seventh
SLM2 member. Its descriptor grants no byte-transfer authority; the operator's
separate initial-seed authorization permits only the exact verified composition
files. Arbitrary model-byte proxying remains outside this contract. The empty
prelaunch catalog publishes the schema identity but no entry,
descriptor, claim, or evidence.

Both catalog and peer feed require a positive safe-integer `revision`, canonical
second-precision `generated_at_utc` and `valid_until_utc`, and a current window
of at most 24 hours. Future skew is limited to five minutes. The peer feed must
be contained within the catalog window. A source update requires a strictly
newer revision and generation time; consumers reject rollback and same-revision
equivocation. Expiry is failure, not permission to reuse stale metadata.

`qualification_receipt_schema` is exactly `tinyrustlm.qualification-receipt/1`.
Each entry binds `receipt_id`, a safe same-origin `receipt_route`, `receipt_bytes`
from 2 through 131072, and a lowercase prefixed `receipt_sha256`. These are
descriptors, not receipt verification: the consumer must check the actual bytes.
The catalog also requires unique `revoked_qualification_receipt_ids` and
`supersessions`. Revoked receipts and compositions superseded at the catalog's
generation time cannot be listed as current.

Peer verifiers are bounded `Ed25519` keys with SHA-256 key IDs, canonical
unpadded base64url 32-byte public keys, and UTC validity intervals. Each
announcement binds its composition, piece set, consent and outside-proof
receipt digests, protocol, normalized peer origin, issuance, expiry, and key ID
in a detached 64-byte signature over the canonical newline-terminated
`tinyrustlm.peer-announcement-proof/1` payload. The key must cover the entire
announcement interval. Both the site and consumer verify signatures
asynchronously before projecting a peer-backed view. A verified signature
authenticates metadata; it does not establish reachability or prove the claims.

The normalized exact `https://minimodel.org/` operator seed requires
`outside_proof_status: "not-required-operator-seed"` and an explicit JSON null
`outside_proof_receipt_sha256`. This declares operator responsibility without
claiming an independent network observation. Every other admitted peer still
requires `current-passed` and a SHA-256 receipt identity. Spoofed domains, HTTP
downgrades, nondefault ports and alternate paths cannot obtain the operator
disposition. Both fields remain inside the detached signature. Qualification,
explicit consent, member hashes, TLS and the 24-hour metadata limit still apply.
Renewal may extend the same approved seed's availability metadata; it cannot
create qualification evidence or turn a signature into a reachability result.

## Empty Metadata Preparation

Catalog strings preserve Unicode scalar values, including supplementary
characters and combining sequences, without normalization or replacement.
Both direct validators and byte parsers reject lone UTF-16 surrogates. UTF-8
streams retain a leading BOM for canonical rejection; it must not be silently
removed. Canonical spelling remains compact JSON.stringify output plus one LF,
so alternate Unicode escapes and duplicate keys do not acquire another identity.

The page loads catalog and peer metadata under one ten-second deadline, including
signature verification. An explicit caller may select 1–60,000 milliseconds or
provide an AbortSignal. Reload cancels the preceding operation; a failed request
cancels its sibling and cannot leave old model actions visible. Late completion
cannot overwrite the newer page state. Responses stream through fatal UTF-8
decoding, with at most one MiB of decoded bytes and 4,096 chunks per document.
Oversized declared lengths reject before reading the body. No whole-body text
fallback is used. Cancellation stops observation of uncooperative injected
dependencies; it does not claim that arbitrary custom promises or Web Crypto
work can be forcibly terminated. Normal browser Fetch receives the same signal.

`tools/build-empty-catalog.js` is limited to the empty prelaunch inventory.
It never creates keys, signatures, entries, receipts, or proofs, and refuses to
erase existing keys, entries, revocations, supersessions, or announcements.
Generation requires explicit `--revision`, `--generated-at-utc`,
`--valid-until-utc`, and `--document catalog` or `--document peerFeed` arguments.
Output goes only to stdout for review; source changes and publication are
separate operations. No expiry window is extended automatically.

Run `node tools/build-empty-catalog.js --check` to check the exact local source
bytes and wall-clock freshness. Run
`node --test tests/catalog.test.js tests/catalog-contract.test.js tests/deployment-custody.test.js`
for deterministic source-window, cross-contract, and custody tests. The
cross-contract suite reads only the named canonical TinyRustLM browser module
from its sibling repository; it does not copy source or contact peers. Synthetic
test metadata and ephemeral test signatures have no qualification authority.

## Catalog Claims

A catalog row is metadata, not proof of availability or quality. The following
claims remain independent:

- source and license binding;
- structural composition acceptance;
- numerical parity;
- model-quality qualification;
- current peer availability;
- outside-network transfer proof;
- browser activation; and
- public site deployment and readback.

An empty, fresh announcement set is valid metadata; expired announcements are
rejected and do not make a model available. Loopback, LAN, fixture, mocked, or
source-only evidence does not establish a public P2P lane.

## Transfer And Installation

Direct peer import requires exact composition and piece-set agreement before
mixing pieces, bounded concurrency, per-piece cryptographic verification,
durable progress, final whole-member verification, and atomic activation. A
piece match alone does not establish qualification.

Every admitted peer announcement must set `protocol_profile` to exactly
`tinyrustlm.direct-composition-http.v1`, the browser-consumable direct
composition HTTP profile. Former names, aliases, version fallbacks, and
compatibility negotiation are not accepted.

Its `peer_base_url` must be an exact root origin: an optional trailing `/` is
canonicalized, but credentials, endpoint paths, query strings, and fragments
are rejected. Public peer URLs require HTTPS and HTTP is loopback-only. Public
HTTPS community peers require a public, non-reserved IP literal. DNS names, including
`localhost`, are rejected because the browser cannot pin their resolution.
Loopback is limited to exact `127.0.0.1` or `[::1]` literals. Explicit ports must
be from 1 through 65535; default ports normalize to the URL origin. Project and
service hosts and the application's own origin cannot serve peer model bytes,
except for the exact operator seed origin `https://minimodel.org`. That origin
uses HTTPS certificate validation, may appear on MiniModel.org itself, and
does not authorize alternate ports, subdomains, arbitrary DNS peers, proxying,
or unverified model activation. Signed announcements and all artifact checks
still apply.
Loopback admission does not establish public reachability or outside-network
P2P evidence.

The local-conversion path must pin and verify the public source revision,
respect source licensing, expose storage and resource requirements, execute a
bounded registered conversion, and run independent inspection before
installation. There is no remote-inference fallback and no instruction-file
download masquerading as model acquisition.

## Privacy

Catalog and peer protocols expose only required public-safe metadata. Logs and
receipts must exclude prompts, local paths, credentials, invitation URLs,
tokens, and raw private evidence. MiniModel.org does not receive TinyRustLM
conversation content.

## MemoryEndpoints Ecosystem Policy

MemoryEndpoints.com has two separate authorized uses elsewhere in the
ecosystem:

1. Agents working on all maintained sites use it directly for agent-to-agent
   long-term memory, meetings, handoffs, and coordination.
2. Separately, the TinyRustLM.com Connect plugin lets a user sign in and
   explicitly select MemoryEndpoints.com as TinyRustLM.com Connect long-term
   memory.

Those lanes keep identity, credentials, consent, scopes, provenance, and
receipts separate. MemoryEndpoints.com is not a MiniModel model-byte transport,
peer relay, catalog authority, or inference service. Prelaunch memory may also
be duplicated in current `.uai`, wiki, public-safe source notes, and approved
private stores only while every maintained copy remains current, secret-safe,
provenance-labeled, and noncontradictory.

## Publication

- Source version: `1.1.2`.
- Publication status and dates are owned by the [current release](../.well-known/site-release.json) and [release history](../.well-known/site-release-history.json), rather than duplicated in site copy.
- Update: public peer URLs require HTTPS and HTTP is loopback-only; exact
  root-origin and authority checks are fail-closed.
- Inventory remains exactly zero qualified catalog entries and zero current
  peer announcements.

Publication requires a clean committed revision, complete site gates,
deployment from that revision, and exact public HTTPS readback. Source control
or a reachable development page is not publication evidence.
