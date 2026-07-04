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
| `minimodel` integration tests | Public CLI command coverage for `prepare-peer-share` sender packaging, live `serve-peer-companion` -> `check-peer-status` -> `preflight-peer-import-request` -> `inspect-peer-import-preflight` -> `import-peer-request` -> `inspect-import-receipt` -> `inspect-peer-import-receipt` receiver transfer, peer companion-state writing from a prepared share, metadata-only boundary output, peer contact-card write/inspect output, catalog snapshot write/inspect output, TinyRustLM share-metadata inspection for metadata-only and share-kit-ready bundles, URL normalization, unsafe peer path rejection, project/Hugging Face host rejection, and chunk-proof drift rejection before output files are written. |

The first pass keeps manifests and peer-piece sidecars line-oriented where possible. Structured browser handoff JSON is parsed as JSON with `serde_json` so the receiver does not rely on string scanning. Networking, DHTs, relays, the optional user-run MiniModel Peer Companion, and peer identity exchange are later phases.

## Commands

```text
minimodel verify-manifest <manifest>
minimodel verify-artifact <manifest> <artifact.slm>
minimodel write-import-receipt <manifest> <artifact.slm> <receipt.minimodel-receipt>
minimodel inspect-import-receipt <receipt.minimodel-receipt> <manifest> <artifact.slm>
minimodel inspect-peer-import-receipt <receipt.minimodel-receipt> <request-json> <preflight> <manifest> <artifact.slm>
minimodel export-peer-pieces <manifest> <artifact.slm> <output-dir>
minimodel serve-peer-pieces <manifest> <piece-dir> <bind-addr>
minimodel serve-peer-companion <state-json> <manifest> <piece-dir> <bind-addr>
minimodel fetch-peer-pieces <manifest> <http-peer-base-url> <output-dir>
minimodel assemble-peer-pieces <manifest> <piece-dir> <output.slm>
minimodel check-peer-status <manifest> <known-peer-base-url> <output-json>
minimodel inspect-peer-import-request <request-json> <manifest>
minimodel preflight-peer-import-request <request-json> <manifest> <output.preflight>
minimodel inspect-peer-import-preflight <output.preflight> <request-json> <manifest>
minimodel import-peer-request <request-json> <manifest> <output-dir>
minimodel prepare-peer-share <artifact.slm> <output-dir> <model-id> [source-huggingface-model-id] [known-peer-base-url]
minimodel write-peer-companion-state <manifest> <piece-dir> <output-json> <known-peer-base-url>
minimodel write-peer-contact-card <output-json> <known-peer-base-url> [peer-label]
minimodel inspect-peer-contact-card <card-json>
minimodel inspect-share-metadata <bundle-json>
minimodel write-catalog-entry <manifest> <peer-offer> <output-json> [source-huggingface-model-id] [known-peer-base-url]
minimodel add-peer-source <catalog-entry-json> <output-json> <known-peer-base-url>
minimodel write-peer-source-announcement <catalog-entry-json> <announcement-json> <known-peer-base-url>
minimodel apply-peer-source-announcements <catalog-entry-json> <output-json> <announcement-json>...
minimodel write-catalog-snapshot-with-announcements <output-json> <catalog-entry-json>... --announcements <announcement-json>...
minimodel write-peer-source-announcement-feed <output-json> [announcement-json...]
minimodel inspect-peer-source-announcement-feed <feed-json>
minimodel write-catalog-snapshot-with-announcement-feed <output-json> <feed-route> <announcement-feed-json> <catalog-entry-json>...
minimodel write-catalog-snapshot <output-json> <catalog-entry-json>...
minimodel inspect-catalog-snapshot <catalog-json>
```

`verify-manifest` checks schema, required fields, boundary fields, license/model-card routes, route shape, and signature placeholder rules.

`verify-artifact` additionally checks local file byte count, SHA256, chunk metadata when present, and TinyRustLM `.slm` compatibility.

`write-import-receipt` only succeeds after `verify-artifact` passes.

`inspect-import-receipt` reparses an existing receipt, revalidates the manifest and `.slm` artifact, compares the receipt fields to freshly computed SHA-256/shape/quantization evidence, and rejects peer-import receipt drift including project-server byte flags.

`inspect-peer-import-receipt` is the stricter final gate for TinyRustLM P2P imports. It first runs the normal receipt/artifact verifier, then binds the peer receipt back to the saved request JSON and saved `.preflight` report so request basename, peer URL, companion-state URL/status, piece-set SHA-256, artifact SHA-256, chunk count, Merkle root, metadata-fetch flags, fetched-piece evidence, assembled-from-peer-pieces evidence, strict UTC creation timestamps, receipt-after-preflight order, and no-project-server-byte flags all agree before browser import.

`export-peer-pieces` only succeeds after `verify-artifact` passes. It writes local piece files plus `minimodel.peer_piece_set.v0`, a metadata sidecar that binds the manifest checksum, artifact checksum, chunk count, chunk SHA256 rows, chunk-list SHA256, and Merkle root. The piece files are model bytes and must stay outside project servers.

`serve-peer-pieces` validates an existing peer-piece directory before binding a local HTTP listener. It serves only `minimodel.peer_status.v0.json`, `minimodel.peer_piece_set.v0`, and the exact piece file names declared in that sidecar. The peer-status route is metadata-only JSON with manifest/artifact/piece-set checksums, no embedded model or piece bytes, no-project-server flags, and a Rust-reverify-required boundary. It does not publish to MiniModel.org, does not discover peers, and stops when the user stops the process.

`serve-peer-companion` validates a `minimodel.peer_companion_state` JSON file against the same manifest and peer-piece directory before binding a local HTTP listener. It serves `minimodel.peer_status.v0.json`, the fixed `minimodel.peer_companion_state.v0.json` route, `minimodel.peer_piece_set.v0`, and the declared piece files only; unlisted files in the share directory stay inaccessible. When the validated `peer_base_url` includes a path prefix such as `http://127.0.0.1:8123/peer-pieces/`, the companion server serves the allow-listed routes under that prefix and rejects unrelated paths.

`fetch-peer-pieces` is the first live transfer-shaped prototype. It pulls `minimodel.peer_piece_set.v0`, opportunistically validates `minimodel.peer_companion_state.v0.json` when the peer advertises that route, then fetches each listed chunk from a user-provided `http://` peer base URL. The base URL may be root-scoped or path-prefixed; Rust joins `minimodel.peer_piece_set.v0`, status, companion-state, and piece filenames under that base URL. It accepts only plain direct `http://` peer URLs with DNS/IPv4 host text and a safe absolute path, rejecting query strings, fragments, traversal, whitespace, control characters, malformed dot hosts, and project/Hugging Face canonical/www/subdomain/root-dot hostnames before transfer. It verifies the piece-set against the manifest, rejects companion-state drift when state exists, verifies every downloaded piece SHA256 before writing it, and leaves final artifact acceptance to `assemble-peer-pieces`.

`assemble-peer-pieces` reads a local `minimodel.peer_piece_set.v0`, verifies each local piece, assembles a temporary `.slm`, runs the full artifact verifier again, and moves the output into place only after verification passes. This is the first transport-shaped primitive; it is not live peer networking.

`inspect-peer-import-request` is the offline receiver-side request preflight for a TinyRustLM `minimodel.peer_import_request.v0` file. It parses the saved request JSON, validates it against the supplied metadata manifest, rejects project-server and Hugging Face byte-source claims, rejects project/Hugging Face peer hosts and unsafe peer URL paths before manifest reads, validates optional structured `metadata_routes` and `receiver_validation_contract` fields when present so route/gate drift fails closed, reports route/gate validation status plus the exact request SHA-256 and manifest SHA-256, and exits before any peer HTTP request. TinyRustLM-generated import scripts run this first so request or manifest drift fails before contacting the peer.

`preflight-peer-import-request` is the receiver-side metadata preflight for the same file after offline inspection has passed. It validates the request against the supplied manifest again, fetches only the peer's small `minimodel.peer_piece_set.v0` metadata file plus optional `minimodel.peer_companion_state.v0.json` metadata, validates those contracts against the manifest, and writes a `.preflight` report with `metadata_routes.status`, each metadata route, `receiver_validation_contract.status`, each receiver validation gate, `piece_set.sha256`, `peer_piece_bytes_fetched=false`, and `artifact_bytes_fetched=false`. If the companion-state route returns 404, the report records `peer.companion_state_status=not-advertised` for legacy peers; if the route exists, any manifest/artifact/piece-set/count/Merkle/peer-URL drift fails preflight before model bytes are fetched.

`inspect-peer-import-preflight` is the offline verifier for that saved `.preflight` report. It parses the report without contacting the peer, binds it back to the request JSON and metadata manifest, rechecks the route/gate contract fields, manifest/model/artifact/chunk identity, metadata fetch flags, and the no-piece/no-artifact/no-project-server-byte boundary, then reports the preflight SHA-256. TinyRustLM-generated import scripts run this after metadata preflight and before `import-peer-request` so a tampered or mismatched preflight artifact fails before peer pieces move.

`check-peer-status` is the receiver-side readiness check for a known direct peer URL. It fetches only `minimodel.peer_status.v0.json`, validates model id, manifest SHA-256, artifact byte count/SHA-256, peer-piece-set SHA-256, piece count, Merkle root, peer URL, and no-project-server/no-embedded-byte boundary flags against the supplied manifest, then writes a metadata-only JSON report with `peer_piece_bytes_fetched=false` and `artifact_bytes_fetched=false`. TinyRustLM-generated import scripts run this before preflight so users get a fast peer identity/readiness failure before any piece transfer.

`import-peer-request` is the receiver-side one-command path for a TinyRustLM `minimodel.peer_import_request.v0` file. It parses the request, rejects project-server or Hugging Face byte-source claims, checks the request's model id, byte count, artifact SHA-256, safe local output names, and known peer URL against the supplied manifest, then runs the same companion-aware fetch, assemble, full artifact verification, and receipt-writing chain into the chosen output directory. The final receipt now includes a peer-import transfer section with the request basename, direct HTTP peer transport, peer URL, piece-set SHA-256, companion-state status, Merkle root, `peer_piece_bytes_fetched=true`, `artifact.assembled_from_peer_pieces=true`, and explicit project-server byte flags set to `false`. TinyRustLM-generated import scripts then run `inspect-import-receipt` against that receipt, the manifest, and the assembled `.slm`, followed by `inspect-peer-import-receipt` against the receipt, request JSON, saved preflight report, manifest, and assembled `.slm` before the user imports the local file; that final chain check rejects malformed UTC timestamps and any receipt whose `receipt.created_utc` predates `preflight.created_utc`. The `peer_import_cli` integration smoke validates `inspect-peer-import-request` without a running peer server, rejects project/Hugging Face and unsafe-path peer URLs before manifest reads, starts the actual `serve-peer-companion` binary on a loopback path-prefixed peer URL, then runs the actual receiver-side `check-peer-status`, `preflight-peer-import-request`, `inspect-peer-import-preflight`, `import-peer-request`, `inspect-import-receipt`, and `inspect-peer-import-receipt` commands against it; it verifies metadata-only status/preflight reports, companion-state validation, fetched peer-piece receipt evidence, drift rejection for preflight and receipt project-server flags, receipt/preflight piece-set SHA drift, future-dated preflight order drift, and byte-for-byte assembled `.slm` output. This is the first "select a proof-ready model, enter peer URL, run one Rust verifier command" path; it still requires a consenting direct peer and never fetches model bytes from MiniModel.org.

`prepare-peer-share` is the sender-side one-command path after a user or converter has a local `.slm`. It validates the `.slm`, writes a MiniModel manifest, writes a `.minimodel-peer-offer` proof, exports `minimodel.peer_piece_set.v0` plus local piece files, and writes a catalog-entry JSON in one output directory. If `[known-peer-base-url]` is supplied, the generated catalog entry includes validated `peer_source_count` / `peer_sources` metadata; otherwise it is proof-ready and still requires a receiver to set one consenting peer URL. The command does not download from Hugging Face, does not upload model bytes, and does not make project servers serve or run the artifact. The `peer_share_cli` integration test now drives the actual `minimodel` binary against the TinyRustLM q8 fixture, verifies the generated manifest, peer-offer, peer-piece sidecar, and catalog entry with a path-prefixed known peer URL, and confirms project/Hugging Face peer URLs fail before artifact bytes are read.

`write-peer-companion-state` is a companion-ready local state export for an already prepared share. It validates the manifest and peer-piece directory, normalizes a consenting direct `http://` peer URL, rejects project/Hugging Face hosts, and writes `minimodel.peer_companion_state` JSON with manifest/artifact/piece-set checksums, piece count, Merkle root, command hints, consent status, and explicit no-project-server-byte flags. `inspect-peer-companion-state` reruns those checks against an existing state JSON plus the manifest and piece directory, then reports the state SHA contract, artifact metadata, chunk/Merkle proof, and metadata-only no-embedded-byte boundary before publication or serving. The JSON contains metadata and local path hints only; it does not contain `.slm` bytes or peer-piece payloads. The `peer_companion_state_cli` integration test now drives the actual `minimodel` binary through `export-peer-pieces`, `write-peer-companion-state`, and `inspect-peer-companion-state` against the TinyRustLM q8 fixture, verifies the metadata-only state contract and path-prefixed peer URL normalization, rejects piece-count drift, rejects project-server flag drift, and confirms project/Hugging Face peer URLs fail before manifest or piece files are read.

`write-peer-contact-card` writes a smaller metadata-only `minimodel.peer_contact_card` JSON file for a reviewed consenting direct peer URL. It validates the same known-peer URL boundary, records a printable label, declares `direct-http-piece-server-v0`, and sets explicit no-project-server serve/run/proxy flags plus `requires_rust_reverify_before_browser_import=true`. `inspect-peer-contact-card` validates that shape before a receiver or TinyRustLM UI uses it. The `peer_contact_card_cli` integration test runs the actual `minimodel` binary for both commands, confirms path-prefixed peer URLs normalize with a trailing slash, and confirms project/Hugging Face peer URLs fail before the card file is written. The card is not a catalog availability claim, does not embed `.slm` bytes or peer pieces, and does not let MiniModel.org host, proxy, fetch, or run model bytes.

`inspect-share-metadata` validates a TinyRustLM `minimodel.local_slm_share_metadata.v0` export before a user, converter, or maintainer treats it as MiniModel metadata. It accepts the metadata-only `Save Share Metadata` bundle and the local script-ready `Save P2P Share Kit` bundle, but it requires both to keep `network_transfer_available=false` and project-server byte flags false. It validates the catalog-entry candidate, artifact filename and SHA-256 fields, peer-offer chunk rows, chunk-list SHA-256, Merkle root, conversion-request source shape, and the exact share-kit descriptor when present. The `share_metadata_cli` integration test runs the public binary against both valid bundle profiles and proves project-server byte claims plus chunk-list drift fail closed; TinyRustLM's `share-metadata-ui` browser smoke writes its actual downloaded bundles to temporary files and runs the same Rust command against them, while `share-metadata-import-ui` proves the browser can load a local bundle back as checksum-bound metadata without promoting it to `P2P Import` before generated catalog/manifest metadata exists. The same import smoke is green against the packaged TinyRustLM.com root through mapped `http://tinyrustlm.com:<port>/`, including chunk-list drift rejection, wrong local `.slm` rejection, matching local `.slm` Rust/WASM load and generation, and an app/runtime-only network trace.

TinyRustLM's `share-metadata-kit-exec` browser smoke now executes the exact downloaded `.minimodel-share-kit.ps1` with `MINIMODEL_KNOWN_PEER_BASE_URL`, verifies the MiniModel manifest, catalog snapshot, with-peer catalog entry, peer-source announcement/feed, peer-companion state, peer-companion-state inspection PASS output, peer contact card, and serve helper outputs, starts `serve-peer-companion`, and runs `check-peer-status` to prove the resulting known-peer readiness report remains metadata-only with `peer_piece_bytes_fetched=false` and `artifact_bytes_fetched=false`. The same smoke is green against the packaged TinyRustLM.com root through mapped `http://tinyrustlm.com:<port>/`, proving the public-host share-kit UX drives the same MiniModel verifier commands.

`write-catalog-entry` validates a MiniModel manifest and `.minimodel-peer-offer` proof against each other before writing JSON catalog metadata. It rejects artifact, SLM field, chunk count, chunk-list, Merkle root, evidence checksum, and per-chunk drift. Any import-ready or peer-sourced catalog entry must carry the complete peer-offer proof envelope: kind, route, proof SHA-256, chunk algorithm, chunk size, chunk count, and Merkle root. The generated entry may advertise `peer_hint_status=peer-offer-ready-known-peer-needed` and `peer_import_status=request-script-ready`, which means TinyRustLM can save a peer-import request/script, not that MiniModel.org serves bytes. When `[known-peer-base-url]` is provided, it also validates a direct `http://` consenting peer URL, rejects project/Hugging Face hosts, and writes `peer_source_count` / `peer_sources` metadata so the browser can offer `P2P Import` without a separate manual peer URL step.

`add-peer-source` is the late availability step for a proof-ready catalog entry. It reads an existing catalog-entry JSON, validates the entry, rejects project/Hugging Face peer hosts, writes exactly one `known-direct-http-peer-v0` row for the provided consenting direct peer URL, revalidates `peer_source_count`, and writes a new catalog-entry JSON. This lets a converter produce proof metadata first, then publish P2P availability later without rerunning Hugging Face conversion or uploading `.slm` bytes.

`write-peer-source-announcement` is the portable availability step. It reads a validated catalog-entry JSON plus a consenting direct peer URL and writes `minimodel.peer_source_announcement` JSON. The announcement binds model id, artifact byte count, artifact SHA256, peer-offer route, peer-offer SHA256, peer-offer chunk count, peer-offer Merkle root, the catalog entry checksum, and one validated `known-direct-http-peer-v0` row. It is metadata only and still does not upload, proxy, fetch, or serve model bytes.

`apply-peer-source-announcements` is the catalog-maintainer merge step. It validates a proof-ready catalog entry, validates one or more peer-source announcements, rejects model/artifact/peer-offer route/checksum/chunk-count/Merkle drift, deduplicates peer URLs, writes `peer_sources`, revalidates `peer_source_count`, and produces a new catalog-entry JSON for the next `write-catalog-snapshot` run. This is the first durable "converted once, later users see P2P Import by default" publication path while keeping project servers out of the byte path.

`write-catalog-snapshot-with-announcements` is the maintainer batch publication step. It validates one or more catalog-entry JSON files, validates one or more peer-source announcement JSON files, rejects unmatched announcement model ids, applies only announcements that match their entry, deduplicates peer URLs, and writes the final metadata-only `models.v0.json` snapshot directly. This is the easiest checked path when many converters have sent availability metadata.

`write-peer-source-announcement-feed` is the maintainer feed step. It validates zero or more `minimodel.peer_source_announcement` JSON files and writes a metadata-only `minimodel.peer_source_announcement_catalog` feed with server-byte, server-run, and server-proxy flags set to `false`. An empty feed is a valid "availability checked, no peer sources published yet" state. `inspect-peer-source-announcement-feed` revalidates an already written feed, reports announcement/model counts plus the exact feed SHA-256, rejects duplicate model/peer rows and no-project-server flag drift, and gives receivers or catalog maintainers the same Rust preflight TinyRustLM expects before a local feed is applied. The feed is small JSON metadata: it can say which consenting direct peers claim availability, but it does not contain `.slm` bytes and does not authorize project infrastructure to fetch them. The `peer_source_announcement_feed_cli` integration test now drives the public binary through a prepared-share announcement feed, an empty feed, and a project-server-flag rejection.

`write-catalog-snapshot-with-announcement-feed` is the checksum-binding publication step. It validates the announcement feed, verifies that every feed model id exists in the supplied catalog entries, rejects unsafe URL-like feed routes, computes the feed SHA-256 from the exact JSON bytes, and writes a `models.v0.json` snapshot with `peer_source_announcement_routes`. TinyRustLM can then fetch that same-origin route, verify the declared SHA-256, validate the metadata-only feed envelope, reject duplicate model/peer rows, and apply matching announcements before showing `P2P Import`. The catalog entries remain proof-ready metadata; model bytes still come only from local files or consenting peers through Rust `import-peer-request`.

TinyRustLM's local `Peer-source announcement/feed JSON` picker now uses the same feed-envelope validator for user-selected `peer-source-announcements.v0.json` files, so a recipient can apply a maintainer-style announcement feed to a local proof-ready `models.v0.json` snapshot without fetching a catalog-hosted feed route.

`write-catalog-snapshot` validates one or more catalog-entry JSON files and writes a metadata-only `minimodel.model_catalog` snapshot. It rejects duplicate `model_id` values, artifact/model/download URLs, project-server byte/execution claims, peer-source count drift, unsupported peer-source rows, and project/Hugging Face peer hosts. `inspect-catalog-snapshot` revalidates an existing `models.v0.json`, checks top-level no-server-byte flags, duplicate model ids, peer-source rows, checksum-bearing same-origin announcement-feed routes, and reports entry/P2P/source/route counts plus the exact catalog SHA-256. This is the first Rust-owned publication step for "converted once, listed as P2P-ready later"; the snapshot can be loaded by TinyRustLM as metadata, while model bytes still come only from local files or consenting peers.

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

Peer-import receipts append transfer evidence after the same local artifact verification fields:

| Field | Meaning |
| --- | --- |
| `peer_import.request_path_hint` | Request basename only. |
| `peer_import.transport` | `known-direct-http-peer-v0`. |
| `peer_import.verification_status` | `passed` after piece fetch, assembly, and artifact verification. |
| `peer.base_url` | Consenting direct peer URL used by Rust. |
| `peer.piece_set_sha256` | SHA-256 of the fetched `minimodel.peer_piece_set.v0` metadata. |
| `peer.companion_state_status` | `validated` or `not-advertised`. |
| `chunks.count` / `chunks.merkle_root_sha256` | Chunk contract accepted for the transfer. |
| `peer_piece_bytes_fetched` | `true` for completed peer imports. |
| `artifact.assembled_from_peer_pieces` | `true` for completed peer imports. |
| `project_server_serves_model_bytes` | Always `false` for the accepted import path. |

Receipts must not contain model bytes, public model-byte URLs, or claims that MiniModel.org endorses the artifact.
`inspect-import-receipt` is the local gate for receipt reuse: it requires the receipt to match the revalidated manifest/artifact bytes and, for peer imports, requires the transfer section and no-project-server flags to remain intact. `inspect-peer-import-receipt` is the stronger receiver-script gate for completed P2P imports because it also proves the receipt still matches the saved request and saved preflight evidence, and that the receipt creation time is not earlier than the preflight creation time.

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
cargo run -p minimodel -- check-peer-status examples\tiny-test-model-q8.minimodel.manifest.v0 http://127.0.0.1:8123/ .\tiny-test-model-q8.peer-status-check.v0.json
cargo run -p minimodel -- inspect-peer-import-request .\tiny-test-model-q8.minimodel-peer-import-request.json examples\tiny-test-model-q8.minimodel.manifest.v0
cargo run -p minimodel -- preflight-peer-import-request .\tiny-test-model-q8.minimodel-peer-import-request.json examples\tiny-test-model-q8.minimodel.manifest.v0 .\tiny-test-model-q8.peer-import.preflight
cargo run -p minimodel -- inspect-peer-import-preflight .\tiny-test-model-q8.peer-import.preflight .\tiny-test-model-q8.minimodel-peer-import-request.json examples\tiny-test-model-q8.minimodel.manifest.v0
cargo run -p minimodel -- import-peer-request .\tiny-test-model-q8.minimodel-peer-import-request.json examples\tiny-test-model-q8.minimodel.manifest.v0 .\tiny-test-model-q8-peer-import
cargo run -p minimodel -- inspect-import-receipt .\tiny-test-model-q8-peer-import\tiny-test-model-q8.minimodel-receipt examples\tiny-test-model-q8.minimodel.manifest.v0 .\tiny-test-model-q8-peer-import\tiny-test-model-q8.slm
cargo run -p minimodel -- inspect-peer-import-receipt .\tiny-test-model-q8-peer-import\tiny-test-model-q8.minimodel-receipt .\tiny-test-model-q8.minimodel-peer-import-request.json .\tiny-test-model-q8.peer-import.preflight examples\tiny-test-model-q8.minimodel.manifest.v0 .\tiny-test-model-q8-peer-import\tiny-test-model-q8.slm
cargo run -p minimodel -- prepare-peer-share ..\SLMComposer.com\tinyrustlm\models\tiny-test-model-q8.slm .\prepared-share\tiny-test-model-q8 tinyrustlm/prepared-tiny-test-model-q8 tinyrustlm/tiny-test-model-q8 http://127.0.0.1:8123/
cargo run -p minimodel -- write-peer-companion-state .\prepared-share\tiny-test-model-q8\tiny-test-model-q8.minimodel.manifest.v0 .\prepared-share\tiny-test-model-q8\tiny-test-model-q8.peer-pieces .\prepared-share\tiny-test-model-q8\peer-companion-state.v0.json http://127.0.0.1:8123/
cargo run -p minimodel -- inspect-peer-companion-state .\prepared-share\tiny-test-model-q8\peer-companion-state.v0.json .\prepared-share\tiny-test-model-q8\tiny-test-model-q8.minimodel.manifest.v0 .\prepared-share\tiny-test-model-q8\tiny-test-model-q8.peer-pieces
cargo run -p minimodel -- serve-peer-companion .\prepared-share\tiny-test-model-q8\peer-companion-state.v0.json .\prepared-share\tiny-test-model-q8\tiny-test-model-q8.minimodel.manifest.v0 .\prepared-share\tiny-test-model-q8\tiny-test-model-q8.peer-pieces 127.0.0.1:8123
cargo run -p minimodel -- write-catalog-entry examples\tiny-test-model-q8.minimodel.manifest.v0 examples\tiny-test-model-q8.minimodel-peer-offer examples\catalog-entry.v0.json
cargo run -p minimodel -- write-catalog-entry examples\tiny-test-model-q8.minimodel.manifest.v0 examples\tiny-test-model-q8.minimodel-peer-offer examples\catalog-entry.with-peer.v0.json tinyrustlm/tiny-test-model-q8 http://127.0.0.1:8123/
cargo run -p minimodel -- add-peer-source examples\catalog-entry.v0.json examples\catalog-entry.later-peer.v0.json http://127.0.0.1:8123/
cargo run -p minimodel -- write-peer-source-announcement examples\catalog-entry.v0.json examples\catalog-entry.peer-source-announcement.v0.json http://127.0.0.1:8123/
cargo run -p minimodel -- apply-peer-source-announcements examples\catalog-entry.v0.json examples\catalog-entry.announced-peer.v0.json examples\catalog-entry.peer-source-announcement.v0.json
cargo run -p minimodel -- write-catalog-snapshot-with-announcements .\models.v0.json examples\catalog-entry.v0.json --announcements examples\catalog-entry.peer-source-announcement.v0.json
cargo run -p minimodel -- write-peer-source-announcement-feed .\peer-source-announcements.v0.json examples\catalog-entry.peer-source-announcement.v0.json
cargo run -p minimodel -- write-peer-source-announcement-feed .\peer-source-announcements.empty.v0.json
cargo run -p minimodel -- write-catalog-snapshot-with-announcement-feed .\models.v0.json peer-source-announcements.v0.json .\peer-source-announcements.v0.json examples\catalog-entry.v0.json
cargo run -p minimodel -- write-catalog-snapshot .\models.v0.json examples\catalog-entry.v0.json
cargo run -p minimodel -- inspect-catalog-snapshot .\models.v0.json
```

The receipt stores the artifact basename only. It must not store private absolute paths, public model-byte URLs, or endorsement claims.

## Later Networking Boundary

Peer transfer can reuse the same manifest, peer-status, peer-piece, direct-HTTP serve/fetch, companion-state, and receipt fields, but it must not change the local verification contract. The first `inspect-peer-companion-state`, `serve-peer-pieces`, `serve-peer-companion`, `check-peer-status`, and `fetch-peer-pieces` commands are intentionally narrow: they are for known consenting peers or local test endpoints, not MiniModel.org hosting. A future peer app may add identity exchange, peer discovery, relays, NAT traversal, resumable transfer, and cache policy; MiniModel.org servers still publish only metadata, schemas, public key routes, and takedown/contact routes.
