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

Every catalog document fixes `browser_admission_schema` to
`tinyrustlm.browser-composition-admission/1`. Every qualified entry must carry
one exact `browser_admission` descriptor containing only a safe same-origin
metadata route, its positive byte count of at most 64 KiB, and the lowercase
prefixed SHA-256 digest of its canonical bytes. TinyRustLM fetches that metadata
separately and must verify the declared length, digest, canonical serialization,
and schema before browser activation. The admission is metadata, not a seventh
SLM2 member. Serving it does not permit MiniModel.org to serve or proxy model
bytes. The empty prelaunch catalog publishes the schema identity but no entry,
descriptor, claim, or evidence.

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

Every admitted peer announcement must set `protocol_profile` to exactly
`tinyrustlm.direct-composition-http.v1`, the browser-consumable direct
composition HTTP profile. Former names, aliases, version fallbacks, and
compatibility negotiation are not accepted.

Its `peer_base_url` must be an exact root origin: an optional trailing `/` is
canonicalized, but credentials, endpoint paths, query strings, and fragments
are rejected. Public peer URLs require HTTPS and HTTP is loopback-only. Public
HTTPS peers require a public, non-reserved DNS name or IP literal; project and
service hosts cannot serve peer model bytes. Development loopback HTTP is
limited to canonical `localhost`, `127.0.0.1`, or `[::1]` with a port from 1
through 65535. Loopback admission does not establish public reachability or
outside-network P2P evidence.

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

- Prepared source release: `1.1.2`, dated 2026-08-31.
- Prepared status: not published or deployed.
- Last published: `1.1.1` on 2026-08-29.
- Update: public peer URLs require HTTPS and HTTP is loopback-only; exact
  root-origin and authority checks are fail-closed.
- Inventory remains exactly zero qualified catalog entries and zero current
  peer announcements.

Publication requires a clean committed revision, complete site gates,
deployment from that revision, and exact public HTTPS readback. Source control
or a reachable development page is not publication evidence.
