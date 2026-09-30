# MiniModel host capability probe

This narrowly scoped probe establishes whether the existing host can support a
private signed-metadata renewal implementation. It does not renew metadata,
qualify a model, publish a seed, or establish browser usability.

The deployment owner installs this source closure in the previously absent
.minimodel-host-capability.v1 directory under the verified account root, holding
the existing .minimodel-deployment-lease.v1 lease. Install private/.htaccess
first and a nonsecret canary; verify that HTTPS retrieval of private/canary.txt
is denied before uploading code or operation.json. Never install a signing key
as part of this probe. An existing or partial namespace requires explicit
reconciliation rather than overwrite or blind retry.

The operator writes compact UTF-8 JSON plus LF to private/operation.json with
keys in this order: schema, operation_id, lease_owner_sha256. The schema is
minimodel.host_capability_operation.v1; operation_id is the lease owner's 32
lowercase hexadecimal identifier. The hash identifies the exact remote
.minimodel-deployment-lease.v1/owner-{operation_id}.json bytes. The canary is
minimodel-host-capability-canary:{operation_id} plus LF. All these values are
nonsecret, but private files must still be denied over HTTPS.

Request GET /.minimodel-host-capability.v1/probe.json without query parameters.
Require HTTP 200, application/json, no-store, the exact nonce, ok=true and every
named check=true. Verify private denial and exact deployed source bytes
independently over HTTPS and FTPS. JSON booleans alone do not prove the HTTP
privacy boundary. A CLI test is local validation only.

The core checks PHP >=8.1, libsodium sign/verify using an explicitly public fixed
test seed, bounded private reads, exclusive scratch creation, write/flush/fsync,
contended flock, rename/readback and exact owned-file retirement. It revalidates
the live deployment lease before mutations. A failed or ambiguous scratch
operation is retained. The probe never contacts a network, accepts a filesystem
path over HTTP, exposes environment values, or uses production keys.
The `flush` check requires both file fsync and directory fsync after rename;
file-only durability is insufficient for the planned atomic metadata renewal.
The `atomicRename` check replaces an existing exact owned old payload with the
staged new payload, then verifies its new inode and bytes. Rename to an absent
name alone does not establish the replacement required by metadata renewal.

The local test reads canonical PHP source through a read-only Docker mount and
uses the already installed php:8.3-cli image with networking disabled. It does
not pull an image or copy a source checkout. Enable that execution with
MINIMODEL_HOST_CAPABILITY_PHP_DOCKER=1. The test reports an explicit skip when
the opt-in runtime is unavailable; this is never production capability evidence.

The admitted live probe completed on 2026-09-30 at 00:31:51 UTC. Its
[append-only receipt](../receipts/site-deployment-20260930T002007Z.json) records
all ten capability checks as true, including file/directory fsync and replacement
of an existing owned file. Private denial, exact namespace removal and lease
release were verified. The temporary endpoint was removed; this is a recorded
observation, not a currently deployed health endpoint.

No signing key, model, discovery metadata or website release was published.
The eventual renewal namespace must prove its own HTTP denial before receiving
a key, and actual qualified-model publication and renewal remain separate gates.
