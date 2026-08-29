# MiniModel.org Catalog Boundary

MiniModel.org is the metadata catalog and discovery surface for qualified tiny
model compositions. It is not a model host, inference service, conversion
service, or model-quality authority.

## Model-Byte Boundary

MiniModel.org supports direct P2P discovery, but it does not proxy model bytes.
Exact qualified composition bytes move only between consenting peers. If an
exact qualified peer composition is unavailable, an installed local companion
may acquire a pinned public source and perform an explicit local conversion.
Project sites do not receive model bytes or prompts.

The current TinyRustLM composition format is SLM2. Catalog metadata must bind
the exact composition identity, member sizes and digests, source revision,
license and provenance, qualification claims, runtime requirements, and the
cryptographic piece/manifests needed for verification. An alias must not point
to different bytes.

## Catalog Claims

A catalog row is metadata, not proof of availability or quality. The following
claims remain independent:

- source and license binding;
- structural composition acceptance;
- numerical parity;
- model-quality qualification;
- current peer availability;
- outside-network transfer proof;
- browser activation; and
- public site deployment and readback.

An empty or expired announcement set is valid metadata but does not make a
model available. Loopback, LAN, fixture, mocked, or source-only evidence does
not establish a public P2P lane.

## Transfer And Installation

Direct peer import requires exact composition and piece-set agreement before
mixing pieces, bounded concurrency, per-piece cryptographic verification,
durable progress, final whole-member verification, and atomic activation. A
piece match alone does not establish qualification.

The local-conversion path must pin and verify the public source revision,
respect source licensing, expose storage and resource requirements, execute a
bounded registered conversion, and run independent inspection before
installation. There is no remote-inference fallback and no instruction-file
download masquerading as model acquisition.

## Privacy

Catalog and peer protocols expose only required public-safe metadata. Logs and
receipts must exclude prompts, local paths, credentials, invitation URLs,
tokens, and raw private evidence. MiniModel.org does not receive TinyRustLM
conversation content.

## MemoryEndpoints Ecosystem Policy

MemoryEndpoints.com has two separate authorized uses elsewhere in the
ecosystem:

1. Agents working on all maintained sites use it directly for agent-to-agent
   long-term memory, meetings, handoffs, and coordination.
2. Separately, the TinyRustLM.com Connect plugin lets a user sign in and
   explicitly select MemoryEndpoints.com as TinyRustLM.com Connect long-term
   memory.

Those lanes keep identity, credentials, consent, scopes, provenance, and
receipts separate. MemoryEndpoints.com is not a MiniModel model-byte transport,
peer relay, catalog authority, or inference service. Prelaunch memory may also
be duplicated in current `.uai`, wiki, public-safe source notes, and approved
private stores only while every maintained copy remains current, secret-safe,
provenance-labeled, and noncontradictory.

## Publication

- Last published: `1.1.1` on 2026-08-29.
- Release date: 2026-08-29.
- Update: obsolete pre-SLM2 executable source removed; current direct-P2P,
  SLM2, cryptographic verification, privacy, and two-lane MemoryEndpoints
  boundaries remain documented.

Publication requires a clean committed revision, complete site gates,
deployment from that revision, and exact public HTTPS readback. Source control
or a reachable development page is not publication evidence.
