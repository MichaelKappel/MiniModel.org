# MiniModel.org Catalog Boundary

MiniModel.org starts as a static metadata catalog and schema documentation site. It is not a model host, inference host, or model approval authority.

## Server-Allow List

Project servers may serve:

- static documentation
- manifest schema docs
- example manifests with placeholder checksums
- metadata-only catalog entries
- signed catalog snapshots
- public key routes
- model-card and license routes when they are metadata only
- takedown and contact routes

Project servers must not serve:

- user model bytes
- `.slm` artifacts from third parties
- safetensors, GGUF, ONNX, or other model-weight files from users
- server-side inference endpoints for user models
- proxy download URLs that turn MiniModel.org into a model-byte relay
- automatic TinyRustLM fetch instructions for catalog entries

## Catalog Entry Shape

A catalog entry may include:

- catalog schema version
- model id and display name
- publisher id and key id
- manifest metadata route
- model-card route
- license route
- source identity summary
- upstream Hugging Face source id when the `.slm` came from a reviewed external checkpoint
- artifact byte count and checksum
- quantization, model shape, and tokenizer checksum
- evidence status summaries
- takedown/contact route
- peer hint status, initially `disabled`; `peer-offer-ready-known-peer-needed` is allowed after a converted `.slm` has admission evidence and a validated peer-offer proof
- peer import status such as `request-script-ready`, meaning a receiver can save a local request and Rust reverify script after choosing a consenting peer URL
- optional `peer_source_count` plus `peer_sources` metadata for already-known consenting peers. The first prototype supports `known-direct-http-peer-v0` sources with `direct-http-piece-server-v0`, a direct `http://` peer base URL, `publisher-declared-consenting-peer`, and explicit `project_server_serves_model_bytes=false` / `project_server_runs_model_bytes=false` boundary flags

A catalog entry must not include a project-server artifact URL. A catalog-listed peer source is still metadata, not hosted model bytes. The user can receive bytes only from their own local file flow or from a consenting peer endpoint that the Rust verifier fetches, assembles, and rechecks locally.

## Source Lanes

MiniModel.org and TinyRustLM should keep three source lanes distinct:

- `local-file` or `local-list`: user-owned local `.slm` files and local JSON metadata lists.
- `huggingface-hub`: user-triggered external discovery using Hugging Face metadata, model cards, file hints, and source revision context.
- `minimodel-p2p`: future consent-based peer transfer with manifest, chunk, Merkle, and checksum verification.

Hugging Face `/resolve/` URLs, LFS/Xet/CAS transfers, Inference Providers, Inference Endpoints, and executable Spaces must not be represented as MiniModel.org-hosted bytes or MiniModel.org-run inference. Project servers do not proxy Hugging Face downloads. If a user obtains bytes from Hugging Face or another external source, they convert or import them locally and then select the verified `.slm` file in TinyRustLM.

## Hugging Face To MiniModel Flow

Hugging Face search results are source records, not runnable TinyRustLM model entries. A result becomes useful to TinyRustLM only after the conversion chain produces a checksum-bound `.slm` artifact:

1. A user or converter worker selects a Hugging Face model id and saves local handoff files.
2. The conversion-request JSON records source identity, target quantization, evidence gates, and no-server-byte boundaries.
3. The optional `.tinyrustlm-convert.ps1` script runs local Rust packer commands: `inspect-safetensors`, `plan-safetensors-source`, `convert-safetensors-trained`, `validate`, `validate-manifest`, `peer-offer`, and `validate-peer-offer`.
4. Conversion happens outside project servers, using reviewed source bytes, config, tokenizer, license, and model-card metadata. The generated script does not download bytes and does not call a project proxy.
5. The converter writes a local `.slm`, validates it with the Rust packer/verifier, and records runtime-smoke plus eval/admission evidence.
6. The converter writes a `.minimodel-peer-offer` text manifest that binds the verified `.slm` to SHA-256 artifact identity, fixed-size chunk SHA-256 rows, a chunk-list SHA-256, and a Merkle root.
7. MiniModel validates the manifest plus `.minimodel-peer-offer` with `minimodel write-catalog-entry` before writing catalog JSON.
8. MiniModel.org may list the resulting metadata with `source_huggingface_model_id`, artifact byte count, artifact SHA-256, manifest route, evidence routes, `peer_offer_route`, `peer_offer_sha256`, `peer_offer_merkle_root_sha256`, `peer_hint_status`, `peer_import_status`, and optional `peer_source_count` / `peer_sources` when a consenting direct peer is already known.
9. Future P2P transfer may use the metadata and chunk/Merkle fields so later users can obtain verified `.slm` bytes from consenting peers instead of repeating a large source download and conversion.

Until that chain exists for a model, the honest status is `Rust conversion required`.

## Local Share Metadata Bundle

TinyRustLM may export `minimodel.local_slm_share_metadata.v0` after a user imports a verified local `.slm`. This bundle is the first practical bridge from "converted once" to later P2P availability:

- `catalog_entry` records the artifact byte count, artifact SHA-256, local filename hint, source identity, optional `source_huggingface_model_id`, and no-server-byte flags.
- `peer_offer` records chunk size, chunk count, per-chunk SHA-256 values, chunk-list SHA-256, and a Merkle root.
- `boundary.peer_transfer_implemented` must be `false` until a real peer transfer implementation exists.
- `peer_offer.transfer.network_transfer_available` must be `false` until a consenting peer app can transfer and reverify chunks.

The Rust packer can also write `.minimodel-peer-offer` with `tinyrustlm-slm-pack peer-offer <input.slm> <output.peer-offer>` and validate it with `validate-peer-offer`. Catalog entries may reference that text manifest with `peer_offer_route`, `peer_offer_sha256`, `peer_offer_chunk_count`, and `peer_offer_merkle_root_sha256`, but a peer-offer route is not a model download route and does not mean project servers serve bytes.

MiniModel can turn an admitted manifest plus peer-offer proof into metadata with:

```text
minimodel write-catalog-entry <manifest> <peer-offer> <output-json> [source-huggingface-model-id] [known-peer-base-url]
```

The command rejects drift between the manifest and proof before writing JSON. Its output may set `peer_hint_status` to `peer-offer-ready-known-peer-needed` and `peer_import_status` to `request-script-ready`. Without a known peer URL, that status means TinyRustLM can offer a P2P import request/script after the user enters one consenting peer URL. With `[known-peer-base-url]`, the command validates the peer URL, rejects project-server hosts, and writes `peer_source_count=1` plus a `peer_sources` row so TinyRustLM can show `P2P Import` immediately while still saving a Rust reverify request/script instead of fetching bytes in the browser.

The MiniModel verifier can now export local peer pieces after verification:

```text
minimodel export-peer-pieces <manifest> <artifact.slm> <output-dir>
minimodel serve-peer-pieces <manifest> <piece-dir> <bind-addr>
minimodel fetch-peer-pieces <manifest> <http-peer-base-url> <output-dir>
minimodel assemble-peer-pieces <manifest> <piece-dir> <output.slm>
minimodel import-peer-request <request-json> <manifest> <output-dir>
```

This creates, serves, fetches, and consumes a local `minimodel.peer_piece_set.v0` sidecar plus chunk files. `serve-peer-pieces` is sender-side only: it validates the local peer-piece directory and serves only the declared sidecar plus declared piece files from a user-started HTTP listener. `fetch-peer-pieces` is receiver-side: it downloads the sidecar and listed chunks from a user-provided `http://` peer endpoint, rejects project-server hostnames, and verifies every piece checksum before writing the local output directory. The sidecar proves what a peer transport should carry, and `assemble-peer-pieces` re-runs full Rust artifact verification before writing the accepted `.slm`. `import-peer-request` is the simplified receiver command for TinyRustLM handoff files: it validates the request against the manifest, fetches from the known peer URL, assembles, verifies, and writes the import receipt into one local output directory. The chunk files are model bytes and must not be uploaded to MiniModel.org.

The bundle may be submitted for review or used by future peer software, but it is not an endorsement and not an execution grant. TinyRustLM still runs only a user-selected local `.slm` after Rust/WASM validation.

## TinyRustLM UI Mapping

TinyRustLM should expose the same source lanes in plain language:

- `Your Local Files`: first-class runnable path through the browser file picker.
- `Hugging Face`: user-triggered metadata discovery, model-card review, source revision context, API endpoint awareness, conversion-request JSON, and optional local Rust conversion-script handoff only.
- `P2P Exchange`: MiniModel metadata, peer-offer proof, optional catalog-listed consenting peer sources, direct known-peer serve/fetch, local peer-piece export/reassembly, one-command `import-peer-request` receiver execution, and catalog entries generated by `write-catalog-entry` for converted `.slm` records now; broader peer discovery and relay flow later, still requiring local receipt and `.slm` verification before browser execution.

Advanced source details may show catalog URLs, Hugging Face search fields, checksums, manifests, evidence routes, and verifier status, but the first-load Chat mode should keep those behind progressive disclosure.

## User Consent Gates

The product flow needs two separate consent moments:

1. Transfer consent: the user chooses to obtain bytes from a peer or from a source outside MiniModel.org.
2. Execution consent: the user chooses to open a verified local `.slm` file in TinyRustLM.

The metadata catalog must not collapse either step into an automatic action.

## Legal And Liability Posture

Catalog language should say:

- listings are user or publisher assertions
- MiniModel.org does not claim listed models are safe, lawful, cleanly trained, or fit for purpose
- license and model-card fields are metadata, not legal advice
- disputed metadata can be reported through a takedown/contact route
- illegal-content references, malware claims, deceptive metadata, and spam can be removed from catalog metadata

Legal review is required before public exchange features, public submissions, or moderation workflows.

## Ecosystem Tradeoffs

The first implementation remains Rust-first:

- Rust-native verified transfer is the preferred later peer-transfer path.
- Rust-libp2p is a research lane for wider protocol reach.
- BitTorrent-style chunking is a useful swarm model, but MiniModel manifests own provenance and license binding.
- Pear, Bare, and Hypercore are useful zero-infrastructure references, but they are not core dependencies for the MiniModel verifier.
- Rust Hyper is ordinary HTTP infrastructure and should not be confused with Hypercore or cloud Hyper products.

## First Public Deliverable

The first public deliverable is not a hosted model network. It is a stable documentation and local-verifier set:

- manifest schema
- verifier plan
- local peer-piece export, direct peer serve/fetch, one-command import request execution, and reassembly
- metadata-only catalog rules
- example manifest
- example catalog entry
- takedown/contact route language

Only after those pieces are stable should broader peer discovery, identity exchange, relay, and cache behavior be prototyped.
