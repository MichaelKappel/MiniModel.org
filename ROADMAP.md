# MiniModel.org Roadmap

MiniModel.org is the metadata-only catalog for exact qualified SLM2
compositions. It does not host model members, proxy model bytes, run inference,
or perform server-side conversion.

## Current Public Release

- Publish the strict `minimodel.slm2_catalog.v1` catalog.
- Publish the strict `minimodel.slm2_peer_announcements.v1` feed.
- Keep both empty until exact current qualification and outside-peer evidence
  exists.
- Present one visible acquisition choice: one exact qualified peer or one
  explicit pinned-source local conversion.
- Retire every former manifest-v0, peer-offer, instruction-handoff, fixture,
  and single-file model route.

## Required Implementation Sequence

1. Generate language-neutral syntax and semantic vectors for catalog,
   composition, piece-set, peer-announcement, and acquisition identities.
2. Implement independent SLM2 validators without retaining the former
   single-file model reader as a compatibility path.
3. Implement the secure loopback companion intent/session boundary.
4. Implement bounded direct-peer import with durable progress, cancellation,
   piece verification, final whole-member hashing, and atomic activation.
5. Implement the pinned-source local conversion branch with exact license,
   source-lock, resource, registration, and independent-inspection gates.
6. Prove one real outside peer lane, one real local conversion, browser
   activation, and local multi-turn chat before adding a catalog entry.
7. Commit, push, deploy, and verify exact public HTTPS bytes and retired routes.

## Current Gaps

The current native MiniModel CLI predates the SLM2 clean-slate contract and is
not publication or compatibility authority. Replacing it is separate
implementation work requiring generated vectors, Rust tests, hostile-input
coverage, and a clean deletion of obsolete executable paths.

No composition, peer, browser round, live MemoryEndpoints round, or catalog
announcement is claimed. Campaign counters remain `0/20`, `0/50`, `0/25`,
`0/40`, and `0/20`.
