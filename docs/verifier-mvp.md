# MiniModel Rust Verifier MVP

The first implementation target is a local Rust verifier. It proves manifest and `.slm` bytes before any peer networking is attempted.

## Current Implementation

MiniModel.org now contains the first Rust verifier workspace:

- `crates/minimodel-core`: line-oriented manifest parser, `serde_json` parser for TinyRustLM peer-import request JSON, boundary checks, SHA256, chunk/Merkle verification, TinyRustLM `.slm` structural validation, peer-piece export/fetch/reassembly, and receipt writing.
- `crates/minimodel`: CLI wrapper for local developer use.
- `examples/tiny-test-model-q8.minimodel.manifest.v0`: checksum-bound manifest for the TinyRustLM q8 fixture.
- `examples/catalog-entry.v0.json`: catalog metadata generated from the fixture manifest plus `.minimodel-peer-offer`, not hand-authored placeholder checksums.

The verifier does not download model bytes. `verify-artifact` only opens the artifact path supplied by the user.

## Crates And Binaries

| Component | Role |
| --- | --- |
| `minimodel-core` | Parser, normalized manifest model, checksum routines, artifact checks, chunk checks, evidence checks, and receipt writer. |
| `minimodel` | CLI wrapper around `minimodel-core`. |
| `minimodel-core` unit tests | Fixture manifests, drift cases, request parser failures, peer host rejection, HTTP piece transfer, and receipt round trips. |

The first pass keeps manifests and peer-piece sidecars line-oriented where possible. Structured browser handoff JSON is parsed as JSON with `serde_json` so the receiver does not rely on string scanning. Networking, DHTs, relays, desktop UI, and peer identity exchange are later phases.

## Commands

```text
minimodel verify-manifest <manifest>
minimodel verify-artifact <manifest> <artifact.slm>
minimodel write-import-receipt <manifest> <artifact.slm> <receipt.minimodel-receipt>
minimodel export-peer-pieces <manifest> <artifact.slm> <output-dir>
minimodel serve-peer-pieces <manifest> <piece-dir> <bind-addr>
minimodel fetch-peer-pieces <manifest> <http-peer-base-url> <output-dir>
minimodel assemble-peer-pieces <manifest> <piece-dir> <output.slm>
minimodel import-peer-request <request-json> <manifest> <output-dir>
minimodel prepare-peer-share <artifact.slm> <output-dir> <model-id> [source-huggingface-model-id] [known-peer-base-url]
minimodel write-catalog-entry <manifest> <peer-offer> <output-json> [source-huggingface-model-id] [known-peer-base-url]
minimodel add-peer-source <catalog-entry-json> <output-json> <known-peer-base-url>
minimodel write-peer-source-announcement <catalog-entry-json> <announcement-json> <known-peer-base-url>
minimodel apply-peer-source-announcements <catalog-entry-json> <output-json> <announcement-json>...
minimodel write-catalog-snapshot <output-json> <catalog-entry-json>...
```

`verify-manifest` checks schema, required fields, boundary fields, license/model-card routes, route shape, and signature placeholder rules.

`verify-artifact` additionally checks local file byte count, SHA256, chunk metadata when present, and TinyRustLM `.slm` compatibility.

`write-import-receipt` only succeeds after `verify-artifact` passes.

`export-peer-pieces` only succeeds after `verify-artifact` passes. It writes local piece files plus `minimodel.peer_piece_set.v0`, a metadata sidecar that binds the manifest checksum, artifact checksum, chunk count, chunk SHA256 rows, chunk-list SHA256, and Merkle root. The piece files are model bytes and must stay outside project servers.

`serve-peer-pieces` validates an existing peer-piece directory before binding a local HTTP listener. It serves only `minimodel.peer_piece_set.v0` and the exact piece file names declared in that sidecar. It does not publish to MiniModel.org, does not discover peers, and stops when the user stops the process.

`fetch-peer-pieces` is the first live transfer-shaped prototype. It pulls `minimodel.peer_piece_set.v0` plus each listed chunk from a user-provided `http://` peer base URL, rejects project-server hostnames, verifies the piece-set against the manifest, verifies every downloaded piece SHA256 before writing it, and leaves final artifact acceptance to `assemble-peer-pieces`.

`assemble-peer-pieces` reads a local `minimodel.peer_piece_set.v0`, verifies each local piece, assembles a temporary `.slm`, runs the full artifact verifier again, and moves the output into place only after verification passes. This is the first transport-shaped primitive; it is not live peer networking.

`import-peer-request` is the receiver-side one-command path for a TinyRustLM `minimodel.peer_import_request.v0` file. It parses the request, rejects project-server or Hugging Face byte-source claims, checks the request's model id, byte count, artifact SHA-256, safe local output names, and known peer URL against the supplied manifest, then runs the same fetch, assemble, full artifact verification, and receipt-writing chain into the chosen output directory. This is the first "select a proof-ready model, enter peer URL, run one Rust verifier command" path; it still requires a consenting direct peer and never fetches model bytes from MiniModel.org.

`prepare-peer-share` is the sender-side one-command path after a user or converter has a local `.slm`. It validates the `.slm`, writes a MiniModel manifest, writes a `.minimodel-peer-offer` proof, exports `minimodel.peer_piece_set.v0` plus local piece files, and writes a catalog-entry JSON in one output directory. If `[known-peer-base-url]` is supplied, the generated catalog entry includes validated `peer_source_count` / `peer_sources` metadata; otherwise it is proof-ready and still requires a receiver to set one consenting peer URL. The command does not download from Hugging Face, does not upload model bytes, and does not make project servers serve or run the artifact.

`write-catalog-entry` validates a MiniModel manifest and `.minimodel-peer-offer` proof against each other before writing JSON catalog metadata. It rejects artifact, SLM field, chunk count, chunk-list, Merkle root, evidence checksum, and per-chunk drift. The generated entry may advertise `peer_hint_status=peer-offer-ready-known-peer-needed` and `peer_import_status=request-script-ready`, which means TinyRustLM can save a peer-import request/script, not that MiniModel.org serves bytes. When `[known-peer-base-url]` is provided, it also validates a direct `http://` consenting peer URL, rejects project-server hosts, and writes `peer_source_count` / `peer_sources` metadata so the browser can offer `P2P Import` without a separate manual peer URL step.

`add-peer-source` is the late availability step for a proof-ready catalog entry. It reads an existing catalog-entry JSON, validates the entry, rejects project-server peer hosts, writes exactly one `known-direct-http-peer-v0` row for the provided consenting direct peer URL, revalidates `peer_source_count`, and writes a new catalog-entry JSON. This lets a converter produce proof metadata first, then publish P2P availability later without rerunning Hugging Face conversion or uploading `.slm` bytes.

`write-peer-source-announcement` is the portable availability step. It reads a validated catalog-entry JSON plus a consenting direct peer URL and writes `minimodel.peer_source_announcement` JSON. The announcement binds model id, artifact byte count, artifact SHA256, peer-offer SHA256, peer-offer Merkle root, the catalog entry checksum, and one validated `known-direct-http-peer-v0` row. It is metadata only and still does not upload, proxy, fetch, or serve model bytes.

`apply-peer-source-announcements` is the catalog-maintainer merge step. It validates a proof-ready catalog entry, validates one or more peer-source announcements, rejects model/artifact/peer-offer drift, deduplicates peer URLs, writes `peer_sources`, revalidates `peer_source_count`, and produces a new catalog-entry JSON for the next `write-catalog-snapshot` run. This is the first durable "converted once, later users see P2P Import by default" publication path while keeping project servers out of the byte path.

`write-catalog-snapshot` validates one or more catalog-entry JSON files and writes a metadata-only `minimodel.model_catalog` snapshot. It rejects duplicate `model_id` values, artifact/model/download URLs, project-server byte/execution claims, peer-source count drift, unsupported peer-source rows, and project-server peer hosts. This is the first Rust-owned publication step for "converted once, listed as P2P-ready later"; the snapshot can be loaded by TinyRustLM as metadata, while model bytes still come only from local files or consenting peers.

## Receipt Fields

The local receipt is also line-oriented:

| Field | Meaning |
| --- | --- |
| `receipt.version` | Receipt schema version. |
| `receipt.kind` | `minimodel.import_receipt`. |
| `receipt.created_utc` | Local verification time. |
| `manifest.sha256` | SHA256 of the manifest file. |
| `manifest.model_id` | Copied model id. |
| `artifact.local_path_hint` | Basename only, not a public path. |
| `artifact.byte_count` | Verified local artifact bytes. |
| `artifact.sha256` | Verified local artifact SHA256. |
| `slm.validation_status` | `passed` or rejected before receipt. |
| `slm.model_shape` | Verified TinyRustLM shape. |
| `slm.quantization` | Verified quantization. |
| `evidence.status` | Summary of available evidence sidecars. |
| `import.next_gate` | `user-selected-tinyrustlm-local-file-open`. |

Receipts must not contain model bytes, public model-byte URLs, or claims that MiniModel.org endorses the artifact.

## Verification Stages

1. Manifest parse:
   - reject duplicate keys
   - reject comments in signed manifests
   - reject unknown required field versions
   - reject non-ASCII keys
2. Boundary parse:
   - require `artifact.project_server_url=none`
   - require `artifact.acquisition=user-local-file`, `user-external-download`, or `consent-peer-transfer`
   - reject remote execution, install hooks, script hooks, or automatic browser-fetch fields
3. Metadata parse:
   - require model id, publisher id, license route, model-card route, artifact kind, byte count, checksum, shape, tokenizer checksum, tensor-layout checksum, runtime compatibility, and evidence admission status
4. Local artifact verification:
   - open only the user-supplied local path
   - verify byte count and SHA256
   - verify chunk list and Merkle root when present
   - verify TinyRustLM `.slm` header and checksum
   - verify tokenizer checksum and tensor-layout checksum against manifest fields
5. Evidence verification:
   - verify sidecar route/checksum pairs when local evidence files are supplied
   - reject `passed` evidence status without a checksum-bearing sidecar route
6. Peer-piece export, fetch, and assembly:
   - export fixed-size local piece files from a verified `.slm`
   - write `minimodel.peer_piece_set.v0` beside those local pieces
   - serve only the declared sidecar and piece files from a user-started local HTTP peer
   - fetch a piece-set and chunk files from a consenting direct HTTP peer endpoint
   - reject MiniModel.org, TinyRustLM.com, SLMComposer.com, MiRust.com, and related project hostnames as model-byte peers
   - reject arbitrary unlisted files in the peer-piece directory
   - reject modified piece bytes before assembly
   - re-run full artifact verification before the assembled `.slm` is accepted
7. Catalog-entry writing:
   - validate `.minimodel-peer-offer` against the manifest
   - reject mismatched artifact, SLM, chunk-list, Merkle, admission-checksum, or per-chunk fields
   - reject known peer source URLs that point at project servers
   - write metadata-only JSON with no project-server artifact URL
   - mark proof-ready entries as import-request ready, optionally with catalog-listed consenting peer source metadata, not server-hosted bytes
8. Receipt:
   - write a local receipt beside the artifact or to a user-selected path
   - include manifest checksum and artifact checksum
   - set the next gate to TinyRustLM local file open

## Drift Tests

The MVP test suite should include one passing fixture and at least these failures:

- missing license route
- missing model-card route
- project-server model-byte URL present
- server-side execution claim present
- artifact byte-count drift
- artifact SHA256 drift
- tokenizer checksum drift
- tensor-layout checksum drift
- unsupported artifact kind
- unsupported quantization
- duplicate key
- route without checksum for passed evidence
- checksum without route for passed evidence
- project-owned Hugging Face token requirement
- Hugging Face `/resolve/` URL represented as a project-server artifact URL
- remote inference requirement from Hugging Face Inference Providers, Inference Endpoints, or Spaces
- chunk count drift
- Merkle root drift
- signature payload drift when signatures are enabled
- peer-offer artifact or chunk drift before catalog-entry writing

## TinyRustLM Handoff

The verifier does not push files into TinyRustLM. It produces a receipt and tells the user which local `.slm` file passed. TinyRustLM continues to load only files selected through the browser file picker, and Rust/WASM remains the final `.slm` validation authority.

## Developer Commands

From the MiniModel.org workspace:

```powershell
cargo run -p minimodel -- verify-manifest examples\tiny-test-model-q8.minimodel.manifest.v0
cargo run -p minimodel -- verify-artifact examples\tiny-test-model-q8.minimodel.manifest.v0 ..\SLMComposer.com\tinyrustlm\models\tiny-test-model-q8.slm
cargo run -p minimodel -- write-import-receipt examples\tiny-test-model-q8.minimodel.manifest.v0 ..\SLMComposer.com\tinyrustlm\models\tiny-test-model-q8.slm .\tiny-test-model-q8.minimodel-receipt
cargo run -p minimodel -- export-peer-pieces examples\tiny-test-model-q8.minimodel.manifest.v0 ..\SLMComposer.com\tinyrustlm\models\tiny-test-model-q8.slm .\peer-pieces\tiny-test-model-q8
cargo run -p minimodel -- serve-peer-pieces examples\tiny-test-model-q8.minimodel.manifest.v0 .\peer-pieces\tiny-test-model-q8 127.0.0.1:8123
cargo run -p minimodel -- fetch-peer-pieces examples\tiny-test-model-q8.minimodel.manifest.v0 http://127.0.0.1:8123/ .\peer-pieces-fetched\tiny-test-model-q8
cargo run -p minimodel -- assemble-peer-pieces examples\tiny-test-model-q8.minimodel.manifest.v0 .\peer-pieces\tiny-test-model-q8 .\tiny-test-model-q8.from-pieces.slm
cargo run -p minimodel -- import-peer-request .\tiny-test-model-q8.minimodel-peer-import-request.json examples\tiny-test-model-q8.minimodel.manifest.v0 .\tiny-test-model-q8-peer-import
cargo run -p minimodel -- prepare-peer-share ..\SLMComposer.com\tinyrustlm\models\tiny-test-model-q8.slm .\prepared-share\tiny-test-model-q8 tinyrustlm/prepared-tiny-test-model-q8 tinyrustlm/tiny-test-model-q8 http://127.0.0.1:8123/
cargo run -p minimodel -- write-catalog-entry examples\tiny-test-model-q8.minimodel.manifest.v0 examples\tiny-test-model-q8.minimodel-peer-offer examples\catalog-entry.v0.json
cargo run -p minimodel -- write-catalog-entry examples\tiny-test-model-q8.minimodel.manifest.v0 examples\tiny-test-model-q8.minimodel-peer-offer examples\catalog-entry.with-peer.v0.json tinyrustlm/tiny-test-model-q8 http://127.0.0.1:8123/
cargo run -p minimodel -- add-peer-source examples\catalog-entry.v0.json examples\catalog-entry.later-peer.v0.json http://127.0.0.1:8123/
cargo run -p minimodel -- write-peer-source-announcement examples\catalog-entry.v0.json examples\catalog-entry.peer-source-announcement.v0.json http://127.0.0.1:8123/
cargo run -p minimodel -- apply-peer-source-announcements examples\catalog-entry.v0.json examples\catalog-entry.announced-peer.v0.json examples\catalog-entry.peer-source-announcement.v0.json
cargo run -p minimodel -- write-catalog-snapshot .\models.v0.json examples\catalog-entry.v0.json
```

The receipt stores the artifact basename only. It must not store private absolute paths, public model-byte URLs, or endorsement claims.

## Later Networking Boundary

Peer transfer can reuse the same manifest, peer-piece, direct-HTTP serve/fetch, and receipt fields, but it must not change the local verification contract. The first `serve-peer-pieces` and `fetch-peer-pieces` commands are intentionally narrow: they are for known consenting peers or local test endpoints, not MiniModel.org hosting. A future peer app may add identity exchange, peer discovery, relays, NAT traversal, resumable transfer, and cache policy; MiniModel.org servers still publish only metadata, schemas, public key routes, and takedown/contact routes.
