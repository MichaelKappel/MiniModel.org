# MiniModel Peer Companion Design

The MiniModel Peer Companion is a later optional local application for advanced peer workflows. It is not implemented as a hosted service, and it is not required for the first TinyRustLM user path.

The current simple path stays:

1. User selects a local `.slm`.
2. Rust verifies the file, manifest, checksums, and `.slm` structure.
3. TinyRustLM imports the same local file through the browser file picker.

The current exchange path stays:

1. A converter or user creates a verified `.slm`.
2. MiniModel `prepare-peer-share` writes a manifest, peer-offer proof, local peer pieces, and catalog-entry metadata.
3. Metadata-only catalog entries or peer-source announcements make availability discoverable.
4. A receiver uses Rust `preflight-peer-import-request` to verify the peer metadata first, then `import-peer-request` or equivalent verifier logic to fetch from a consenting direct peer, assemble pieces, verify the `.slm`, and write a receipt.

The companion exists to make step 2 through step 4 understandable for power users without weakening the browser-local boundary.

## Status

Roadmap only. Do not claim a MiniModel Peer Companion is live until a signed local application, CLI-equivalent verification tests, consent flows, and release documentation exist.

## Non-Negotiable Rules

- Project servers never run user model bytes.
- Project servers never serve user model bytes.
- Project servers never proxy Hugging Face, peer, or local model-byte downloads.
- MiniModel.org may publish docs, schemas, examples, metadata catalogs, public-key routes, peer-source announcement feeds, and takedown/contact routes only.
- The companion is user-run local software. It is not MiniModel.org infrastructure.
- Rust owns manifest parsing, `.slm` validation, chunk verification, peer-piece assembly, and receipt writing.
- JavaScript or a WebView shell must not become a second trust path for model bytes.
- Every serve, fetch, import, cache, or availability-publish action requires explicit user consent.

## Product Job

The companion should turn the advanced peer workflow into a local control center:

- show which verified `.slm` files the user has chosen to share
- prepare local peer pieces from a verified artifact
- start and stop a user-owned peer-piece server
- show whether a catalog entry is proof-ready, P2P-ready, or missing peer availability
- write portable peer-source announcements for catalog maintainers
- import from a consenting direct peer with local Rust verification
- maintain local import receipts and transfer logs
- enforce cache quotas and let the user remove stored pieces
- hand the verified output path back to TinyRustLM for browser file-picker import

It should reduce command complexity, not hide trust decisions.

## Non-Goals

- no hosted model marketplace
- no project-hosted `.slm`, safetensors, GGUF, ONNX, tokenizer, or adapter bytes
- no remote inference service
- no distributed inference or P2P KV-cache sharing in the current product
- no automatic relay operated by project infrastructure
- no auto-import into TinyRustLM without a local verification receipt and user file-picker step
- no default dependency on a JavaScript runtime for core verification or transfer

## Source Lanes

The companion should preserve the same three source lanes as TinyRustLM and MiniModel.

### Local Files

The user provides a local `.slm`. The companion can call the same Rust verification path as:

```text
minimodel verify-manifest <manifest>
minimodel verify-artifact <manifest> <artifact.slm>
minimodel write-import-receipt <manifest> <artifact.slm> <receipt>
```

The output is a local receipt. The model still enters TinyRustLM through the browser file picker.

### Hugging Face Metadata And Local Conversion

Hugging Face remains metadata and user-side acquisition context. The companion may help organize local conversion requests and local source files, but it must not use project-owned tokens, proxy `/resolve/` routes, or treat Hub model-card results as runnable.

Accepted local chain:

```text
tinyrustlm-slm-pack convert-huggingface-request <request.json> <source.safetensors> <output-dir>
minimodel prepare-peer-share <output.slm> <share-dir> <model-id> [source-huggingface-model-id] [known-peer-base-url]
minimodel write-catalog-snapshot <models.v0.json> <catalog-entry-json>...
```

If peer availability is known later, use metadata-only publication:

```text
minimodel add-peer-source <catalog-entry-json> <with-peer-json> <known-peer-base-url>
minimodel write-peer-source-announcement <catalog-entry-json> <announcement-json> <known-peer-base-url>
minimodel apply-peer-source-announcements <catalog-entry-json> <output-json> <announcement-json>...
minimodel write-catalog-snapshot-with-announcements <models.v0.json> <catalog-entry-json>... --announcements <announcement-json>...
minimodel write-peer-source-announcement-feed <feed-json> <announcement-json>...
minimodel write-catalog-snapshot-with-announcement-feed <models.v0.json> <feed-route> <feed-json> <catalog-entry-json>...
```

### P2P Import

P2P import must still end in local Rust verification.

Current CLI-equivalent receiver chain:

```text
minimodel preflight-peer-import-request <request-json> <manifest> <output.preflight>
minimodel import-peer-request <request-json> <manifest> <output-dir>
```

The companion may wrap those commands with progress, logs, retry controls, and cache policy, but the acceptance criteria stay the same:

- request model id matches manifest model id
- artifact byte count matches
- artifact SHA-256 matches
- peer-offer proof matches
- peer preflight fetched only `minimodel.peer_piece_set.v0` metadata before any piece bytes
- peer URL is a consenting direct peer and is not a project server
- chunks match declared SHA-256 rows
- chunk list and Merkle root match when present
- assembled `.slm` passes full artifact and runtime checks
- import receipt is written before TinyRustLM import

## Architecture

Use shared Rust verifier crates first. Choose the presentation shell later.

### Recommended Crate Boundaries

- `minimodel-core`: manifest, catalog, peer-source, peer-piece, `.slm`, checksum, and receipt verification.
- `minimodel`: CLI command surface that remains the reference behavior and test oracle.
- future `minimodel-companion-core`: application use cases such as prepare share, start serve session, import from peer, write announcement, enforce cache quota, and list receipts.
- future `minimodel-companion-store`: local metadata store for receipts, peer identities, transfer logs, cache index, and catalog snapshots.
- future `minimodel-companion-transport`: adapters for direct HTTP first, then Iroh/QUIC, libp2p, WebRTC, WebTransport, or other transports only after the verifier contract is preserved.
- future presentation shell: Tauri, Iced, Slint, egui, Dioxus, or another UI after the domain layer is stable.

The dependency direction should point inward:

```text
presentation -> application use cases -> verifier/domain
transport/storage adapters -> application use cases -> verifier/domain
```

The verifier/domain layer must not depend on the UI shell.

## Runtime Concurrency Pattern

If the companion adds a long-running network worker, treat that worker as an actor rather than shared mutable UI state. The UI should send typed commands to the worker and receive typed status events back. The worker owns peer sessions, serve sessions, fetch sessions, retries, cancellation, and shutdown.

Rules:

- do not share a transport swarm or local server through broad `Mutex` state
- use message passing between presentation, application use cases, and transport workers
- keep cancellation explicit for every serve or fetch session
- make shutdown drain logs and receipts before sockets or local stores close
- test worker use cases without the presentation shell

This keeps direct HTTP, future Iroh/QUIC, future libp2p, or future WebRTC/WebTransport adapters behind the same application contract.

## Tauri Boundary If Used

Tauri v2 is plausible only as a shell around Rust-owned commands. If selected later:

- use strict capabilities
- expose a narrow command allowlist
- keep file access in Rust
- keep peer transfer in Rust
- keep manifest verification in Rust
- keep receipt writing in Rust
- do not expose arbitrary filesystem paths to frontend code
- do not let JavaScript fetch, assemble, or validate model bytes
- test commands without the WebView as normal Rust application use cases

Pure Rust UI options remain open. The product does not need a shell decision before the local verifier and direct known-peer handoff are solid.

## Local State

The companion may store local metadata. It should not store secrets or model bytes casually.

Acceptable local state:

- known local `.slm` artifact references chosen by the user
- manifest paths and hashes
- catalog snapshot hashes
- peer-source announcement hashes
- receipt paths and hashes
- transfer logs
- cache index
- local peer identity public key
- user consent records
- cache quota settings

Sensitive state:

- private peer identity keys
- credentials for any external service
- user-selected private directories

Sensitive state belongs in the operating system secure store where available. Do not write private keys or tokens into bundled files, public docs, catalog JSON, or portable receipts.

## Piece Store

The piece store is local user storage for chunk files and sidecars.

Rules:

- piece files are model bytes
- piece files must never be deployed to MiniModel.org, TinyRustLM.com, or documentation sites
- piece files must be tied to a manifest, artifact SHA-256, chunk size, chunk count, chunk-list SHA-256, and Merkle root
- cache eviction must be explicit and understandable
- serving pieces must be opt-in per model or per session
- stopping the companion must stop local serving unless the user explicitly enabled a background service later

## Consent Model

Use two separate consent gates.

Transfer consent:

- user agrees to fetch pieces from a named peer
- user agrees to serve pieces for a named model
- user agrees to publish peer-source availability metadata

Import consent:

- user reviews the verified receipt
- user chooses the output `.slm`
- user imports through TinyRustLM's file picker

The companion should log consent decisions locally, but those logs are not public catalog evidence unless the user exports them.

## Peer Identity

Peer identity is a later phase. Direct known-peer HTTP can exist without global identity.

When identity is added:

- generate keys locally
- keep private keys in OS secure storage
- publish only public keys or signed metadata
- bind availability claims to artifact SHA-256 and peer-offer proof
- allow key rotation
- avoid requiring a project account

The first identity proof can be metadata-only. It should not force MiniModel.org to become an account, relay, or byte-hosting service.

## Transport Phases

### Phase 0: Current Direct Known-Peer Prototype

Implemented by the CLI:

- `prepare-peer-share`
- `serve-peer-pieces`
- `fetch-peer-pieces`
- `assemble-peer-pieces`
- `import-peer-request`
- `add-peer-source`
- `write-peer-source-announcement`
- `apply-peer-source-announcements`
- `write-peer-source-announcement-feed`
- `write-catalog-snapshot-with-announcement-feed`
- `write-catalog-snapshot-with-announcements`
- `write-catalog-snapshot`

The companion should wrap this before adding broader discovery.

### Phase 1: Local Companion Wrapper

Add local UI around known-peer direct transfer:

- choose artifact
- prepare share
- start local server
- copy peer URL
- import from peer URL
- show verification progress
- write receipt
- open output folder

No relay, no swarm, no DHT.

### Phase 2: Local Discovery Experiments

Evaluate LAN-friendly discovery after Phase 1:

- mDNS
- local peer beacons
- signed availability metadata
- explicit allowlist

Still keep direct transfer receipt gates.

### Phase 3: Wider P2P Transport Evaluation

Compare transports only after the verifier and consent model are stable:

- Iroh/QUIC
- Rust-libp2p
- WebRTC
- WebTransport
- BitTorrent-style chunking
- Pear/Bare/Hypercore as design references

Selection criteria:

- Rust-first verifier integration
- resumable chunk transfer
- NAT traversal behavior
- relay requirements
- ability to reject project-server byte paths
- local identity and key storage story
- deterministic testability
- dependency risk
- packaging and update burden

## July 3 Intake Triage

The desktop/P2P reports are useful, but their recommendations do not all fit the current product boundary.

Accepted for the companion roadmap:

- multi-crate Rust architecture with verifier/domain code isolated from presentation
- strict Tauri capability and IPC boundaries if a WebView shell is used
- message-passing or actor-style network workers instead of shared global transport state
- OS secure storage for private peer identity keys or external credentials
- local SQLite or equivalent only for metadata, receipts, peer identities, transfer logs, and cache indexes
- signed releases, published checksums, and platform-appropriate packaging before broad distribution
- domain tests before UI tests, with drift rejection and consent gates as the first test surface

Deferred research lanes:

- WebTransport with certificate-hash pinning
- WebRTC Direct or manual out-of-band SDP exchange
- mDNS and local network discovery
- DHT routing or bootstrap strategies
- UPnP/NAT-PMP port mapping
- Iroh/QUIC, Rust-libp2p, BitTorrent-style chunking, Pear, Bare, and Hypercore

Rejected for the current product:

- project-operated STUN/TURN, relay, rendezvous, signaling, bootstrap, or DHT infrastructure as a default dependency
- project-server proxying, caching, or serving of `.slm`, `.safetensors`, GGUF, ONNX, tokenizer, adapter, or peer-piece bytes
- project-run remote inference or distributed inference
- browser JavaScript or Service Worker code becoming the verifier or model-byte assembly authority
- automatic background serving before explicit user consent

Any future project-operated discovery or relay service would be a separate legal, operational, and product decision after direct known-peer transfer and the local companion wrapper are proven. It still must preserve checksum-bound manifests, local receipts, explicit consent, and the no-server-byte rule.

## UI Requirements

The companion is for advanced users, but it should still be understandable.

Beginner-safe labels:

- Local files
- P2P import
- Start serving
- Stop serving
- Prepare share
- Import from peer
- Write announcement
- View receipt

Advanced details:

- artifact SHA-256
- chunk count
- chunk-list SHA-256
- Merkle root
- peer-offer checksum
- manifest route and checksum
- catalog-entry checksum
- peer-source announcement checksum
- signature/key identity

Do not hide verification results behind vague success messages. A successful import should show exactly which model id, byte count, artifact SHA-256, manifest, peer URL, output path, and receipt path were accepted.

## Test Plan

Domain tests first:

- manifest drift rejection
- catalog-entry drift rejection
- peer-offer drift rejection
- peer-source URL project-host rejection
- duplicate peer-source deduplication
- chunk SHA-256 drift rejection
- chunk-list SHA-256 drift rejection
- Merkle root drift rejection
- interrupted fetch cleanup
- cache quota enforcement
- receipt replay rejection or clear receipt replay semantics
- unsafe local path rejection
- consent-required command rejection

Application tests:

- prepare share from verified artifact
- serve session start/stop
- import session happy path
- import session bad peer
- import session bad chunk
- write portable announcement
- apply announcement to proof-ready catalog
- local catalog load and P2P-ready row promotion

Presentation tests:

- browser-only UI tests with mocked Rust commands for fast feedback
- full native smoke tests only after shell selection
- screenshot or accessibility checks for text overflow and disabled states

Release tests:

- signed binary checksum publication
- Windows signing plan
- macOS notarization plan
- Linux package integrity plan
- update channel rollback plan

## Documentation Requirements

Before a companion release exists, docs must say "optional future companion" or "roadmap." After a release exists, docs must say:

- exactly what it can do
- exactly what it cannot do
- how it stores local data
- how users stop serving pieces
- how users delete cached pieces
- how to verify a receipt
- how to report takedown or abuse concerns
- that MiniModel.org still does not host model bytes

## Release And Update Gate

A companion release is not credible until distribution is boring and verifiable.

Required before broad release:

- signed binaries or installers for supported platforms
- published SHA-256 checksums for release artifacts
- Windows signing plan
- macOS signing and notarization plan
- Linux package integrity plan
- rollback plan for broken updates
- no long-lived signing or service credentials in the repository
- reproducible domain-level test run for verifier, transfer, receipt, and unsafe-path rejection

## Open Questions

- Should the first companion be a thin GUI around the CLI, or a separate application crate that shares domain code with the CLI?
- Should local state use SQLite or plain files first?
- Should peer identity be added before wider transport, or only after direct known-peer transfer is smooth?
- Should same-origin peer-source announcement feeds be enough for early shared catalogs, or should a signed transparency log come earlier?
- Which release-signing path is practical first for Windows and macOS?

## Completion Gate For First Companion Prototype

A first prototype is credible only when all of the following are true:

- it reuses MiniModel Rust verifier logic
- it can prepare a share from a verified local `.slm`
- it can start and stop a local known-peer serve session
- it can import from a consenting direct peer
- it writes the same receipt semantics as the CLI
- it refuses project-server model-byte paths
- it exposes clear consent prompts
- it has domain tests for drift and unsafe URL rejection
- it has docs that preserve the no-server-byte boundary
- TinyRustLM still imports only a local user-selected `.slm`
