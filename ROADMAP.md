# MiniModel.org Roadmap

MiniModel.org is planned as a Rust-first peer-to-peer discovery and verification layer for tiny local AI model packages. It is not a model host and not an inference service.

## Core Rule

Project servers never run user models and never serve user model bytes. They may serve static documentation, signed catalog metadata, schemas, contact/takedown routes, and checksum-bearing manifest examples.

## Architecture

1. `minimodel-core`: Rust verification library for manifests, checksums, chunks, identities, and import receipts.
2. `minimodel` CLI: local verifier for `.slm` artifacts, license/model-card fields, evidence routes, and SLMComposer import readiness.
3. Static catalog: metadata-only listings and peer hints, with no model weights.
4. Peer app: later Rust desktop app for consent-based discovery, resumable transfer, NAT traversal, local quotas, and checksum validation.
5. SLMComposer handoff: verified local `.slm` files open in the browser-local Rust/WASM runtime.

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

The first implementation target is a local Rust verifier, not a network. It parses the manifest, rejects server-byte and server-execution claims, verifies a user-supplied local `.slm` artifact, checks TinyRustLM compatibility fields, and writes a local import receipt.

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
4. Publish metadata-only static catalog prototypes.
5. Prototype peer transfer with chunk/Merkle verification.
6. Build a desktop peer app with consent, local cache controls, and SLMComposer handoff.

## Examples

- [examples/minimodel.manifest.v0.example](examples/minimodel.manifest.v0.example)
- [examples/catalog-entry.v0.json](examples/catalog-entry.v0.json)
