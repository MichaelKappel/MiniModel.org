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
This tool does not extend that closure or bypass its unbound transport identity,
expired catalog or incomplete source-authority scan. A seed publication route
must separately admit exact target ownership and hashes, stream verified bytes
from the canonical model store, and verify remote readback before advertising
availability. Existing source, site, seed and qualification records remain distinct.

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
No Apache executable was found on PATH and the installed Docker client could not
connect to its stopped Linux engine during this review. No service was started,
image downloaded or remote configuration changed. Current tests validate
generated bytes, bounds and cross-language identities; host execution and live
model serving remain unverified. Use the already tested native Rust seed behind
a controlled HTTPS origin if the static host cannot satisfy the current contract;
that choice still needs proven hosting and publication authority.

After replacement activation and publication verification, withdraw the prior
announcement and retire the exact superseded remote payload directory using
the established owned-path review. Keep compact hashes, results and lessons.
Do not retain historical remote or local payloads as an unbounded rollback store.
