# MiniModel.org Roadmap

MiniModel.org is planned as a Rust-first peer-to-peer discovery and verification layer for tiny local AI model packages. It is not a model host and not an inference service.

## Core Rule

Project servers never run user models and never serve user model bytes. They may serve static documentation, signed catalog metadata, schemas, contact/takedown routes, and checksum-bearing manifest examples.

## Architecture

1. `minimodel-core`: Rust verification library for manifests, checksums, chunks, identities, and import receipts.
2. `minimodel` CLI: local verifier for `.slm` artifacts, license/model-card fields, evidence routes, one-command peer-share preparation after conversion, metadata-only catalog snapshot writing, direct known-peer piece serve/fetch, peer-piece export/reassembly, catalog-entry writing from manifest plus peer-offer proof, one-command TinyRustLM peer-import request execution, and TinyRustLM import readiness.
3. Static catalog: metadata-only listings and peer hints, with no model weights.
4. Peer app: later Rust desktop app for consent-based discovery, resumable transfer, NAT traversal, local quotas, and checksum validation.
5. TinyRustLM handoff: verified local `.slm` files open in the browser-local Rust/WASM runtime.

## Source Options

MiniModel.org should support three source lanes, all ending in local verification:

- User-owned local files and lists: users bring `.slm` files and optional local JSON metadata lists from their own machine.
- Hugging Face discovery: users may search or inspect external Hugging Face metadata, model cards, file hints, and source revisions, but project servers do not proxy `/resolve/` downloads and do not use project-owned tokens.
- MiniModel P2P: consenting peers exchange chunks that are verified by checksum and Merkle data before import. The first prototype is a direct `http://` peer-piece sender/receiver pair from a user-provided peer endpoint; discovery, identity exchange, relays, and NAT traversal remain later.

All source lanes must converge on a user-controlled local `.slm` artifact, a local verifier result or receipt, and a browser file-picker import into TinyRustLM. Hugging Face Inference Providers, Inference Endpoints, and executable Spaces are remote execution products, not MiniModel or TinyRustLM run paths.

The Hugging Face lane should produce local handoff files when no admitted `.slm` metadata exists yet. The conversion-request JSON identifies the upstream model id, preferred `.slm` quantization targets, required review/conversion/eval gates, and the rule that project servers do not serve the resulting bytes. TinyRustLM may also save a `.tinyrustlm-convert.ps1` script that runs the local Rust packer command `convert-huggingface-request` against a user-owned local `.safetensors` file, then runs MiniModel `prepare-peer-share` to write the local manifest, peer-offer proof, peer pieces, and catalog entry, followed by `write-catalog-snapshot` to produce metadata-only `models.v0.json`. The script does not download model bytes.

Once one user or converter completes the chain, TinyRustLM can export a `minimodel.local_slm_share_metadata.v0` bundle with artifact SHA-256, chunk hashes, and a Merkle root, and MiniModel can now run `prepare-peer-share` to produce the local manifest, deterministic `.minimodel-peer-offer` text manifest, peer-piece directory, and catalog entry in one output directory. If a consenting direct peer is already known, the same command can validate and publish `peer_source_count` / `peer_sources` metadata for that peer without making MiniModel.org a byte host. If peer availability becomes known later, `add-peer-source` can attach a validated consenting peer URL to the existing proof-ready catalog entry and write a new entry for the next metadata-only snapshot. `write-catalog-snapshot` then revalidates generated entries, rejects artifact/model/download URLs and project-server peer hosts, and writes a metadata-only catalog snapshot that TinyRustLM can later search. TinyRustLM treats proof-ready rows as a P2P import-request path: when a catalog peer source exists, it can save the request/script immediately; otherwise the receiver supplies one consenting peer URL first. Rust `import-peer-request` then validates the request against the manifest, fetches pieces, assembles the `.slm`, verifies it again, and writes a receipt in one output directory. The MiniModel CLI can already prepare peer shares, attach later peer-source metadata, write catalog snapshots, export, serve, fetch, assemble, reverify peer pieces, and execute TinyRustLM peer-import requests for known consenting peers; broader discovery and availability still remain later. That is the durable bridge toward users obtaining the admitted `.slm` through P2P instead of repeating a large source download and conversion.

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
9. Build a desktop peer app with discovery, identity exchange, consent, local cache controls, and TinyRustLM handoff.

## Examples

- [catalog/models.v0.json](catalog/models.v0.json)
- [examples/tiny-test-model-q8.minimodel.manifest.v0](examples/tiny-test-model-q8.minimodel.manifest.v0)
- [examples/minimodel.manifest.v0.example](examples/minimodel.manifest.v0.example)
- [examples/catalog-entry.v0.json](examples/catalog-entry.v0.json)
- [examples/share-metadata.v0.json](examples/share-metadata.v0.json)
