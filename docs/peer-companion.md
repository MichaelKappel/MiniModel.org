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
4. A receiver uses Rust `inspect-peer-import-request`, `check-peer-status`, `inspect-peer-status-check`, and `preflight-peer-import-request` to verify the saved request, manifest, peer metadata, saved status report, and advertised companion state before piece bytes move, then `import-peer-request` or equivalent verifier logic to fetch from a consenting direct peer, assemble pieces, verify the `.slm`, write a receipt, and `inspect-import-receipt` before browser import.

The companion exists to make step 2 through step 4 understandable for power users without weakening the browser-local boundary.

## Status

The full local companion application remains roadmap only. Do not claim a signed MiniModel Peer Companion app is live until that application, consent flows, and release documentation exist. The Rust CLI now has companion-shaped primitives: `prepare-peer-share` writes a verified sender share package from a local `.slm`, `inspect-share-metadata` validates TinyRustLM metadata-only and share-kit-ready local share bundles, `write-peer-companion-state` writes verified metadata-only state, `inspect-peer-companion-state` revalidates that state against the manifest and piece directory, `write-peer-contact-card` writes a metadata-only reviewed known-peer URL card, `inspect-peer-contact-card` revalidates that card, `inspect-peer-source-announcement-feed` preflights existing metadata-only peer-source announcement feeds, `inspect-catalog-snapshot` preflights existing `models.v0.json` catalog snapshots, `inspect-peer-import-request` validates saved receiver handoff JSON against the manifest before network access, `inspect-peer-status-check` validates saved readiness JSON against the manifest without contacting the peer, `inspect-import-receipt` revalidates completed local import receipts against the manifest and assembled `.slm`, `write-import-receipt-ledger` writes a local metadata-only receipt index after reopening every receipt/manifest/artifact through Rust verification, `inspect-import-receipt-ledger` revalidates that index and rejects drift, `serve-peer-companion` validates companion state before serving the fixed companion-state route plus the existing peer-piece routes from a user-run local process, and `check-peer-status` fetches only `minimodel.peer_status.v0.json` to validate a running peer's metadata before piece preflight or transfer. CLI integration coverage now exercises the public `prepare-peer-share` command from the q8 fixture, including manifest, peer-offer, peer-piece, and catalog output; it validates public `inspect-peer-import-request` offline, starts the public `serve-peer-companion` binary, and runs public `check-peer-status`, `inspect-peer-status-check`, `preflight-peer-import-request`, `import-peer-request`, `inspect-import-receipt`, `inspect-peer-import-receipt`, `write-import-receipt-ledger`, and `inspect-import-receipt-ledger` commands against it to prove the receiver path; it exercises saved status-check validation, ledger metadata-only output plus stale receipt-SHA drift rejection; it exercises `inspect-share-metadata` against both valid TinyRustLM share-bundle profiles plus project-server-byte and chunk-list drift failures; it exercises `write-peer-companion-state` / `inspect-peer-companion-state` from a prepared q8 share, including metadata-only state output, normalized path-prefixed/root-dot peer URLs, state/piece-count drift rejection, project-server flag drift rejection, malformed host rejection, and project/Hugging Face host rejection before manifest or piece reads; it exercises `write-peer-contact-card` / `inspect-peer-contact-card`, including normalized path-prefixed peer URLs and project/Hugging Face host rejection before output; it also exercises `write-peer-source-announcement-feed` / `inspect-peer-source-announcement-feed`, including a prepared-share announcement feed, an empty feed, and project-server-flag rejection; and it exercises `inspect-catalog-snapshot` against normal/feed-bound snapshots plus project-server-flag drift.

TinyRustLM-generated local share kits also exercise the companion/audit primitives from the sender side: after `prepare-peer-share`, the script writes and inspects a local `.minimodel-receipt`, then writes and inspects a metadata-only `import-receipts.v0.json` ledger with `peer_import_count=0` before publishing optional peer-source, catalog, companion-state, or contact-card metadata.

The sender-side share kit now also writes and inspects `minimodel.peer_companion_inventory.v0` when a consenting Known peer URL is set. The inventory summarizes the companion state, manifest, peer-piece directory, sender-side receipt ledger, local piece-store byte/quota/SHA-256 evidence, optional peer contact-card metadata, and a `ready-after-rust-reverify` serve plan with exact status/piece/state routes and preflight commands, without carrying `.slm` bytes or peer-piece payloads. Duplicate contact-card peer URLs are rejected before a companion UI can present one endpoint as multiple peers.

TinyRustLM can load the same inventory as local browser metadata in its P2P panels. That UI preview validates the no-byte boundary and shows a Rust-reverify-required summary, but `inspect-peer-companion-inventory` remains the verifier gate before a companion trusts the file.

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
- export or import metadata-only peer contact cards for reviewed consenting direct peer URLs
- write portable peer-source announcements for catalog maintainers
- import from a consenting direct peer with local Rust verification
- maintain local import receipts, metadata-only receipt ledgers, and transfer logs
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
minimodel inspect-import-receipt <receipt.minimodel-receipt> <manifest> <artifact.slm>
minimodel write-import-receipt-ledger <output-json> <receipt> <manifest> <artifact.slm> [<receipt> <manifest> <artifact.slm>...]
minimodel inspect-import-receipt-ledger <ledger-json>
minimodel write-peer-companion-state <manifest> <piece-dir> <state-json> <known-peer-base-url>
minimodel inspect-peer-companion-state <state-json> <manifest> <piece-dir>
minimodel write-peer-companion-inventory <output-json> <state-json> <manifest> <piece-dir> <receipt-ledger-json> <piece-store-quota-bytes> [contact-card-json...]
minimodel inspect-peer-companion-inventory <inventory-json>
minimodel write-peer-contact-card <output-json> <known-peer-base-url> [peer-label]
minimodel inspect-peer-contact-card <card-json>
minimodel write-peer-contact-book <output-json> <contact-card-json>...
minimodel inspect-peer-contact-book <book-json>
minimodel inspect-share-metadata <bundle-json>
minimodel inspect-peer-source-announcement-feed <feed-json>
minimodel inspect-catalog-snapshot <catalog-json>
minimodel serve-peer-companion <state-json> <manifest> <piece-dir> <bind-addr>
minimodel check-peer-status <manifest> <known-peer-base-url> <output-json>
minimodel inspect-peer-status-check <status-check-json> <manifest>
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
minimodel inspect-peer-source-announcement-feed <feed-json>
minimodel write-catalog-snapshot-with-announcement-feed <models.v0.json> <feed-route> <feed-json> <catalog-entry-json>...
minimodel inspect-catalog-snapshot <models.v0.json>
```

If a receiver only needs a reviewed known-peer URL for a proof-ready row, the sender can share a metadata-only contact card instead of asking them to paste the URL manually:

```text
minimodel write-peer-contact-card <output-json> <known-peer-base-url> [peer-label]
minimodel inspect-peer-contact-card <card-json>
minimodel write-peer-contact-book <output-json> <contact-card-json>...
minimodel inspect-peer-contact-book <book-json>
minimodel inspect-share-metadata <bundle-json>
```

The card carries no `.slm` bytes, no peer pieces, and no catalog availability claim. It only lets TinyRustLM or a future companion fill the Known peer URL before Rust `import-peer-request` revalidates and transfers from that consenting peer.

For a small reviewed peer list, `write-peer-contact-book` embeds 1-64 validated cards into `minimodel.peer_contact_book.v0` with per-card SHA-256s and an explicit no-model-byte/no-peer-piece/no-catalog-claim boundary. `inspect-peer-contact-book` revalidates the book before a UI trusts it; TinyRustLM uses the first embedded peer as the active Known peer while still requiring Rust reverify before transfer.

The CLI integration test `peer_share_cli` keeps the sender-side public command path aligned with the core validator: it runs `prepare-peer-share` through the binary against the TinyRustLM q8 fixture, verifies the generated manifest, peer-offer, peer-piece sidecar, and catalog entry, checks the normalized `http://127.0.0.1:8123/peer-pieces/` URL, and verifies `http://MiniModel.org/peer-pieces` is rejected before artifact bytes are read. The `peer_import_cli` smoke keeps the receiver-side public path aligned: it exports pieces, writes companion state, starts `serve-peer-companion` on a path-prefixed loopback URL, waits for `check-peer-status`, runs `inspect-peer-status-check`, runs `preflight-peer-import-request`, runs `inspect-peer-import-preflight`, runs `import-peer-request`, runs `inspect-import-receipt`, runs `inspect-peer-import-receipt`, verifies metadata-only status and preflight reports, validates advertised companion state, verifies receipt evidence, rejects unsafe peer URL paths before manifest reads, rejects drifted project-server and piece-set receipt fields, rejects a future-dated preflight whose timestamp would make the receipt predate the preflight, and checks the imported `.slm` bytes match the source fixture. The `share_metadata_cli` test keeps TinyRustLM's browser export contract aligned with Rust: it accepts metadata-only and share-kit-ready `minimodel.local_slm_share_metadata.v0` bundles, rejects project-server-byte claims, and rejects chunk-list drift before a maintainer or converter treats the bundle as MiniModel metadata. The `peer_companion_state_cli` test keeps the companion-state public command path aligned with the core validator: it exports peer pieces through the binary, writes companion state through the binary, inspects that state through the binary, checks the same path-prefixed URL normalization, verifies the JSON remains metadata-only with no project-server-byte flags, rejects piece-count drift, rejects project-server flag drift, and verifies the project host is rejected before manifest or piece files are read. The companion `peer_contact_card_cli` test keeps the smaller card contract aligned: it writes a card through the binary, inspects it through the binary, checks the same path-prefixed URL normalization, and verifies the project host is rejected before any card file is created. The `peer_contact_book_cli` test keeps reviewed peer lists aligned: it writes two cards, writes and inspects a contact book, verifies embedded card hashes and source-path omission, and proves duplicate peer URLs fail before any book output. The `peer_source_announcement_feed_cli` test keeps feed publication aligned: it writes and inspects a prepared-share announcement feed, accepts an empty feed, and rejects project-server-byte flag drift before TinyRustLM or a maintainer applies the feed. The `catalog_snapshot_cli` test keeps final snapshot publication aligned: it writes and inspects normal and feed-bound snapshots, reports catalog SHA-256/counts, and rejects project-server flag drift before the snapshot is loaded or shared.

### P2P Import

P2P import must still end in local Rust verification.

Current CLI-equivalent receiver chain:

```text
minimodel inspect-peer-import-request <request-json> <manifest>
minimodel check-peer-status <manifest> <known-peer-base-url> <output-json>
minimodel inspect-peer-status-check <status-check-json> <manifest>
minimodel preflight-peer-import-request <request-json> <manifest> <output.preflight>
minimodel import-peer-request <request-json> <manifest> <output-dir>
minimodel inspect-import-receipt <receipt.minimodel-receipt> <manifest> <artifact.slm>
minimodel inspect-peer-import-receipt <receipt.minimodel-receipt> <request-json> <preflight> <manifest> <artifact.slm>
minimodel write-import-receipt-ledger <output-json> <receipt> <manifest> <artifact.slm> [<receipt> <manifest> <artifact.slm>...]
minimodel inspect-import-receipt-ledger <ledger-json>
```

The companion may wrap those commands with progress, logs, retry controls, and cache policy, but the acceptance criteria stay the same:

- request model id matches manifest model id
- artifact byte count matches
- artifact SHA-256 matches
- peer-offer proof matches
- peer status check fetched only `minimodel.peer_status.v0.json` metadata before preflight or piece bytes
- peer preflight fetched only `minimodel.peer_piece_set.v0` metadata before any piece bytes
- peer URL is a consenting direct peer, not a project/Hugging Face host, and not a malformed dot host
- chunks match declared SHA-256 rows
- chunk list and Merkle root match when present
- assembled `.slm` passes full artifact and runtime checks
- import receipt is written before TinyRustLM import
- import receipt is inspected against the assembled `.slm` before TinyRustLM import
- import receipt can be indexed in a metadata-only ledger that reopens the receipt, manifest, and artifact before later companion use

## Architecture

Use shared Rust verifier crates first. Choose the presentation shell later.

### Recommended Crate Boundaries

- `minimodel-core`: manifest, catalog, peer-source, peer-piece, `.slm`, checksum, and receipt verification.
- `minimodel`: CLI command surface that remains the reference behavior and test oracle.
- future `minimodel-companion-core`: application use cases such as prepare share, start serve session, import from peer, write announcement, enforce cache quota, and list receipts.
- future `minimodel-companion-store`: local metadata store for receipts, receipt ledgers, peer identities, transfer logs, cache index, and catalog snapshots; the current CLI receipt ledger is the first metadata-only file primitive, not the full store.
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
- peer companion state hashes
- receipt paths and hashes
- receipt ledger paths and hashes
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
- bind availability claims to artifact SHA-256, peer-offer route, chunk count, Merkle root, and peer-offer proof SHA-256
- allow key rotation
- avoid requiring a project account

The first identity proof can be metadata-only. It should not force MiniModel.org to become an account, relay, or byte-hosting service.

## Transport Phases

### Phase 0: Current Direct Known-Peer Prototype

Implemented by the CLI:

- `prepare-peer-share`
- `serve-peer-pieces`
- `serve-peer-companion`
- `inspect-peer-import-request`
- `check-peer-status`
- `inspect-peer-status-check`
- `fetch-peer-pieces`
- `assemble-peer-pieces`
- `import-peer-request`
- `inspect-import-receipt`
- `inspect-peer-import-receipt`
- `write-import-receipt-ledger`
- `inspect-import-receipt-ledger`
- `write-peer-companion-state`
- `inspect-peer-companion-state`
- `write-peer-companion-inventory`
- `inspect-peer-companion-inventory`
- `add-peer-source`
- `write-peer-source-announcement`
- `apply-peer-source-announcements`
- `write-peer-source-announcement-feed`
- `write-catalog-snapshot-with-announcement-feed`
- `write-catalog-snapshot-with-announcements`
- `write-catalog-snapshot`
- `inspect-catalog-snapshot`

The companion should wrap this before adding broader discovery.

`write-peer-companion-state` is the first CLI artifact shaped for that wrapper. It validates a prepared share directory, binds a consenting direct peer URL, and writes metadata-only JSON with manifest/artifact/piece-set checksums, piece count, Merkle root, command hints, and no-project-server-byte flags. `inspect-peer-companion-state` reruns the same state/manifest/piece-directory checks against an existing JSON file and reports the state, manifest, artifact, piece-set, chunk, and no-embedded-byte boundary before a script publishes or serves the handoff. `write-peer-companion-inventory` layers on top of that state by binding it to the sender-side receipt ledger, optional contact-card metadata, and local piece-store byte/quota/SHA-256 evidence; `inspect-peer-companion-inventory` revalidates that metadata-only summary, including piece-store SHA-256 drift and duplicate contact-card peer URL rejection, before a UI trusts it. `serve-peer-companion` is the matching local serving primitive: it validates the state against the manifest and peer-piece directory, serves `minimodel.peer_status.v0.json`, `minimodel.peer_companion_state.v0.json`, and keeps the existing allow-list behavior for `minimodel.peer_piece_set.v0` plus declared chunk files. Path-prefixed base URLs such as `http://127.0.0.1:8123/peer-pieces/` are honored by serving those same allow-listed routes under the prefix. `serve-peer-pieces` also exposes the metadata-only status route for peers without companion state. Receiver-side `check-peer-status` validates that route before preflight, and `inspect-peer-status-check` revalidates the saved readiness report before a script trusts it; `preflight-peer-import-request`, `fetch-peer-pieces`, and `import-peer-request` validate companion state when the route exists and record `not-advertised` for older piece-only peers. Completed `import-peer-request` receipts also record the direct peer transport, piece-set SHA-256, companion-state status, chunk/Merkle contract, fetched peer-piece bytes, assembled-from-peer-pieces status, and no-project-server-byte flags; `inspect-import-receipt` and `inspect-peer-import-receipt` recheck that evidence against the manifest, request, preflight report, and assembled `.slm`. `write-import-receipt-ledger` then lets a local companion or audit trail index one or more verified receipts without copying model bytes, while `inspect-import-receipt-ledger` reopens every receipt, manifest, and artifact before trusting the index. A future UI can read that state, inventory, ledger, status-check, and receipt evidence without parsing model bytes or inventing a separate verifier path.

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
- local SQLite or equivalent only for metadata, receipts, receipt ledgers, peer identities, transfer logs, and cache indexes
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
- receipt ledger SHA-256
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
- peer-offer route/checksum/chunk-count/Merkle drift rejection
- peer-source URL project-host rejection
- duplicate peer-source deduplication
- chunk SHA-256 drift rejection
- chunk-list SHA-256 drift rejection
- Merkle root drift rejection
- interrupted fetch cleanup
- cache quota enforcement
- receipt replay rejection or clear receipt replay semantics
- receipt ledger drift rejection
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
