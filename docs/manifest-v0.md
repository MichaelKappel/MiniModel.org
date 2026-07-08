# MiniModel Manifest v0 Draft

This draft defines the first implementable `minimodel.manifest` shape. It is designed for a small Rust parser, local artifact verification, and later peer transfer. It is not a server fetch contract.

## Boundary

- MiniModel.org servers may publish this schema, example manifests, signed catalog metadata, public key routes, and contact routes.
- MiniModel.org servers must not host user model bytes or proxy model-byte downloads.
- TinyRustLM remains browser-local. Users select local `.slm` files; Rust/WASM validates the bytes.
- The first verifier accepts a manifest file plus a user-provided local artifact path.

## Format

The MVP manifest is a UTF-8 text file with one `key=value` field per line.

- Lines are LF or CRLF terminated.
- Empty lines are ignored.
- `#` comments are allowed only in examples and must be rejected in signed manifests.
- Keys are lowercase ASCII with dot separators.
- Values are trimmed ASCII unless a later field explicitly allows UTF-8.
- The signed canonical form sorts all non-signature keys lexicographically, uses LF endings, and ends with one trailing LF.
- SHA256 values use `sha256:` plus 64 hexadecimal characters. The verifier normalizes hex case.

The line-oriented format matches the current TinyRustLM evidence style and keeps the first Rust verifier independent of a JSON dependency.

The first real fixture manifest is [../examples/tiny-test-model-q8.minimodel.manifest.v0](../examples/tiny-test-model-q8.minimodel.manifest.v0). It binds the TinyRustLM q8 fixture to its artifact SHA256, `.slm` header checksum, tokenizer checksum, tensor-layout checksum, peer-offer route, chunk-list SHA256, and Merkle root without hosting the model bytes.

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
| `signature.kind` | `unsigned-draft` for examples, or `external-signature-v0` for externally verified signatures with Rust-checked payload binding. |

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
| `source.huggingface_model_id` | Hugging Face model id when the converted `.slm` originated from a reviewed Hub checkpoint. |
| `source.discovery.user_token_required` | `true` only when a user-supplied external credential is required; project-owned tokens are forbidden. |

Hugging Face `/resolve/`, Xet/CAS, Inference Providers, Inference Endpoints, and executable Spaces are not MiniModel.org server capabilities. They may inform external discovery or user-side acquisition only.

TinyRustLM may save a `minimodel.huggingface_conversion_request.v0` JSON handoff when a Hub result has no admitted `.slm` metadata yet. That request is not a manifest and not an execution grant. It is a local planning artifact for a converter or future peer workflow: review the source, convert once, validate the produced `.slm`, publish checksum-bound metadata, and only then make peer availability visible.

TinyRustLM may also save a `.tinyrustlm-convert.ps1` handoff script for the same Hub record. The script should be treated as a local convenience wrapper around Rust commands, not as MiniModel metadata and not as an acquisition grant. Safetensors handoffs run `tinyrustlm-slm-pack convert-huggingface-request`, which consumes the saved conversion request plus a user-owned local `.safetensors` file or shard directory, writes a verified `.slm`, and writes a checksum-bound conversion receipt before MiniModel `prepare-peer-share` publishes metadata. GGUF handoffs are review-first and run `inspect-gguf`, `plan-gguf-source`, and `convert-gguf-trained` against a user-owned F32/F16/BF16/Q2_K/Q3_K/Q4_0/Q4_1/Q4_K/Q5_0/Q5_1/Q5_K/Q6_K/Q8_0/Q8_1/Q8_K `.gguf` file before the same MiniModel metadata path; GGUF conversion still requires supported tensor names/shapes, tokenizer token-count compatibility, and Rust validation before .slm import; `plan-gguf-source` fails closed when `tokenizer.ggml.tokens` contradicts the target vocab, and BPE source shapes import `tokenizer.ggml.tokens`/`tokenizer.ggml.merges` into the raw source manifest when both are declared. The script must not download model bytes.

The peer-offer output is a text manifest, usually named `.minimodel-peer-offer`, with SHA-256 artifact identity, fixed-size chunk SHA-256 rows, a chunk-list SHA-256, and a Merkle root. It proves what a future peer client must transfer and what the recipient must reverify before local browser import. It is not executable code and not a project-hosted artifact URL.

After a user imports a verified local `.slm`, TinyRustLM may save a `minimodel.local_slm_share_metadata.v0` JSON bundle. `Save Share Metadata` remains a metadata-only bundle with a catalog-entry candidate and a `minimodel.peer_offer.v0` metadata block containing real artifact SHA-256, chunk SHA-256 entries, chunk-list SHA-256, and a Merkle root. `Save P2P Share Kit` may save a second bundle for the same artifact with `peer_transfer_implementation=generated-local-minimodel-share-kit-v0`, `local_share_kit_available=true`, and direct MiniModel protocol hints, but it must still keep `network_transfer_available=false` until the user runs local scripts, chooses a consenting peer URL, and Rust re-verifies transfer. MiniModel can validate either bundle shape with `inspect-share-metadata`.

Generated TinyRustLM share-kit descriptors advertise `writes_import_receipt_ledger=true` when the local script writes and inspects a sender-side `.minimodel-receipt` plus metadata-only `import-receipts.v0.json` before peer availability metadata. That ledger is local audit/companion metadata only; it does not imply a peer import happened or that network transfer is available.

MiniModel can prepare the normal post-conversion share package in one local command:

```text
minimodel prepare-peer-share <artifact.slm> <output-dir> <model-id> [source-huggingface-model-id] [known-peer-base-url]
minimodel write-peer-companion-state <manifest> <piece-dir> <output-json> <known-peer-base-url>
minimodel inspect-peer-companion-state <state-json> <manifest> <piece-dir>
minimodel write-peer-companion-inventory <output-json> <state-json> <manifest> <piece-dir> <receipt-ledger-json> <piece-store-quota-bytes> [contact-card-json...]
minimodel inspect-peer-companion-inventory <inventory-json>
minimodel write-peer-contact-card <output-json> <known-peer-base-url> [peer-label]
minimodel inspect-peer-contact-card <card-json>
minimodel write-peer-contact-book <output-json> <contact-card-json>...
minimodel inspect-peer-contact-book <book-json>
minimodel inspect-share-metadata <bundle-json>
minimodel serve-peer-companion <state-json> <manifest> <piece-dir> <bind-addr>
minimodel check-peer-status <manifest> <known-peer-base-url> <output-json>
minimodel inspect-peer-status-check <status-check-json> <manifest>
minimodel inspect-peer-import-request <request-json> <manifest>
minimodel inspect-import-receipt <receipt.minimodel-receipt> <manifest> <artifact.slm>
minimodel inspect-peer-import-receipt <receipt.minimodel-receipt> <request-json> <preflight> <manifest> <artifact.slm>
minimodel write-import-receipt-ledger <output-json> <receipt> <manifest> <artifact.slm> [<receipt> <manifest> <artifact.slm>...]
minimodel inspect-import-receipt-ledger <ledger-json>
```

`prepare-peer-share` validates the local `.slm`, writes a MiniModel manifest, writes a matching peer-offer proof, exports local peer pieces, and writes a catalog-entry JSON. `[source-huggingface-model-id]` records the upstream Hub identity when the `.slm` came from a reviewed local conversion. `[known-peer-base-url]` is optional consenting-peer metadata; it never makes MiniModel.org a byte host.

`write-peer-companion-state` validates an already prepared share and writes metadata-only `minimodel.peer_companion_state` JSON for a future local companion UI. It binds manifest, artifact, piece-set, piece-count, Merkle-root, command-hint, consent, and no-project-server-byte fields without embedding `.slm` bytes or peer-piece payloads. `inspect-peer-companion-state` reruns the state/manifest/piece-directory checks on an existing JSON file and reports the metadata-only boundary before the state is served or handed to TinyRustLM.

`write-peer-companion-inventory` writes metadata-only `minimodel.peer_companion_inventory.v0` JSON for a future local companion UI. It binds an already inspected companion state to the manifest, peer-piece directory, sender-side receipt ledger, piece-store byte/quota/SHA-256 evidence, optional contact cards, and a `serve_plan` with exact user-run serving routes, preflight commands, consent, and Rust-reverify flags. `inspect-peer-companion-inventory` revalidates that envelope and rejects piece-store SHA-256, duplicate contact-card peer URLs, serve-plan, or byte-boundary drift; the inventory carries no `.slm` bytes or peer-piece payloads.

`write-peer-contact-card` writes metadata-only `minimodel.peer_contact_card` JSON for the known-peer URL itself. The card carries a printable label, the normalized direct `http://` peer base URL, `direct-http-piece-server-v0`, explicit no-project-server serve/run/proxy flags, and a `requires_rust_reverify_before_browser_import` boundary flag. `inspect-peer-contact-card` revalidates the same envelope. TinyRustLM may use an accepted card to fill the Known peer URL for proof-ready rows, but the card is not a manifest, not a peer-source announcement, and not model-byte availability by itself.

`write-peer-contact-book` writes metadata-only `minimodel.peer_contact_book.v0` JSON for a reviewed list of peer contact cards. The book embeds each card with a canonical SHA-256, rejects duplicate peer URLs, declares no model bytes, peer pieces, or catalog availability claims, and keeps `requires_rust_reverify_before_browser_import=true`. `inspect-peer-contact-book` revalidates the embedded cards and hashes before TinyRustLM or a companion UI trusts the first peer as the active Known peer.

`serve-peer-companion` validates that state JSON against the manifest and peer-piece directory, then serves `minimodel.peer_status.v0.json`, `minimodel.peer_companion_state.v0.json`, `minimodel.peer_piece_set.v0`, and declared piece files from the user-run local listener. If the state URL is path-prefixed, for example `http://127.0.0.1:8123/peer-pieces/`, the fixed metadata routes and piece files are served under that prefix. `serve-peer-pieces` also serves `minimodel.peer_status.v0.json` for piece-only peers. The peer-status route is metadata-only JSON: it binds model id, manifest SHA-256, artifact byte count/SHA-256, peer-piece-set SHA-256, piece count, Merkle root, route hints, and explicit no-project-server/no-embedded-byte flags. Browser-facing metadata routes now return CORS/PNA headers for `minimodel.peer_status.v0.json`, `minimodel.peer_piece_set.v0`, and advertised companion-state metadata so TinyRustLM can run `Check Peer`; declared chunk files remain non-CORS and still move through the Rust import path. `check-peer-status` fetches only that route from a known direct peer URL and writes a JSON readiness report with `peer_piece_bytes_fetched=false` and `artifact_bytes_fetched=false` before preflight or import. `inspect-peer-status-check` reopens that saved report, validates strict UTC status, peer/status routes, manifest/artifact/piece hashes, no-byte-transfer flags, and `requires_rust_reverify_before_browser_import=true` against the manifest without contacting the peer. The server still rejects project/Hugging Face peer hosts through the state URL validation path and still refuses unlisted files.

The generated catalog-entry JSON can be assembled into a metadata-only catalog snapshot with:

```text
minimodel write-catalog-snapshot <output-json> <catalog-entry-json>...
```

`write-catalog-snapshot` revalidates every entry before writing `minimodel.model_catalog` JSON. It rejects duplicate model ids, artifact/model/download URLs, project-server byte/execution claims, peer-source count drift, unsupported peer-source rows, and project/Hugging Face peer hosts. `inspect-catalog-snapshot <catalog-json>` revalidates an existing `models.v0.json`, checks the top-level metadata-only boundary, duplicate model ids, peer-source rows, checksum-bearing announcement-feed routes, and reports the exact catalog SHA-256. A valid snapshot can make a converted model appear as proof-ready or P2P-ready metadata in TinyRustLM, but it still never contains or serves runnable model bytes.

MiniModel can also write a catalog entry from an admitted manifest plus peer-offer proof:

```text
minimodel write-catalog-entry <manifest> <peer-offer> <output-json> [source-huggingface-model-id] [known-peer-base-url]
```

The command validates that the peer-offer artifact fields, `.slm` fields, chunk count, chunk-list SHA-256, Merkle root, admission checksum, and per-chunk rows match the manifest before JSON is written. A generated catalog row may use `peer_hint_status=peer-offer-ready-known-peer-needed` and `peer_import_status=request-script-ready`. Those fields mean the UI can save a P2P import request and Rust `import-peer-request` script; they do not authorize TinyRustLM or MiniModel.org to fetch model bytes from project servers.

When `[known-peer-base-url]` is supplied, `write-catalog-entry` validates the direct `http://` peer URL, rejects project/Hugging Face host families, normalizes host case/root dots and the trailing slash, and writes:

| Field | Requirement |
| --- | --- |
| `peer_source_count` | Number of peer source rows. The first prototype writes `0` or `1`. |
| `peer_sources[].source_kind` | `known-direct-http-peer-v0`. |
| `peer_sources[].transport` | `direct-http-piece-server-v0`. |
| `peer_sources[].peer_base_url` | Direct consenting peer URL for `minimodel.peer_piece_set.v0` and declared chunk files. |
| `peer_sources[].consent_status` | `publisher-declared-consenting-peer`. |
| `peer_sources[].project_server_serves_model_bytes` | Must be `false`. |
| `peer_sources[].project_server_runs_model_bytes` | Must be `false`. |

If a catalog entry was generated before a consenting peer URL was known, MiniModel can attach that peer-source metadata later without rerunning conversion:

```text
minimodel add-peer-source <catalog-entry-json> <output-json> <known-peer-base-url>
```

`add-peer-source` reads and validates the existing catalog entry, rejects project/Hugging Face peer hosts, writes exactly one `known-direct-http-peer-v0` row, revalidates `peer_source_count`, and writes a new catalog-entry JSON for the next `write-catalog-snapshot` run. It does not upload, proxy, or fetch `.slm` bytes.

For catalog publication where a converter needs to hand peer availability to a maintainer, MiniModel can write and apply a metadata-only announcement:

```text
minimodel write-peer-source-announcement <catalog-entry-json> <announcement-json> <known-peer-base-url>
minimodel apply-peer-source-announcements <catalog-entry-json> <output-json> <announcement-json>...
minimodel write-catalog-snapshot-with-announcements <output-json> <catalog-entry-json>... --announcements <announcement-json>...
minimodel write-peer-source-announcement-feed <output-json> [announcement-json...]
minimodel inspect-peer-source-announcement-feed <feed-json>
minimodel write-catalog-snapshot-with-announcement-feed <output-json> <feed-route> <announcement-feed-json> <catalog-entry-json>...
minimodel inspect-catalog-snapshot <catalog-json>
```

The announcement binds the catalog entry checksum, model id, artifact byte count, artifact SHA256, peer-offer route, peer-offer SHA256, peer-offer chunk count, Merkle root, and one validated consenting direct peer URL. The apply command rejects model/artifact/proof route/checksum/chunk-count/Merkle drift, deduplicates peer URLs, writes `peer_sources`, revalidates the entry, and leaves the next `write-catalog-snapshot` run to publish metadata only. The batch snapshot command validates many entries and announcements together, rejects unmatched announcement model ids, and writes `models.v0.json` directly. This is the maintainable route for "converted once, then later users see P2P Import" without making project servers byte hosts.

`write-peer-source-announcement-feed` validates zero or more portable announcement JSON files into one `minimodel.peer_source_announcement_catalog` metadata feed. An empty feed is valid and means no consenting peer sources are currently published. `inspect-peer-source-announcement-feed` validates an existing feed with the same Rust envelope checks, reports announcement/model counts, computes the exact feed SHA-256, and rejects duplicate model/peer rows or project-server-byte flag drift before a maintainer binds or a receiver applies that feed. `write-catalog-snapshot-with-announcement-feed` validates the feed, confirms every feed model id exists in the catalog entries, computes the feed body SHA-256, rejects URL-like or unsafe feed routes, and writes the checksum-bound feed route into `models.v0.json`.

Catalog snapshots may expose announcement feeds as metadata routes, not byte routes. A snapshot can carry `peer_source_announcement_route` with `peer_source_announcement_sha256`, or `peer_source_announcement_routes` / `peer_source_announcement_feeds` arrays with route and SHA-256 fields. TinyRustLM accepts only HTTP(S) routes that resolve on the same origin as the catalog, checks the feed body against the declared SHA-256, validates the `minimodel.peer_source_announcement_catalog` envelope and duplicate model/peer rules, then applies valid announcements to proof-ready entries that already carry complete peer-offer proof fields. The feed must not contain `.slm` bytes, artifact URLs, Hugging Face `/resolve/` URLs, project/Hugging Face peer hosts, or execution claims.

TinyRustLM may use a catalog-listed peer source to prefill the receiver request, but Rust still fetches, assembles, verifies, writes a local import receipt, inspects that receipt against the assembled `.slm`, and binds the receipt back to the saved request plus preflight report before the user imports the file through the browser picker. A user-run companion or local audit script can then write a metadata-only import receipt ledger for one or more verified receipts; inspecting that ledger reopens each receipt, manifest, and artifact so stale receipt hashes or mismatched artifacts fail closed.

The MiniModel verifier can also turn a verified local `.slm` into local peer pieces with `export-peer-pieces`. The output directory contains chunk files plus `minimodel.peer_piece_set.v0`. `serve-peer-pieces` validates that directory, starts a user-controlled local HTTP listener, and serves only `minimodel.peer_status.v0.json`, the declared sidecar, and declared piece file names. `inspect-peer-companion-state` validates an existing companion-state JSON against the manifest and peer-piece directory before publication or serving. `serve-peer-companion` adds a verified `minimodel.peer_companion_state.v0.json` route for companion-aware clients without exposing extra files, including path-prefixed peer base URLs such as `/peer-pieces/`. `inspect-peer-import-request` validates the saved TinyRustLM request JSON against the metadata manifest and reports route/gate validation status plus request/manifest SHA-256 without contacting the peer. `check-peer-status` can validate the metadata-only status route before any piece-set or piece bytes are fetched, and `inspect-peer-status-check` revalidates that saved status-check JSON against the manifest before later scripts trust it. `fetch-peer-pieces` can retrieve that sidecar and its listed chunks from a user-provided direct `http://` peer endpoint, rejects project/Hugging Face hostnames, validates advertised companion state when present, and verifies every chunk SHA-256 before writing the local piece directory. `assemble-peer-pieces` verifies each local chunk again, assembles a temporary `.slm`, and then applies the full manifest/artifact/`.slm` verifier before writing the accepted output file. `import-peer-request` wraps the receiver sequence for TinyRustLM request JSON: request validation, manifest binding, optional companion-state validation, known-peer fetch, assembly, artifact verification, and receipt writing in one output directory. `inspect-import-receipt` then checks the receipt against the manifest and assembled `.slm`, including peer-transfer fields and no-project-server-byte flags; `inspect-peer-import-receipt` also checks that the receipt still matches the saved request and saved `.preflight` report before browser import. `write-import-receipt-ledger` writes a local `minimodel.import_receipt_ledger.v0` JSON index only after reopening the receipt, manifest, and artifact through that same verifier; `inspect-import-receipt-ledger` repeats the chain and rejects stale entry fields. Peer-import receipts and ledger entries include the direct peer transport, validated receiver-validation contract status/gates, peer URL, piece-set SHA-256, companion-state status, chunk count, Merkle root, `peer_piece_bytes_fetched=true`, `artifact.assembled_from_peer_pieces=true`, and no-project-server-byte flags. The ledger contains no `.slm` bytes and no peer-piece bytes. The request can also carry structured `metadata_routes` for `minimodel.peer_status.v0.json`, `minimodel.peer_piece_set.v0`, and `minimodel.peer_companion_state.v0.json`, plus a `receiver_validation_contract` requiring status check, saved status-check inspection, preflight, advertised companion-state validation, and receipt inspection; MiniModel validates those fields when present, `preflight-peer-import-request` records the resulting route/gate status in the saved `.preflight` report, and `inspect-peer-import-preflight` revalidates that report before piece transfer. This tests the byte contract a future P2P transport must preserve while still keeping project servers out of the model-byte path.

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

The MVP may publish unsigned examples, but signed manifests now have a local Ed25519 verification path.

| Field | Requirement |
| --- | --- |
| `signature.kind` | `unsigned-draft` for examples, or `external-signature-v0` for a payload bound to Ed25519 verification. |
| `signature.key_id` | Publisher key id, or `none` for unsigned draft examples. |
| `signature.public_key_route` | Metadata route for public key material, not a model byte route. |
| `signature.payload_sha256` | SHA256 of the canonical non-signature manifest body. |
| `signature.value` | `ed25519-base64:<base64-signature>` for local signed verification, or `none` for unsigned draft examples. |

For `external-signature-v0`, `verify-manifest` fails closed unless `signature.payload_sha256` exactly matches the canonical sorted manifest body after all `signature.*` fields are removed. `verify-signed-manifest <manifest> <public-key-json>` then verifies `signature.value` as an Ed25519 signature over that canonical payload. The public key JSON is metadata-only `minimodel.public_key.v0` and must bind `key_algorithm=ed25519`, the manifest `publisher.id`, the manifest `signature.key_id`, the manifest `signature.public_key_route`, `public_key_sha256`, `usage=minimodel-manifest-signing-v0`, `revoked=false`, and a UTC creation timestamp. Transparency-log policy and remote key-discovery policy remain later gates. Unknown signature kinds are rejected until registered.

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
9. Write and inspect a local import receipt, then optionally write and inspect a metadata-only receipt ledger.

The manifest alone never authorizes TinyRustLM to fetch bytes. The user must provide the local artifact.
