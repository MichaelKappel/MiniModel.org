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
3. The optional `.tinyrustlm-convert.ps1` script runs `tinyrustlm-slm-pack convert-huggingface-request` with the saved request plus a user-owned local `.safetensors` file or shard directory. GGUF source rows use a separate review-first browser kit with `source_format=gguf` and `TINYRUSTLM_SOURCE_GGUF`; it can run the local `tinyrustlm-slm-pack inspect-gguf`, `plan-gguf-source`, `extract-gguf-source`, and `convert-gguf-trained` gates when tensors are supported F32/F16/BF16/Q2_K/Q3_K/Q4_0/Q4_1/Q4_K/Q5_0/Q5_1/Q5_K/Q6_K/Q8_0/Q8_1/Q8_K payloads. GGUF conversion still requires supported tensor names/shapes, tokenizer token-count compatibility, and Rust validation before .slm import; `plan-gguf-source` fails closed when `tokenizer.ggml.tokens` contradicts the target vocab, and BPE source shapes import `tokenizer.ggml.tokens`/`tokenizer.ggml.merges` into the raw source manifest when both are declared.
4. Conversion happens outside project servers, using reviewed source bytes, config, tokenizer, license, and model-card metadata. The generated script does not download bytes and does not call a project proxy.
5. The converter writes a local `.slm`, validates it with the Rust packer/verifier, and records a checksum-bound conversion receipt plus runtime-smoke and eval/admission evidence.
6. The converter runs `minimodel prepare-peer-share <artifact.slm> <output-dir> <model-id> [source-huggingface-model-id] [known-peer-base-url]`.
7. `prepare-peer-share` validates the `.slm`, writes a MiniModel manifest, writes a `.minimodel-peer-offer` proof, exports local peer pieces, and writes catalog JSON in one output directory.
8. The converter can also run `minimodel write-peer-contact-card <output-json> <known-peer-base-url> [peer-label]` to share a metadata-only reviewed peer URL card with a receiver, or `minimodel write-peer-contact-book <output-json> <contact-card-json>...` to bundle a small reviewed list of cards with per-card SHA-256s. TinyRustLM can load a card or contact book to set its Known peer URL, but neither artifact is catalog publication and neither embeds model bytes or peer-piece bytes.
9. If a consenting peer URL becomes known after the proof-ready entry exists, the converter can run `minimodel write-peer-source-announcement <catalog-entry-json> <announcement-json> <known-peer-base-url>` and send the announcement metadata to a catalog maintainer, or run `minimodel add-peer-source <catalog-entry-json> <output-json> <known-peer-base-url>` for a direct local entry update.
10. A catalog maintainer can run `minimodel apply-peer-source-announcements <catalog-entry-json> <output-json> <announcement-json>...` to validate model/artifact/proof identity, deduplicate peer URLs, and write `peer_sources` without touching model bytes.
11. For many entries or many announcement files, a catalog maintainer can run `minimodel write-catalog-snapshot-with-announcements <output-json> <catalog-entry-json>... --announcements <announcement-json>...` to validate matching announcements and write the final metadata-only snapshot in one checked command.
12. For browser-visible late availability without rewriting catalog entries, a catalog maintainer can run `minimodel write-peer-source-announcement-feed <output-json> <announcement-json>...`, preflight the result with `minimodel inspect-peer-source-announcement-feed <feed-json>`, then bind that feed into a snapshot with `minimodel write-catalog-snapshot-with-announcement-feed <output-json> <feed-route> <announcement-feed-json> <catalog-entry-json>...`. The snapshot records the feed route and SHA-256 only; it does not inline peer chunks or project-hosted artifacts.
13. The converter or maintainer can run `minimodel write-catalog-snapshot <output-json> <catalog-entry-json>...` to merge one or more generated entries into a metadata-only catalog snapshot after Rust rejects artifact URLs, project-server byte/execution claims, duplicate model ids, peer-source count drift, and project/Hugging Face peer hosts.
14. Before publishing or loading an existing snapshot, the converter or maintainer can run `minimodel inspect-catalog-snapshot <catalog-json>` to revalidate the metadata-only boundary, duplicate model ids, peer sources, checksum-bearing announcement-feed routes, and exact catalog SHA-256.
15. MiniModel.org may list the resulting metadata with `source_huggingface_model_id`, artifact byte count, artifact SHA-256, manifest route, evidence routes, `peer_offer_route`, `peer_offer_sha256`, `peer_offer_merkle_root_sha256`, `peer_hint_status`, `peer_import_status`, optional `peer_source_count` / `peer_sources`, and optional same-origin `peer_source_announcement_routes` when consenting direct peer availability is known through a validated entry or announcement feed.
16. P2P transfer may use the metadata and chunk/Merkle fields so later users can obtain verified `.slm` bytes from consenting peers instead of repeating a large source download and conversion.

Until that chain exists for a model, the honest status is `Rust conversion required`.

## Local Share Metadata Bundle

TinyRustLM may export `minimodel.local_slm_share_metadata.v0` after a user imports a verified local `.slm`. This bundle is the first practical bridge from "converted once" to later P2P availability:

- `catalog_entry` records the artifact byte count, artifact SHA-256, local filename hint, source identity, optional `source_huggingface_model_id`, and no-server-byte flags.
- `peer_offer` records chunk size, chunk count, per-chunk SHA-256 values, chunk-list SHA-256, and a Merkle root.
- `Save Share Metadata` must stay metadata-only with `boundary.peer_transfer_implemented=false`, no transfer protocols, no share-kit descriptor, and no project-server byte claims.
- `Save P2P Share Kit` may write a second bundle with `boundary.peer_transfer_implemented=true`, `peer_transfer_implementation=generated-local-minimodel-share-kit-v0`, direct MiniModel protocol hints, `local_share_kit_available=true`, and a `tinyrustlm.local_p2p_share_kit.v0` descriptor for local scripts.
- The share-kit descriptor may advertise `writes_import_receipt_ledger=true` only when the generated local script writes and inspects a sender-side `.minimodel-receipt` plus metadata-only `import-receipts.v0.json` before peer availability metadata.
- `peer_offer.transfer.network_transfer_available` must remain `false` for both bundle shapes until the user runs local scripts, sets a consenting direct peer URL, and a receiver uses Rust `import-peer-request` to reverify chunks.
- `minimodel inspect-share-metadata <bundle-json>` validates both bundle shapes, rejects project-server byte claims, recomputes chunk-list SHA-256 and Merkle root, and checks that a share-kit-ready bundle still does not claim live network transfer.

The simplest sender path is now:

```text
minimodel prepare-peer-share <artifact.slm> <output-dir> <model-id> [source-huggingface-model-id] [known-peer-base-url]
```

That command writes the MiniModel manifest, `.minimodel-peer-offer`, local peer pieces, and catalog entry together after validating the `.slm`. Catalog entries may reference the peer-offer text manifest with `peer_offer_route`, `peer_offer_sha256`, `peer_offer_chunk_count`, and `peer_offer_merkle_root_sha256`, but a peer-offer route is not a model download route and does not mean project servers serve bytes.

TinyRustLM may also save a local `.minimodel-share-kit.ps1` beside the metadata bundle after a verified `.slm` import. The script is a convenience wrapper around the same MiniModel verifier commands: it resolves `MINIMODEL_WORKSPACE` and `TINYRUSTLM_LOCAL_SLM`, verifies expected artifact byte count and SHA-256 before running, calls `prepare-peer-share`, writes and inspects a local import receipt plus sender-side `import-receipts.v0.json`, optionally writes peer-source announcement/feed metadata, `write-peer-companion-state` metadata, inspects that companion state with `inspect-peer-companion-state`, writes `write-peer-contact-card` and `write-peer-contact-book` metadata when a consenting direct peer URL is set, writes and inspects `write-peer-companion-inventory` metadata, writes and inspects metadata-only catalog snapshots, and prints sender-side `serve-peer-companion` or `serve-peer-pieces` commands. The contact book and companion inventory are still only metadata-only Known peer/local companion handoffs, and they require Rust reverify before TinyRustLM or a companion UI trusts them. The script does not upload `.slm` bytes or peer pieces to MiniModel.org, TinyRustLM.com, or any project server.

MiniModel can turn an admitted manifest plus peer-offer proof into metadata with:

```text
minimodel write-catalog-entry <manifest> <peer-offer> <output-json> [source-huggingface-model-id] [known-peer-base-url]
```

The command rejects drift between the manifest and proof before writing JSON. Its output may set `peer_hint_status` to `peer-offer-ready-known-peer-needed` and `peer_import_status` to `request-script-ready`, and MiniModel now requires import-ready or peer-sourced rows to include the complete peer-offer proof envelope: kind, route, proof SHA-256, chunk algorithm, chunk size, chunk count, and Merkle root. Without a known peer URL, that status means TinyRustLM can offer a P2P import request/script after the user enters one consenting peer URL. With `[known-peer-base-url]`, the command validates the peer URL, rejects project/Hugging Face host families, and writes `peer_source_count=1` plus a `peer_sources` row so TinyRustLM can show `P2P Import` immediately while still saving a Rust reverify request/script instead of fetching bytes in the browser.

MiniModel can attach peer-source metadata after conversion when the converter did not know the public peer endpoint yet:

```text
minimodel add-peer-source <catalog-entry-json> <output-json> <known-peer-base-url>
```

The command validates the existing entry, rejects project/Hugging Face peer hosts, writes one consenting direct-peer row, and revalidates the result before writing. This is the low-friction "converted once, listed as P2P-ready later" step.

For catalog publication, a converter can publish a metadata-only peer-source announcement and a maintainer can merge it into the proof-ready entry:

```text
minimodel write-peer-source-announcement <catalog-entry-json> <announcement-json> <known-peer-base-url>
minimodel apply-peer-source-announcements <catalog-entry-json> <output-json> <announcement-json>...
minimodel write-catalog-snapshot-with-announcements <output-json> <catalog-entry-json>... --announcements <announcement-json>...
minimodel write-peer-source-announcement-feed <output-json> [announcement-json...]
minimodel inspect-peer-source-announcement-feed <feed-json>
minimodel write-catalog-snapshot-with-announcement-feed <output-json> <feed-route> <announcement-feed-json> <catalog-entry-json>...
minimodel inspect-catalog-snapshot <catalog-json>
```

The announcement path lets a converter hand over a small JSON file instead of a rewritten catalog entry or model bytes. The apply command validates the announcement against the catalog entry, including peer-offer route, SHA-256, chunk count, and Merkle root, and produces the P2P-ready entry used by `write-catalog-snapshot`. TinyRustLM can also apply a local peer-source announcement JSON to a loaded proof-ready `models.v0.json` snapshot for local review and import-request generation; that browser overlay still validates only metadata, never fetches model bytes, and still hands the receiver to Rust `import-peer-request`.

`write-peer-source-announcement-feed` is the durable publication path for zero or more portable availability files. It writes `minimodel.peer_source_announcement_catalog` JSON after validating each announcement, rejecting duplicate model/peer rows, and recording `project_server_serves_model_bytes=false`, `project_server_runs_model_bytes=false`, and `project_server_proxies_model_bytes=false`. An empty feed is a valid checked state and does not make any row P2P-ready. `inspect-peer-source-announcement-feed` runs those same feed-envelope checks against an existing file, rejects feed JSON above 4 MiB before parsing, reports announcement/model counts plus exact feed bytes, and hashes those exact bytes before the file is handed to TinyRustLM or bound into a catalog route. `write-catalog-snapshot-with-announcement-feed` applies the same 4 MiB feed budget, then computes the feed SHA-256 from the exact feed bytes and writes a catalog route row.

A catalog snapshot may also declare checksum-bound peer-source announcement feed routes for the browser to verify automatically. The accepted fields are `peer_source_announcement_route` plus `peer_source_announcement_sha256`, or `peer_source_announcement_routes` / `peer_source_announcement_feeds` arrays containing `route` and SHA-256 fields. The Rust snapshot command writes the array shape with `route`, `sha256`, and `route_kind=minimodel.peer_source_announcement_catalog`. TinyRustLM resolves each route relative to the loaded catalog URL, requires it to stay on the same origin as the catalog, verifies the fetched JSON SHA-256, validates the metadata-only feed envelope and duplicate model/peer rules, then applies the announcement records with the same proof-ready catalog matching rules. The feed is still metadata only: a failed feed does not create peer sources, a passing feed does not fetch `.slm` bytes, and Rust `import-peer-request` remains the transfer and receipt boundary.

MiniModel can merge generated entries into a metadata-only catalog snapshot with:

```text
minimodel write-catalog-snapshot <output-json> <catalog-entry-json>...
minimodel inspect-catalog-snapshot <catalog-json>
```

The snapshot writer validates every entry again and rejects model-byte URLs, project-server byte/execution flags, duplicate model ids, peer-source count drift, unsupported peer-source rows, and project/Hugging Face peer hosts. The inspector rejects catalog JSON above 4 MiB before parsing, revalidates an existing `models.v0.json`, checks checksum-bearing announcement-feed route rows, and reports the exact catalog byte count plus SHA-256 before the file is shared or loaded. This is the safe "converted once, discoverable later" metadata path; it does not publish `.slm` bytes and does not make MiniModel.org a peer relay.

The MiniModel verifier can now export local peer pieces after verification:

```text
minimodel export-peer-pieces <manifest> <artifact.slm> <output-dir>
minimodel serve-peer-pieces <manifest> <piece-dir> <bind-addr>
minimodel serve-peer-companion <state-json> <manifest> <piece-dir> <bind-addr>
minimodel check-peer-status <manifest> <known-peer-base-url> <output-json>
minimodel inspect-peer-status-check <status-check-json> <manifest>
minimodel inspect-peer-import-request <request-json> <manifest>
minimodel fetch-peer-pieces <manifest> <http-peer-base-url> <output-dir>
minimodel assemble-peer-pieces <manifest> <piece-dir> <output.slm>
minimodel import-peer-request <request-json> <manifest> <output-dir>
minimodel inspect-import-receipt <receipt.minimodel-receipt> <manifest> <artifact.slm>
minimodel inspect-peer-import-receipt <receipt.minimodel-receipt> <request-json> <preflight> <manifest> <artifact.slm>
minimodel write-import-receipt-ledger <output-json> <receipt> <manifest> <artifact.slm> [<receipt> <manifest> <artifact.slm>...]
minimodel inspect-import-receipt-ledger <ledger-json>
```

This creates, serves, fetches, and consumes a local `minimodel.peer_piece_set.v0` sidecar plus chunk files. `serve-peer-pieces` is sender-side only: it validates the local peer-piece directory and serves only `minimodel.peer_status.v0.json`, the declared sidecar, and declared piece files from a user-started HTTP listener. `serve-peer-companion` first validates metadata-only companion state against the manifest and peer-piece directory, then serves the same status route plus the fixed `minimodel.peer_companion_state.v0.json` route along with the same allow-listed piece-set and chunk routes. If the validated peer URL includes a path prefix, the routes are served under that prefix. Browser metadata routes include CORS/PNA headers for TinyRustLM `Check Peer` and future metadata preflight, but declared chunk routes remain non-CORS so web pages cannot read model-piece payloads directly. Known peer URLs must be plain direct `http://` URLs with DNS/IPv4 host text and safe absolute paths; project and Hugging Face canonical/www/subdomain/root-dot hosts, malformed dot hosts, query/fragment URLs, traversal, whitespace, and control characters are rejected before manifest reads, output writes, or peer fetches. `inspect-peer-import-request` is receiver-side and offline: it validates a saved TinyRustLM request JSON against the metadata manifest, rejects drift in optional structured `metadata_routes` and `receiver_validation_contract` fields when present, reports route/gate validation status plus request/manifest SHA-256, and performs no peer HTTP access. `check-peer-status` is receiver-side metadata only: it fetches the status route, validates model/artifact/piece-set identity and no-project-server/no-embedded-byte flags against the manifest, and writes a readiness report before any piece bytes move. `inspect-peer-status-check` is the matching offline gate for that saved report: it verifies the check kind/version, strict UTC status, peer/status routes, manifest/artifact/piece hashes, no-piece/no-artifact/no-project-server-byte flags, and the Rust-reverify requirement without contacting the peer. `preflight-peer-import-request` records the request `metadata_routes` and `receiver_validation_contract` status in its `.preflight` report while still fetching only peer metadata, and `inspect-peer-import-preflight` revalidates that saved report against the request and manifest before any piece bytes move. `fetch-peer-pieces` is receiver-side: it downloads the sidecar, validates advertised companion state when present, downloads listed chunks from a user-provided `http://` peer endpoint, rejects project/Hugging Face hostnames, and verifies every piece checksum before writing the local output directory. The sidecar proves what a peer transport should carry, and `assemble-peer-pieces` re-runs full Rust artifact verification before writing the accepted `.slm`. `import-peer-request` is the simplified receiver command for TinyRustLM handoff files: it validates the request against the manifest, performs the same companion-aware known-peer fetch, assembles, verifies, and writes the import receipt into one local output directory. `inspect-import-receipt` reparses that receipt, revalidates the manifest and assembled `.slm`, and confirms the peer-transfer evidence plus no-project-server-byte flags still match. `inspect-peer-import-receipt` is the final receiver-side gate for generated P2P scripts: it binds that same receipt back to the saved request JSON and saved preflight report so receiver-validation contract gates, peer URL, companion-state URL/status, piece-set SHA-256, artifact SHA-256, chunk/Merkle evidence, fetched-piece flags, strict UTC creation timestamps, receipt-after-preflight order, and no-project-server-byte flags all agree before browser import. `write-import-receipt-ledger` and `inspect-import-receipt-ledger` turn those verified receipts into a local metadata-only companion index, then re-open every receipt/manifest/artifact to catch stale or drifted entries. The receipt and ledger record direct peer transport, validated receiver-validation contract status/gates, peer URL, piece-set SHA-256, companion-state status, chunk count, Merkle root, fetched peer-piece bytes, assembled-from-peer-pieces status, and no-project-server-byte flags. The chunk files are model bytes and must not be uploaded to MiniModel.org.

The protected TinyRustLM NuGet SDK exposes a narrower public ecosystem path for .NET applications: it can prepare a local share kit, inspect a metadata-bound P2P manifest, assemble already-local manifest-declared pieces into a verified `.slm`, write `<output.slm>.p2p-import-receipt.json`, and inspect that receipt sidecar against a caller-supplied local `.slm` with no network fetch, peer discovery, private routing, conversion, or model-ranking logic. That SDK receipt is useful for app-level adoption and local audit trails; the MiniModel CLI remains the fuller direct known-peer verifier for peer status, preflight, companion-state validation, import-receipt chain binding, and receipt ledgers.

The same protected SDK release evidence now carries matching `.snupkg` symbol package files for the audited package set. Publish plans record `symbol_publish_mode=explicit_snupkg_push_after_primary_package`, and the SHA256SUMS manifest is symbol-hash-bound so consumers can inspect symbols without exposing private conversion, routing, ranking, or training internals.

Publication status as of 2026-07-07: local package-folder feed-current and SHA256SUMS-bound package-hash verification pass for the protected source-level `TinyRustLM.Abstractions`, `TinyRustLM.Core`, `TinyRustLM.P2P`, and `TinyRustLM.Memory` lane at source version `0.1.8`. Public NuGet.org availability is currently carried by the branded `UAIX.Browser.Core.Abstractions`, `UAIX.Browser.p2pLLM.Runtime`, `UAIX.Browser.p2pRuntime.P2P`, `UAIX.Browser.Memory`, and `UAIX.Browser.p2pLLM.Conversion` package IDs, all published and post-publish verified at version `0.1.8` with NuGet.org feed-current plus SHA256SUMS-bound package-byte verification gates.

The release-readiness report also lifts package count, symbol package count, `symbol_publish_mode=explicit_snupkg_push_after_primary_package`, package-and-symbol SHA256SUMS manifest hash/match status, and local feed-hash mismatch counts into top-level evidence so catalog maintainers can review NuGet package currency without opening every subreport. Browser readiness audits write their no-push dry-run plan to `uaix-browser-nuget-release-readiness-publish-plan-dry-run.json` by default so retained `uaix-browser-nuget-publish-plan.json` post-push evidence is not overwritten during routine freshness checks.

The bundle may be submitted for review or used by future peer software, but it is not an endorsement and not an execution grant. TinyRustLM still runs only a user-selected local `.slm` after Rust/WASM validation.

## TinyRustLM UI Mapping

TinyRustLM should expose the same source lanes in plain language:

- `Your Local Files`: first-class runnable path through the browser file picker.
- `Hugging Face`: user-triggered metadata discovery, model-card review, source revision context, API endpoint awareness, conversion-request JSON, and optional local Rust conversion-script handoff into `prepare-peer-share`.
- `P2P Exchange`: MiniModel metadata, peer-offer proof, optional catalog-listed consenting peer sources, metadata-only peer contact cards and contact books for reviewed known-peer URLs, peer-source announcement metadata, companion-readable peer-share state, metadata-only peer companion inventories, direct known-peer serve/fetch, local peer-piece export/reassembly, one-command `prepare-peer-share` sender execution, metadata-only snapshots generated by `write-catalog-snapshot`, receiver-side `inspect-peer-import-request` -> `check-peer-status` -> `inspect-peer-status-check` -> `preflight-peer-import-request` -> `inspect-peer-import-preflight` -> `import-peer-request` -> `inspect-import-receipt` -> `inspect-peer-import-receipt` execution, local import receipt ledgers generated by `write-import-receipt-ledger` and revalidated by `inspect-import-receipt-ledger`, and catalog entries generated by `write-catalog-entry` for advanced/manual records now; broader peer discovery and relay flow later, still requiring local receipt and `.slm` verification before browser execution.

Advanced source details may show catalog URLs, Hugging Face search fields, checksums, manifests, evidence routes, and verifier status, but the first-load Chat mode should keep those behind progressive disclosure.

## User Consent Gates

The product flow needs two separate consent moments:

1. Transfer consent: the user chooses to obtain bytes from a peer or from a source outside MiniModel.org.
2. Execution consent: the user chooses to open a verified local `.slm` file in TinyRustLM.

The metadata catalog must not collapse either step into an automatic action.

## Optional Peer Companion Boundary

The optional MiniModel Peer Companion is a later user-run desktop app for advanced peer duties, not a hosted MiniModel.org service. Its useful jobs are local peer identity, local piece storage, explicit serve/fetch controls, cache quotas, consent prompts, import receipts, metadata-only receipt ledgers, metadata-only peer companion inventories, and local logs. It should reuse the Rust verifier crates and command contracts instead of duplicating trust decisions in a UI shell.

`minimodel.peer_companion_inventory.v0` is a local metadata summary for that future UI. It binds companion state, manifest and piece metadata, receipt-ledger evidence, piece-store byte/quota/SHA-256 checks, optional contact-card metadata, and a `serve_plan` that names the direct HTTP status/piece/state routes plus required Rust preflight, but it still contains no model bytes and no peer-piece payloads. Inventory write/inspect gates reject duplicate contact-card peer URLs so one endpoint cannot appear as multiple available peers.

TinyRustLM may preview that inventory from a user-selected local JSON file to show model id, piece-store quota, piece-store SHA-256, receipt, contact-card, serve-plan status route, and SHA-256 metadata. That preview is not a catalog availability claim and does not replace Rust `inspect-peer-companion-inventory`.

Tauri v2, Iced, Slint, egui, Dioxus, Iroh/QUIC, Rust-libp2p, WebRTC, WebTransport, BitTorrent-style chunking, Pear, Bare, and Hypercore remain evaluation lanes. None of them changes the core rule: project servers may publish metadata and docs, but model bytes move only through user-owned local files or consenting direct peers and are reverified locally before TinyRustLM import.

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

## Tiny Public Demo Exception

MiniModel.org now carries one narrow first-run demo lane for TinyRustLM.com: `catalog/default-demo.v0.json` declares the default MiniModel demo and points to `artifacts/demo/tiny-test-model-q8.slm`. TinyRustLM.com may fetch that tiny runtime-smoke `.slm` with credentials omitted only after validating the declared byte count, SHA-256, and optional `artifact_byte_checksum`; `slm_checksum` remains the runtime/manifest checksum. This does not make MiniModel.org a general hosted model network; user/private models, premium weights, peer pieces, converted artifacts, and broader catalog entries still use local files or P2P reverify paths.

## First Public Deliverable

The first public deliverable is not a broad hosted model network. It is a stable documentation and local-verifier set:

- manifest schema
- verifier plan
- local peer-piece export, direct peer serve/fetch, one-command import request execution, and reassembly
- metadata-only catalog rules
- example manifest
- example catalog entry
- takedown/contact route language

Only after those pieces are stable should broader peer discovery, identity exchange, relay, and cache behavior be prototyped.
