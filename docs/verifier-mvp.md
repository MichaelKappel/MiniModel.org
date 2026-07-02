# MiniModel Rust Verifier MVP

The first implementation target is a local Rust verifier. It proves manifest and `.slm` bytes before any peer networking is attempted.

## Current Implementation

MiniModel.org now contains the first standard-library Rust verifier workspace:

- `crates/minimodel-core`: parser, boundary checks, SHA256, chunk/Merkle verification, TinyRustLM `.slm` structural validation, peer-piece export/fetch/reassembly, and receipt writing.
- `crates/minimodel`: CLI wrapper for local developer use.
- `examples/tiny-test-model-q8.minimodel.manifest.v0`: checksum-bound manifest for the TinyRustLM q8 fixture.

The verifier does not download model bytes. `verify-artifact` only opens the artifact path supplied by the user.

## Crates And Binaries

| Component | Role |
| --- | --- |
| `minimodel-core` | Parser, normalized manifest model, checksum routines, artifact checks, chunk checks, evidence checks, and receipt writer. |
| `minimodel` | CLI wrapper around `minimodel-core`. |
| `minimodel-tests` | Fixture manifests, drift cases, and receipt round trips. |

The first pass should prefer standard-library Rust and line-oriented parsing. Networking, DHTs, relays, desktop UI, and peer identity exchange are later phases.

## Commands

```text
minimodel verify-manifest <manifest>
minimodel verify-artifact <manifest> <artifact.slm>
minimodel write-import-receipt <manifest> <artifact.slm> <receipt.minimodel-receipt>
minimodel export-peer-pieces <manifest> <artifact.slm> <output-dir>
minimodel serve-peer-pieces <manifest> <piece-dir> <bind-addr>
minimodel fetch-peer-pieces <manifest> <http-peer-base-url> <output-dir>
minimodel assemble-peer-pieces <manifest> <piece-dir> <output.slm>
```

`verify-manifest` checks schema, required fields, boundary fields, license/model-card routes, route shape, and signature placeholder rules.

`verify-artifact` additionally checks local file byte count, SHA256, chunk metadata when present, and TinyRustLM `.slm` compatibility.

`write-import-receipt` only succeeds after `verify-artifact` passes.

`export-peer-pieces` only succeeds after `verify-artifact` passes. It writes local piece files plus `minimodel.peer_piece_set.v0`, a metadata sidecar that binds the manifest checksum, artifact checksum, chunk count, chunk SHA256 rows, chunk-list SHA256, and Merkle root. The piece files are model bytes and must stay outside project servers.

`serve-peer-pieces` validates an existing peer-piece directory before binding a local HTTP listener. It serves only `minimodel.peer_piece_set.v0` and the exact piece file names declared in that sidecar. It does not publish to MiniModel.org, does not discover peers, and stops when the user stops the process.

`fetch-peer-pieces` is the first live transfer-shaped prototype. It pulls `minimodel.peer_piece_set.v0` plus each listed chunk from a user-provided `http://` peer base URL, rejects project-server hostnames, verifies the piece-set against the manifest, verifies every downloaded piece SHA256 before writing it, and leaves final artifact acceptance to `assemble-peer-pieces`.

`assemble-peer-pieces` reads a local `minimodel.peer_piece_set.v0`, verifies each local piece, assembles a temporary `.slm`, runs the full artifact verifier again, and moves the output into place only after verification passes. This is the first transport-shaped primitive; it is not live peer networking.

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
7. Receipt:
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
```

The receipt stores the artifact basename only. It must not store private absolute paths, public model-byte URLs, or endorsement claims.

## Later Networking Boundary

Peer transfer can reuse the same manifest, peer-piece, direct-HTTP serve/fetch, and receipt fields, but it must not change the local verification contract. The first `serve-peer-pieces` and `fetch-peer-pieces` commands are intentionally narrow: they are for known consenting peers or local test endpoints, not MiniModel.org hosting. A future peer app may add identity exchange, peer discovery, relays, NAT traversal, resumable transfer, and cache policy; MiniModel.org servers still publish only metadata, schemas, public key routes, and takedown/contact routes.
