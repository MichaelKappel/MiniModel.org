# MiniModel.org Roadmap

MiniModel.org is planned as a Rust-first peer-to-peer discovery and verification layer for tiny local AI model packages. It is not a model host and not an inference service.

## Core Rule

Project servers never run user models and never serve user model bytes. They may serve static documentation, signed catalog metadata, schemas, contact/takedown routes, and checksum-bearing manifest examples.

## Architecture

1. `minimodel-core`: Rust verification library for manifests, checksums, chunks, identities, and import receipts.
2. `minimodel` CLI: local verifier for `.slm` artifacts, license/model-card fields, evidence routes, direct known-peer piece serve/fetch, peer-piece export/reassembly, catalog-entry writing from manifest plus peer-offer proof, and TinyRustLM import readiness.
3. Static catalog: metadata-only listings and peer hints, with no model weights.
4. Peer app: later Rust desktop app for consent-based discovery, resumable transfer, NAT traversal, local quotas, and checksum validation.
5. TinyRustLM handoff: verified local `.slm` files open in the browser-local Rust/WASM runtime.

## Source Options

MiniModel.org should support three source lanes, all ending in local verification:

- User-owned local files and lists: users bring `.slm` files and optional local JSON metadata lists from their own machine.
- Hugging Face discovery: users may search or inspect external Hugging Face metadata, model cards, file hints, and source revisions, but project servers do not proxy `/resolve/` downloads and do not use project-owned tokens.
- MiniModel P2P: consenting peers exchange chunks that are verified by checksum and Merkle data before import. The first prototype is a direct `http://` peer-piece sender/receiver pair from a user-provided peer endpoint; discovery, identity exchange, relays, and NAT traversal remain later.

All source lanes must converge on a user-controlled local `.slm` artifact, a local verifier result or receipt, and a browser file-picker import into TinyRustLM. Hugging Face Inference Providers, Inference Endpoints, and executable Spaces are remote execution products, not MiniModel or TinyRustLM run paths.

The Hugging Face lane should produce local handoff files when no admitted `.slm` metadata exists yet. The conversion-request JSON identifies the upstream model id, preferred `.slm` quantization targets, required review/conversion/eval gates, and the rule that project servers do not serve the resulting bytes. TinyRustLM may also save a `.tinyrustlm-convert.ps1` script that runs the local Rust packer commands for `inspect-safetensors`, `plan-safetensors-source`, `convert-safetensors-trained`, `validate`, `validate-manifest`, `peer-offer`, and `validate-peer-offer` against user-owned local files. The script does not download model bytes.

Once one user or converter completes the chain, TinyRustLM can export a `minimodel.local_slm_share_metadata.v0` bundle with artifact SHA-256, chunk hashes, and a Merkle root, and the Rust packer can write a deterministic `.minimodel-peer-offer` text manifest with the same artifact/chunk proof. MiniModel can then run `write-catalog-entry` to validate the admitted manifest plus peer-offer proof and write metadata with `peer_hint_status=peer-offer-ready-known-peer-needed` and `peer_import_status=request-script-ready`. TinyRustLM treats that as a P2P import-request path: the receiver still supplies a consenting peer URL, Rust fetches pieces, assembles the `.slm`, verifies it again, and writes a receipt. The MiniModel CLI can already export, serve, fetch, assemble, and reverify peer pieces for known consenting peers; broader discovery and availability still remain later. That is the durable bridge toward users obtaining the admitted `.slm` through P2P instead of repeating a large source download and conversion.

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

The first implementation target is a local Rust verifier plus the smallest possible direct peer-piece transfer prototype. The initial `minimodel-core` and `minimodel` crates now parse manifests, reject server-byte and server-execution claims, verify a user-supplied local `.slm` artifact, check TinyRustLM compatibility fields, validate chunk/Merkle metadata when present, write catalog entries only after manifest/peer-offer drift checks pass, export local peer pieces, serve those pieces from a user-started direct HTTP peer endpoint, fetch peer pieces from a consenting direct HTTP peer endpoint, reassemble those pieces with full re-verification, and write a local import receipt.

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
5. Export local share-metadata bundles from verified `.slm` imports, including chunk hashes and Merkle roots.
6. Generate catalog entries from admitted manifests plus peer-offer proofs, rejecting drift before metadata is published.
7. Prototype peer transfer using the existing peer-piece sidecar, direct peer HTTP serve/fetch, and chunk/Merkle verification.
8. Build a desktop peer app with discovery, identity exchange, consent, local cache controls, and TinyRustLM handoff.

## Examples

- [catalog/models.v0.json](catalog/models.v0.json)
- [examples/tiny-test-model-q8.minimodel.manifest.v0](examples/tiny-test-model-q8.minimodel.manifest.v0)
- [examples/minimodel.manifest.v0.example](examples/minimodel.manifest.v0.example)
- [examples/catalog-entry.v0.json](examples/catalog-entry.v0.json)
- [examples/share-metadata.v0.json](examples/share-metadata.v0.json)
