# Host metadata renewal

In progress; not published and no production key exists in this source bundle.

The fixed PHP handler serves only the existing catalog and peer-feed URLs. A publisher must first validate the initial catalog, exact six-member seed, admission, qualification and license, prove HTTP denial in this distinct private namespace, and install the private configuration, initial signed pair, zero-byte lock and a new 32-byte secret seed through the authorized secure transport. No key is created by a request.

Configuration schema minimodel.metadata_renewal_config.v1 contains exact initial_catalog and initial_peer_feed canonical JSON+LF strings; release_owner (site, release_id, source_commit, publisher_commit, manifest_sha256, authority_sha256); source_references (relative path, bytes, sha256); ttl_seconds 86400 and refresh_before_seconds 21600. References bind all six seed members and immutable admission/qualification/license evidence; the publisher validates this closure before installation. Pair schema minimodel.metadata_pair.v1 contains config_sha256, catalog and peer_feed strings. The sole operator seed and public verification key remain unchanged during renewal.

Near expiry, the handler takes a local lock and the shared deployment directory lease, hashes the referenced files, updates only revisions and validity timestamps, and signs the peer announcement. File and directory fsync, a pending journal and atomic pair replacement provide recoverable publication. An ambiguous interrupted transaction retains its files and lease for operator review. No force/retry/overwrite endpoint exists. A failed renewal can serve only the previously verified pair while it remains valid; an expired pair returns 503.

An atomic disk pair does not make two client HTTP requests atomic: the browser must perform one bounded pair reload if a renewal occurs between them. This is metadata recovery, never an inference retry. Key rotation, model changes, revocation, crash recovery and deployment integration are separate explicit publisher operations; this code cannot qualify a model or change consent.

Local tests must exercise real PHP/libsodium and unchanged JavaScript signature/catalog readers. Host capability and private HTTP denial remain independently required before publication.
