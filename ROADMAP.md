# MiniModel.org Roadmap

MiniModel.org publishes discovery metadata for exact qualified SLM2
compositions. The owner authorizes this server to provide the initial verified
seeds, with payloads only in D:\LLMs\TinyRustLM. It does not proxy arbitrary model
bytes, run inference, or perform server-side conversion.

## Current Source Requirements

- Publish the strict `minimodel.slm2_catalog.v1` catalog.
- Publish the strict `minimodel.slm2_peer_announcements.v1` feed.
- Keep both empty until exact current qualification and verified seed or consenting-peer evidence
  exists.
- Present one visible acquisition choice: one exact qualified peer or one
  explicit pinned-source local conversion.
- Retire every former manifest-v0, peer-offer, instruction-handoff, fixture,
  and single-file model route.

## Required Integration Evidence

1. Verify the canonical language-neutral syntax and semantic vectors for catalog,
   composition, piece-set, peer-announcement, and acquisition identities.
2. Verify the existing independent SLM2 validators without retaining the former
   single-file model reader as a compatibility path.
3. Exercise the canonical secure loopback companion intent/session boundary.
4. Exercise bounded direct-peer import with durable progress, cancellation,
   piece verification, final whole-member hashing, and atomic activation.
5. Exercise the Advanced pinned-source local conversion branch with exact license,
   source-lock, resource, registration, and independent-inspection gates.
6. Qualify a real small starter composition, bind an authorized initial server
   seed or consenting peer, and prove browser activation and local multi-turn
   chat before adding a catalog entry. Restore a usable cached model or acquire
   that qualified default; local files, Hugging Face, conversion and manual
   peer/IP controls remain under Advanced.
7. Commit, push, deploy, and verify exact public HTTPS bytes and retired routes.

## Current Implementation Boundary

The obsolete pre-SLM2 native CLI, its v0 fixtures, its single-file model demo,
and its cross-checkout runtime dependency are deleted from current source.
They remain available only through Git history and have no executable,
compatibility, fallback, or test authority.

This repository owns the strict SLM2 catalog, peer-feed validator and browser
surface. The acquisition companion and current Rust runtime belong to the
canonical sibling TinyRustLM.com repository, including its dotnet projects and
Rust crates. Use that implementation; do not recreate a MiniModel native
workspace. Source and fixture checks do not prove a deployed companion, a
qualified starter, a live server seed or a successful end-user acquisition.

No composition, peer, browser round, live MemoryEndpoints round, or catalog
announcement is claimed. Campaign counters remain `0/20`, `0/50`, `0/25`,
`0/40`, and `0/20`.
