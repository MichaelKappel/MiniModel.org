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
- peer hint status, initially `disabled`, later `available` only after a converted `.slm` has admission evidence and peer-transfer metadata

A catalog entry must not include a project-server artifact URL. The user can download or receive bytes only through a future consent-based peer app or through their own local file flow.

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
7. MiniModel.org may list the resulting metadata with `source_huggingface_model_id`, artifact byte count, artifact SHA-256, manifest route, evidence routes, `peer_offer_route`, `peer_offer_sha256`, `peer_offer_merkle_root_sha256`, and `peer_hint_status`.
8. Future P2P transfer may use the metadata and chunk/Merkle fields so later users can obtain verified `.slm` bytes from consenting peers instead of repeating a large source download and conversion.

Until that chain exists for a model, the honest status is `Rust conversion required`.

## Local Share Metadata Bundle

TinyRustLM may export `minimodel.local_slm_share_metadata.v0` after a user imports a verified local `.slm`. This bundle is the first practical bridge from "converted once" to later P2P availability:

- `catalog_entry` records the artifact byte count, artifact SHA-256, local filename hint, source identity, optional `source_huggingface_model_id`, and no-server-byte flags.
- `peer_offer` records chunk size, chunk count, per-chunk SHA-256 values, chunk-list SHA-256, and a Merkle root.
- `boundary.peer_transfer_implemented` must be `false` until a real peer transfer implementation exists.
- `peer_offer.transfer.network_transfer_available` must be `false` until a consenting peer app can transfer and reverify chunks.

The Rust packer can also write `.minimodel-peer-offer` with `tinyrustlm-slm-pack peer-offer <input.slm> <output.peer-offer>` and validate it with `validate-peer-offer`. Catalog entries may reference that text manifest with `peer_offer_route`, `peer_offer_sha256`, `peer_offer_chunk_count`, and `peer_offer_merkle_root_sha256`, but a peer-offer route is not a model download route and does not mean project servers serve bytes.

The bundle may be submitted for review or used by future peer software, but it is not an endorsement and not an execution grant. TinyRustLM still runs only a user-selected local `.slm` after Rust/WASM validation.

## TinyRustLM UI Mapping

TinyRustLM should expose the same source lanes in plain language:

- `Your Local Files`: first-class runnable path through the browser file picker.
- `Hugging Face`: user-triggered metadata discovery, model-card review, source revision context, API endpoint awareness, conversion-request JSON, and optional local Rust conversion-script handoff only.
- `P2P Exchange`: MiniModel metadata and peer-offer proof for converted `.slm` records now, future peer-transfer flow later, still requiring local receipt and `.slm` verification before browser execution.

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

The first public deliverable is not a network. It is a stable documentation set:

- manifest schema
- verifier plan
- metadata-only catalog rules
- example manifest
- example catalog entry
- takedown/contact route language

Only after those pieces are stable should peer transfer be prototyped.
