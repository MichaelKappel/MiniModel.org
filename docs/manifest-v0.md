# MiniModel Manifest v0 Draft

This draft defines the first implementable `minimodel.manifest` shape. It is designed for a small Rust parser, local artifact verification, and later peer transfer. It is not a server fetch contract.

## Boundary

- MiniModel.org servers may publish this schema, example manifests, signed catalog metadata, public key routes, and contact routes.
- MiniModel.org servers must not host user model bytes or proxy model-byte downloads.
- SLMComposer remains browser-local. Users select local `.slm` files; Rust/WASM validates the bytes.
- The first verifier accepts a manifest file plus a user-provided local artifact path.

## Format

The MVP manifest is a UTF-8 text file with one `key=value` field per line.

- Lines are LF or CRLF terminated.
- Empty lines are ignored.
- `#` comments are allowed only in examples and must be rejected in signed manifests.
- Keys are lowercase ASCII with dot separators.
- Values are trimmed ASCII unless a later field explicitly allows UTF-8.
- The signed canonical form sorts all non-signature keys lexicographically, uses LF endings, and ends with one trailing LF.
- SHA256 values use `sha256:` plus 64 uppercase hexadecimal characters.

The line-oriented format matches the current TinyRustLM evidence style and keeps the first Rust verifier independent of a JSON dependency.

## Required Fields

| Field | Requirement |
| --- | --- |
| `manifest.version` | Must be `0`. |
| `manifest.kind` | Must be `minimodel.manifest`. |
| `manifest.schema_id` | Must be `minimodel.manifest.v0`. |
| `manifest.schema_checksum` | SHA256 of the published schema text or schema key list. |
| `manifest.created_utc` | ISO-like UTC timestamp from the publisher. |
| `model.id` | Stable model id, lowercase route-safe text. |
| `model.version` | Publisher version string. |
| `publisher.id` | Route-safe publisher id. |
| `publisher.key_id` | Public key identity used for signatures. |
| `model_card.route` | Metadata route, never model bytes. |
| `license.route` | License route or license evidence route. |
| `artifact.kind` | Must be `slm` for the first verifier. |
| `artifact.byte_count` | Decimal byte count for the local `.slm` file. |
| `artifact.sha256` | SHA256 of the local `.slm` file. |
| `artifact.acquisition` | Must be `user-local-file`, `user-external-download`, or `consent-peer-transfer`. |
| `artifact.project_server_url` | Must be `none`. |
| `slm.format_version` | `.slm` format version expected by TinyRustLM. |
| `slm.model_shape` | TinyRustLM source-shape id or explicit shape id. |
| `slm.quantization` | `f32`, `q8_0`, `q4_0`, or a later registered value. |
| `slm.tokenizer_checksum` | Tokenizer checksum carried by the `.slm` evidence. |
| `slm.tensor_layout_checksum` | Tensor-layout checksum carried by the `.slm` evidence. |
| `runtime.compatibility` | Runtime family, for example `tinyrustlm`. |
| `runtime.minimum_version` | Minimum runtime version required by the publisher. |
| `source.kind` | `safetensors`, `slm-native`, `synthetic`, or `unknown`. |
| `source.id` | Source checkpoint, recipe, or local source id. |
| `source.revision` | Source revision, commit, digest, or `unknown`. |
| `evidence.admission.status` | `passed`, `pending`, or `unavailable`. |
| `signature.kind` | `unsigned-draft` for examples, or a later registered signature envelope. |

## Evidence Fields

Evidence fields are optional only when their paired status is `unavailable` or `pending`.

| Field group | Purpose |
| --- | --- |
| `source.config.route`, `source.config.sha256` | Source config evidence. |
| `source.tokenizer.route`, `source.tokenizer.sha256` | Tokenizer evidence. |
| `evidence.source_review.route`, `evidence.source_review.sha256` | Reviewed pre-extraction candidate report. |
| `evidence.source_validation.route`, `evidence.source_validation.sha256` | Source validation sidecar. |
| `evidence.runtime_smoke.route`, `evidence.runtime_smoke.sha256` | Runtime smoke sidecar. |
| `evidence.eval.route`, `evidence.eval.sha256` | Eval sidecar. |
| `evidence.admission.route`, `evidence.admission.sha256` | Admission sidecar. |

The verifier must reject a checksum without a route, a route without a checksum when the status is `passed`, and any route that claims to be a project-hosted model-byte route.

## Source Discovery Fields

Source discovery fields are optional in v0 but reserved so the verifier can distinguish local, Hugging Face, and future P2P paths without changing the artifact boundary.

| Field | Requirement |
| --- | --- |
| `source.discovery.kind` | `local-file`, `local-list`, `huggingface-hub`, `minimodel-p2p`, or `unknown`. |
| `source.discovery.route` | Metadata or model-card route only; it must not be a project-server model-byte route. |
| `source.discovery.revision` | External source revision, commit, or `unknown`. |
| `source.discovery.user_token_required` | `true` only when a user-supplied external credential is required; project-owned tokens are forbidden. |

Hugging Face `/resolve/`, Xet/CAS, Inference Providers, Inference Endpoints, and executable Spaces are not MiniModel.org server capabilities. They may inform external discovery or user-side acquisition only.

## Chunk Fields

Chunk fields are reserved for the later peer-transfer phase. The local-only verifier must still parse and validate them when present.

| Field | Requirement |
| --- | --- |
| `chunks.mode` | `none` or `fixed-size-merkle-v0`. |
| `chunks.size` | Decimal bytes per chunk when chunking is enabled. |
| `chunks.count` | Decimal chunk count when chunking is enabled. |
| `chunks.merkle_root_sha256` | Merkle root over chunk SHA256 values. |
| `chunks.list.route` | Optional metadata route for chunk hashes, not model bytes. |
| `chunks.list.sha256` | Checksum for the chunk list metadata. |

For `chunks.mode=none`, the verifier skips chunk verification but still verifies the whole artifact byte count and checksum.

## Signature Fields

The MVP may publish unsigned examples, but implementation should reserve canonical signature fields now.

| Field | Requirement |
| --- | --- |
| `signature.kind` | `unsigned-draft` or a registered signature scheme. |
| `signature.key_id` | Publisher key id, or `none` for unsigned draft examples. |
| `signature.public_key_route` | Metadata route for public key material, not a model byte route. |
| `signature.payload_sha256` | SHA256 of the canonical non-signature manifest body. |
| `signature.value` | Encoded signature value, or `none` for unsigned draft examples. |

## Rejection Rules

The first verifier must reject manifests that:

- include a project-server URL for model bytes
- require server-side execution
- omit license or model-card routes
- declare an artifact kind other than `slm`
- mismatch artifact byte count or SHA256
- mismatch tokenizer, tensor-layout, runtime, eval, source-review, source-validation, runtime-smoke, or admission evidence when those fields are present
- contain remote code hooks, install commands, scripts, post-load callbacks, or automatic browser-fetch instructions
- claim project endorsement, safety, legality, training cleanliness, or fitness for purpose

## Verification Order

1. Parse line fields and reject duplicate keys.
2. Verify required schema and boundary fields.
3. Verify license and model-card routes are present.
4. Verify signature fields when `signature.kind` is not `unsigned-draft`.
5. Verify chunk metadata if `chunks.mode` is enabled.
6. Verify local artifact byte count and SHA256.
7. Verify TinyRustLM `.slm` header, checksum, tokenizer checksum, tensor layout checksum, shape, and quantization.
8. Verify evidence sidecar checksums and route echoes when available.
9. Write a local import receipt.

The manifest alone never authorizes SLMComposer to fetch bytes. The user must provide the local artifact.
