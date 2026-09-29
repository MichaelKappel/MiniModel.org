# Static seed preparation

`tools/build-static-seed-plan.js` compiles one current Rust/companion seed
descriptor into six immutable public member URLs and a hash-bound `.htaccess`
configuration. The input is at most 8 KiB and must match the sole canonical
compact UTF-8/LF representation. It rejects unsupported profiles, extra or
duplicate fields, wrong ordering, unsafe names, invalid hashes and oversized
members. The generated piece-set ID is the exact descriptor SHA-256, matching
TinyRustLM's native seed contract corrected in commit `8515972`.

The module API accepts a Buffer and returns the preparation record. The CLI
reads exact bytes from stdin and emits one JSON record on stdout. It accepts no
arguments. From a shell supporting byte-preserving stdin redirection:

```text
node tools/build-static-seed-plan.js < canonical-seed-descriptor.json
```

Windows PowerShell text pipelines can change encoding/newlines. An existing
Node caller can read the exact descriptor bytes into a Buffer and call
`buildStaticSeedPlan(bytes)` without making a source or model copy. The tool
does not read model files, write configuration, sign announcements, renew
catalog authority or contact a server. `publicationAllowed` is always false.
This record is preparation metadata, not a qualification or publication receipt.

The intended remote directory is
`tinyrustlm/direct-composition/v1/compositions/sha256:{graphDigest}/members`.
It contains the six fixed member names and the generated `.htaccess`. The colon
is part of the existing browser URL contract; the remote filesystem must admit
it. These remote paths must never be passed to a Windows local staging path
API. Developer payloads remain in `D:\LLMs\TinyRustLM`. A necessary remote
serving copy has an explicit deployment lifecycle, separate from local storage.

The configuration requires working per-directory rewrite/header/type/options
overrides. It disables listings and content negotiation, admits GET/HEAD for the
six filenames, rejects query strings and other filenames/methods, sets the
binary media type, requests compression suppression, and exposes exact response
identities to `https://tinyrustlm.com`. CORS grants no credential access. The
server must compute Content-Length from actual bytes; the configuration never
forges it or removes Content-Encoding to conceal compression. Browser-managed
composition storage owns persistence and retirement, so these responses use
`no-store, no-transform` to avoid another retained HTTP-cache payload.

The plan also returns `requiredServerConfiguration` with `TraceEnable Off`,
explicitly marked unverified. Apache handles TRACE before directory rewrite and
header rules; `.htaccess` alone cannot enforce the GET/HEAD-only contract. This
directive belongs in the server or virtual-host configuration, never in the
generated `.htaccess`. The host must establish an equivalent rejection if its
implementation differs. See [Apache TraceEnable](https://httpd.apache.org/docs/2.4/mod/core.html#traceenable),
living documentation retrieved September 19, 2026. Do not treat FTPS upload access
as proof that this server setting is controlled or already correct.

Apache documents separate `onsuccess` and `always` header tables; the generated
configuration clears one and sets the other to avoid inherited duplicate
identities. [Apache mod_headers](https://httpd.apache.org/docs/2.4/mod/mod_headers.html)
and [core directives](https://httpd.apache.org/docs/2.4/mod/core.html), living
documentation retrieved September 19, 2026. LiteSpeed documents `no-gzip` and
`no-brotli`, and a difference in rewrite `[L]` processing. This generator uses
terminal error rules without internal rewrites.
[LiteSpeed configuration](https://docs.litespeedtech.com/lsws/configuration/),
[compression](https://docs.litespeedtech.com/lsws/cp/cpanel/compression/) and
[rewrite differences](https://docs.litespeedtech.com/lsws/cp/cpanel/switch-apache/),
retrieved September 19, 2026. Documentation support does not establish the
actual host's module availability, override permissions or behavior.

The existing site publisher still accepts only its reviewed 22-file web closure.
This tool does not extend that closure or bypass target and catalog admission.
The transport tuple is now bound, but the public catalog remains empty/expired.
Current registered-root source-authority admission has passed;
that result does not establish host ownership or seed availability. A seed publication route
must separately admit exact target ownership and hashes, stream verified bytes
from the canonical model store, and verify remote readback before advertising
availability. Existing source, site, seed and qualification records remain distinct.

The September 29 seed core adds a deliberately empty
`deploy/static-seed-namespace.v1.json`. Its compiler uses the existing descriptor
parser and derives exactly six member paths plus the generated `.htaccess`.
Registration means an exact path set is available for review; it does not mean
the files exist, are qualified, or may be advertised. The custody module can
validate that exact colon-bearing namespace while its normal website and local
source path grammar remains unchanged. A complete expected directory tree
rejects missing members and unknown entries. The existing website entrypoint
does not yet load the registry or publish seeds.

`MiniModel.StaticSeed.psm1` provides a 64 KiB streaming identity check. It can
hash without a destination, or hash while copying into a caller-owned stream;
short bodies, extra bytes and digest disagreement fail. It leaves stream
lifetime and transport admission to the caller. A failed copy can already have
written partial bytes, so a future publisher must use an exclusively owned,
unadvertised transaction and promote only after all six members and configuration
verify. This primitive does not read model paths or grant publication authority.

Read-only FTPS observation on September 29 matched the homepage and both live
catalog files to public HTTPS bytes and obtained identical repeated root lists.
The root sentinel was absent. The listing includes structural `.`/`..` entries,
which the older publisher rejected, and six entries outside its web
closure: `.ftpquota`, `workspace.uai`, `artifacts`, `examples`, and two dated web
backup names. Their names alone do not authorize deletion. Keep provider state
and unreviewed content untouched; inspect exact ownership and dependencies before
any root cleanup. Full hashes and the target tuple are in
`artifacts/verification/static-seed-core-review-20260929.json`.

The bounded NLST reader handles the two literal structural listing entries
without admitting arbitrary paths. Its request uses an explicit directory URI:
the actual host returns prefixed paths for a file-shaped `/catalog` request and
leaf names for `/catalog/`. The strict leaf parser remains unchanged. Empty
directories are valid observations, requiring two LISTs before absence is
established. Two bootstrap attempts stopped before remote mutation; their failed
outcomes are retained. The corrected September 29 operation installed the exact
165-byte sentinel and verified its SHA-256, stage absence and lease cleanup.
Receipt `deploy/receipts/site-deployment-20260929T224551Z.json` is 999 bytes,
SHA-256 `6bac53fb169557a474356329912b100efdcb709099aeea77b17fce3d2e07143f`.
That operation changed no website, model or catalog bytes.

The publisher now has single-request STOR and streaming RETR primitives using
the same 64 KiB identity checker. They resolve only the six registered members
and `.htaccess`, or the seven exact `name.seed-{operationId}.tmp` stages for one
operation. Another operation, unknown leaf or rename destination is rejected.
Uploads require a retained readable/seekable source of exact length, an owned
lease and proven stage absence. A lost upload response remains a failure; it
does not authorize an automatic retry or deletion. Source handles belong to the
caller; transport handles are closed. Existing website file/total byte budgets
are unchanged. The explicit seed operation described below consumes these
primitives; ordinary website publication retains its separate admission path.

The publisher's metadata reader now binds the registry, catalog, feed, admission
and qualification receipt to one exact Git commit. The real seed validator runs
through the existing bounded Node process boundary with pinned read leases for
its full canonical dependency closure. The returned metadata hashes must match
the exact input bytes. The still-empty registry fails before payload or account
access. Tests distinguish this plumbing from the validator's semantic tests.

`Open-StaticSeedSourceSet` admits only an absolute directory in
`D:\LLMs\TinyRustLM`, rejects reparse ancestry, opens the six fixed names with
read access and read sharing only, and verifies all lengths and hashes while
retaining those same handles. Any failure closes the handles already opened.
Transport must use the returned handles, then explicitly close the set; the
reader never creates copies or deletes payloads. Its tests use in-memory streams
and mocked file opens, so they do not claim a real model was read or qualified.

The explicit `-PublishSeed -SeedSourceDirectory D:\LLMs\TinyRustLM\<current>`
operation now wires these layers together. It requires the fresh clean/source
gate and exact current metadata/qualification before opening source payloads or
credentials. It verifies the installed marker, acquires the shared lease, and
admits only an absent namespace or the complete seven-file namespace. It preserves
unrelated account-root entries. Configuration is promoted first; all six member
stages must verify before any model member is promoted. Existing complete bytes
are reverified without overwrite. Full FTP readback and HTTPS GET/HEAD checks use
bounded streams. A per-request cancellation deadline covers headers and body.
The HTTP checker rejects redirects, incorrect type/length/identity/CORS, encoding,
credential CORS, and chunked framing; it checks the entire GET hash and EOF.
These are parsed .NET HTTP header checks, not proof of raw wire syntax or a browser
origin's CORS access. Real browser readback remains an explicit separate gate.

Successful seed installation leaves discovery and website documents unchanged.
The immutable admission and qualification receipt, license notice, and fresh signed
catalog/feed must be published and read back by the separate discovery operation
before first-run availability is claimed. A long transfer rechecks metadata
freshness before success. Failure retains its compact journal and any ambiguous
lease or partial namespace for exact ownership review; the command does not retry,
erase, overwrite, or adopt that state. Partial-state recovery is not yet an
automated operator command. This is a release limitation until the actual
first-publication operation has succeeded or a reviewed exact recovery is applied.
A subsequent search located the canonical receipt implementation in TinyRust's
`app/catalog-admission-loader.js`, outside the earlier converter/runtime search.
It binds the receipt to the parsed browser admission and catalog identities.
`tools/admit-static-seed-publication.js` now reuses that implementation with
`browser-model-admission.js` and the actual signed catalog validator. It owns five
bounded input byte arrays and uses a closed two-document in-memory fetch map.
It validates exact operator seed identity, member/piece-set identities and actual
receipt binding. Its result still sets `publicationAllowed` to false because it
has not read payloads or verified the host. Real reviewed passing evidence
is still required before seed upload. Registration or passing metadata strings
alone do not establish that the underlying experiments succeeded.

The next host gate must establish all of the following against the actual server:

1. The remote directory naming and every directive are accepted. Missing required
   modules/overrides are failures, not reasons to omit integrity or CORS fields.
2. Every fixed URL returns 200 without redirect, exact length/type, no encoding,
   one value for each expected identity header, and the descriptor's whole-file
   SHA-256. HEAD agrees with the GET length. Advertise no unqualified bytes.
3. Real browser Fetch from TinyRustLM can read all required headers and complete
   verification. A foreign origin gets no readable CORS response. Invalid names,
   trailing paths, queries, methods and directory listing requests are rejected.
4. Range/resume behavior, if claimed, is separately measured. Today's browser
   installer requests whole members; a 206 metadata probe does not establish
   resumable model acquisition.
5. The complete qualified composition installs, produces a useful conversation,
   restores offline and retires a replaced payload. Only then publish the exact
   current signed catalog/announcement under its full admission requirements.

The C32 readback established HEAD/range/CORS on existing public metadata only.
The subsequent C34 check used Apache 2.4.58 already installed in OSM-Ubuntu,
through WSL. It started a separate loopback instance with operation-owned output
under the canonical MiniModel artifacts directory and removed that output after
verifying its own server exited. The existing server configuration was unchanged;
Docker, installation, image downloads and remote mutation were unnecessary.

All six synthetic members passed GET/HEAD length, type, whole-body hash, identity
and duplicate-header checks. Compression remained disabled despite an enabled
DEFLATE control. Invalid routes/queries and POST/PUT/DELETE/OPTIONS/TRACE were
rejected; malformed directives returned 500 and disabled overrides failed identity
admission. Before the server prerequisite was added, TRACE returned 200 despite
the directory method rule. Core-generated TRACE rejection precedes directory
headers; Apache can append an empty Allow field on other error responses. The
checks compare the allowed method list there, while member identity fields must
still occur exactly once on successful responses.

These are local HTTP observations with deliberately invalid model fixture bytes.
An Alias mapped the exact colon-bearing URL to a Windows-safe fixture directory;
this does not prove remote filesystem acceptance of the colon. No TLS, real-browser
CORS, LiteSpeed, public seed or model qualification result follows. Exact evidence
and earlier harness failures are in `artifacts/reports/static-seed-apache-review-20260919.json`.

The later C53 check adds installed Edge Fetch against the real Apache listener.
All six complete bodies match their hashes, and page JavaScript reads the exact
exposed identity headers. Negative controls establish that omitting exposure
hides the identity fields despite HTTP 200, invalid queries remain 404, and a
foreign document origin cannot read the response. The browser consumes the
allow-origin header internally; it is not itself exposed to page JavaScript.
Only the two synthetic document navigations are intercepted; seed responses
come from Apache unchanged. Local-network permission is granted solely for the
loopback fixture. This closes the local browser CORS gap without establishing
public TLS, LiteSpeed behavior, real model installation or publication. See
`artifacts/reports/static-seed-browser-review-20260919.json`.
Use the already tested native Rust seed behind
a controlled HTTPS origin if the static host cannot satisfy the current contract;
that choice still needs proven hosting and publication authority.

After replacement activation and publication verification, withdraw the prior
announcement and retire the exact superseded remote payload directory using
the established owned-path review. Keep compact hashes, results and lessons.
Do not retain historical remote or local payloads as an unbounded rollback store.
