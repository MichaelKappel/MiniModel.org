# MiniModel Rust Verifier MVP

The first implementation target is a local Rust verifier. It should prove manifest and `.slm` bytes before any peer networking is attempted.

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
```

`verify-manifest` checks schema, required fields, boundary fields, license/model-card routes, route shape, and signature placeholder rules.

`verify-artifact` additionally checks local file byte count, SHA256, chunk metadata when present, and TinyRustLM `.slm` compatibility.

`write-import-receipt` only succeeds after `verify-artifact` passes.

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
| `import.next_gate` | `user-selected-slmcomposer-local-file-open`. |

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
6. Receipt:
   - write a local receipt beside the artifact or to a user-selected path
   - include manifest checksum and artifact checksum
   - set the next gate to SLMComposer local file open

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

## SLMComposer Handoff

The verifier does not push files into SLMComposer. It produces a receipt and tells the user which local `.slm` file passed. SLMComposer continues to load only files selected through the browser file picker, and Rust/WASM remains the final `.slm` validation authority.

## Later Networking Boundary

Peer transfer can reuse the same manifest and receipt fields, but it must not change the local verification contract. A future peer app may fetch chunks from consenting peers; MiniModel.org servers still publish only metadata, schemas, public key routes, and takedown/contact routes.
