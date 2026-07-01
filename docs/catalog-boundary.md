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
- automatic SLMComposer fetch instructions for catalog entries

## Catalog Entry Shape

A catalog entry may include:

- catalog schema version
- model id and display name
- publisher id and key id
- manifest metadata route
- model-card route
- license route
- source identity summary
- artifact byte count and checksum
- quantization, model shape, and tokenizer checksum
- evidence status summaries
- takedown/contact route
- peer hint status, initially `disabled`

A catalog entry must not include a project-server artifact URL. The user can download or receive bytes only through a future consent-based peer app or through their own local file flow.

## User Consent Gates

The product flow needs two separate consent moments:

1. Transfer consent: the user chooses to obtain bytes from a peer or from a source outside MiniModel.org.
2. Execution consent: the user chooses to open a verified local `.slm` file in SLMComposer.

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
