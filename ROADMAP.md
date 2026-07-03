# MiniModel.org Roadmap

MiniModel.org is planned as a Rust-first peer-to-peer discovery and verification layer for tiny local AI model packages. It is not a model host and not an inference service.

## Core Rule

Project servers never run user models and never serve user model bytes. They may serve static documentation, signed catalog metadata, schemas, contact/takedown routes, and checksum-bearing manifest examples.

## Architecture

1. `minimodel-core`: Rust verification library for manifests, checksums, chunks, identities, and import receipts.
2. `minimodel` CLI: local verifier for `.slm` artifacts, license/model-card fields, evidence routes, one-command peer-share preparation after conversion, metadata-only catalog snapshot writing, direct known-peer piece serve/fetch, peer-piece export/reassembly, catalog-entry writing from manifest plus peer-offer proof, one-command TinyRustLM peer-import request execution, and TinyRustLM import readiness.
3. Static catalog: metadata-only listings and peer hints, with no model weights.
4. Optional peer companion: later user-run Rust desktop app for consent-based discovery, local peer identity, local piece stores, resumable transfer, NAT traversal, local quotas, and checksum validation.
5. TinyRustLM handoff: verified local `.slm` files open in the browser-local Rust/WASM runtime.

## Source Options

MiniModel.org should support three source lanes, all ending in local verification:

- User-owned local files and lists: users bring `.slm` files and optional local JSON metadata lists from their own machine.
- Hugging Face discovery: users may search or inspect external Hugging Face metadata, model cards, file hints, and source revisions, but project servers do not proxy `/resolve/` downloads and do not use project-owned tokens.
- MiniModel P2P: consenting peers exchange chunks that are verified by checksum and Merkle data before import. The first prototype is a direct `http://` peer-piece sender/receiver pair from a user-provided peer endpoint; discovery, identity exchange, relays, and NAT traversal remain later.

All source lanes must converge on a user-controlled local `.slm` artifact, a local verifier result or receipt, and a browser file-picker import into TinyRustLM. Hugging Face Inference Providers, Inference Endpoints, and executable Spaces are remote execution products, not MiniModel or TinyRustLM run paths.

The Hugging Face lane should produce local handoff files when no admitted `.slm` metadata exists yet. The conversion-request JSON identifies the upstream model id, preferred `.slm` quantization targets, required review/conversion/eval gates, and the rule that project servers do not serve the resulting bytes. TinyRustLM may also save a `.tinyrustlm-convert.ps1` script that runs the local Rust packer command `convert-huggingface-request` against a user-owned local `.safetensors` file, then runs MiniModel `prepare-peer-share` to write the local manifest, peer-offer proof, peer pieces, catalog entry, and optional peer-source announcement metadata, followed by `write-catalog-snapshot`, `write-catalog-snapshot-with-announcements`, or `write-catalog-snapshot-with-announcement-feed` to produce metadata-only `models.v0.json`. The script does not download model bytes.

Once one user or converter completes the chain, TinyRustLM can export a `minimodel.local_slm_share_metadata.v0` bundle with artifact SHA-256, chunk hashes, and a Merkle root, and MiniModel can now run `prepare-peer-share` to produce the local manifest, deterministic `.minimodel-peer-offer` text manifest, peer-piece directory, and catalog entry in one output directory. If a consenting direct peer is already known, the same command can validate and publish `peer_source_count` / `peer_sources` metadata for that peer without making MiniModel.org a byte host. If peer availability becomes known later, `add-peer-source` can attach a validated consenting peer URL directly, or `write-peer-source-announcement` can produce portable availability metadata that a catalog maintainer validates with `apply-peer-source-announcements` before the next metadata-only snapshot. For many entries and announcement files, `write-catalog-snapshot-with-announcements` validates everything together, rejects unmatched announcement model ids, and writes the final metadata-only `models.v0.json` snapshot directly. For browser-visible late availability without rewriting catalog entries, `write-peer-source-announcement-feed` validates portable announcements into a metadata feed and `write-catalog-snapshot-with-announcement-feed` binds that same-origin feed route into `models.v0.json` by SHA-256. `write-catalog-snapshot` still revalidates generated entries, rejects artifact/model/download URLs and project-server peer hosts, and writes a metadata-only catalog snapshot that TinyRustLM can later search. TinyRustLM treats proof-ready rows as a P2P import-request path: when a catalog peer source exists from a validated entry or announcement feed, it can save the request/script immediately; otherwise the receiver supplies one consenting peer URL first. Rust `import-peer-request` then validates the request against the manifest, fetches pieces, assembles the `.slm`, verifies it again, and writes a receipt in one output directory. The MiniModel CLI can already prepare peer shares, attach later peer-source metadata, write and apply peer-source announcements, publish checksum-bound announcement feeds, batch-publish snapshots from announcements or feed routes, write catalog snapshots, export, serve, fetch, assemble, reverify peer pieces, and execute TinyRustLM peer-import requests for known consenting peers; broader discovery and availability still remain later. That is the durable bridge toward users obtaining the admitted `.slm` through P2P instead of repeating a large source download and conversion.

TinyRustLM can also follow catalog-declared, same-origin peer-source announcement feed routes when the catalog binds those routes to SHA-256 checksums. That browser fetch is only for small JSON metadata; it applies the same announcement validation rules and never requests MiniModel-hosted, TinyRustLM-hosted, or Hugging Face-hosted model bytes. Empty announcement feeds are valid availability checks and keep proof-ready rows at "known peer needed" until a real consenting peer source is published.

## Optional Peer Companion

The July 3 desktop/P2P research batch points to an optional MiniModel Peer Companion as a later phase, not as a replacement for the browser app. The companion would run on the user's machine and make advanced peer work easier: local piece storage, peer identity, serve/fetch controls, cache quotas, explicit consent prompts, import receipts, and local logs.

The companion must reuse the same Rust verifier crates and command semantics as the CLI. It should not create a second trust path in JavaScript or a privileged WebView. If the UI uses Tauri v2 later, the Rust backend must own file access, peer transfer, manifest verification, and receipt writing through narrow commands and strict capabilities. Pure Rust UI options such as Iced, Slint, egui, or Dioxus remain open until the product shape requires a desktop shell.

Protocol choices remain research lanes. Direct known-peer HTTP is the current proof vehicle; Iroh/QUIC, Rust-libp2p, WebRTC, WebTransport, mDNS, Kademlia DHTs, BitTorrent-style chunking, and Pear/Bare/Hypercore can be evaluated only after they preserve manifest-bound chunks, local receipts, consent gates, and project-server byte rejection. The companion is still user infrastructure, not MiniModel.org infrastructure.

The July 3 intake also included infrastructure-assisted designs such as STUN/TURN, relays, rendezvous services, signaling servers, bootstrap nodes, and DHT entry points. Those are not part of the current product. Any project-operated discovery or relay service would be a separate legal, operational, and product decision after direct known-peer transfer and the local companion wrapper are proven, and it still could not host, proxy, cache, run, or serve model bytes.

The first durable companion specification is [docs/peer-companion.md](docs/peer-companion.md). It defines the local-only boundary, source lanes, Rust-owned command contracts, local state, piece-store rules, consent gates, transport phases, UI requirements, tests, and completion gate for a future prototype.

## Manifest MVP

Each model entry should bind:

- model id, publisher identity, model-card route, and license route
- artifact kind, byte count, final checksum, model shape, quantization, tokenizer checksum, and tensor-layout checksum
- source checkpoint identity and source revision when known
- config, tokenizer, eval, source-review, source-validation, runtime-smoke, and admission evidence checksums when available
- chunk size, Merkle root, and piece metadata for peer transfer
- signature fields for publisher identity and future transparency logs

The first concrete schema draft is [docs/manifest-v0.md](docs/manifest-v0.md). It uses a line-oriented `key=value` format so the initial Rust verifier can stay small and local.

## Verifier MVP

The first implementation target is a local Rust verifier plus the smallest possible direct peer-piece transfer prototype. The initial `minimodel-core` and `minimodel` crates now parse manifests, reject server-byte and server-execution claims, verify a user-supplied local `.slm` artifact, check TinyRustLM compatibility fields, validate chunk/Merkle metadata when present, prepare a one-command sender share package from a local `.slm`, write catalog entries only after manifest/peer-offer drift checks pass, attach later consenting peer-source metadata to proof-ready entries, write metadata-only catalog snapshots only after entry validation passes, export local peer pieces, serve those pieces from a user-started direct HTTP peer endpoint, fetch peer pieces from a consenting direct HTTP peer endpoint, reassemble those pieces with full re-verification, execute TinyRustLM peer-import request JSON as a one-command receiver flow, and write a local import receipt.

Verifier planning lives in [docs/verifier-mvp.md](docs/verifier-mvp.md).

## Legal Boundary

MiniModel.org should act as a metadata directory and information-location tool. Listings should include uploader assertions, license warnings, intended-use fields, prohibited-use fields, consent screens, and takedown/contact routes. Legal review is required before public exchange features.

Catalog and server-boundary rules live in [docs/catalog-boundary.md](docs/catalog-boundary.md).

## Ecosystem Direction

Rust-native verified transfer is the first path. Rust-libp2p, BitTorrent/WebTorrent, Pear, Bare, Hypercore, and Hyper ecosystem pieces remain research lanes, not mandatory dependencies. Pear/Bare/Hypercore are especially useful references for zero-infrastructure P2P application design, but MiniModel.org should keep its core verifier and transfer logic Rust-first.

## Tiny Model Path

Model creation and import should follow the TinyRustLM evidence chain: safetensors review, source extraction, source validation, conversion to `.slm`, runtime smoke, eval evidence, admission, and local import receipt. Distillation, pruning, quantization, ternary weights, and small-vocabulary designs are creation strategies, not server responsibilities.

## Phases

1. Publish manifest schema, model-card fields, legal boundary, and static catalog rules.
2. Build the Rust verifier and checksum drift tests.
3. Write local import receipts for verified `.slm` files.
4. Publish metadata-only static catalog prototypes that distinguish proof-ready rows with no known peer from rows with validated consenting peer-source metadata.
5. Export local share-metadata bundles from verified `.slm` imports, including chunk hashes and Merkle roots.
6. Generate catalog entries from admitted manifests plus peer-offer proofs, rejecting drift before metadata is published.
7. Generate metadata-only catalog snapshots from verified catalog entries, rejecting byte-hosting claims and unsafe peer sources before publication.
8. Prototype peer transfer using the existing peer-piece sidecar, direct peer HTTP serve/fetch, and chunk/Merkle verification.
9. Design the optional MiniModel Peer Companion around shared Rust crates, local piece stores, peer identity, consent gates, cache controls, receipts, and TinyRustLM handoff.
10. Evaluate broader peer discovery and NAT traversal transports only after the companion preserves the verifier and no-server-byte boundary.

## Examples

- [catalog/models.v0.json](catalog/models.v0.json)
- [catalog/peer-source-announcements.v0.json](catalog/peer-source-announcements.v0.json)
- [examples/tiny-test-model-q8.minimodel.manifest.v0](examples/tiny-test-model-q8.minimodel.manifest.v0)
- [examples/minimodel.manifest.v0.example](examples/minimodel.manifest.v0.example)
- [examples/catalog-entry.v0.json](examples/catalog-entry.v0.json)
- [examples/share-metadata.v0.json](examples/share-metadata.v0.json)
