# MiniModel.org Publication

The prepared 1.1.2 release is not published. Publication is a fail-closed
transition from the exact public 1.1.1 predecessor; it is not an overlay of a
checkout, an ignored staging directory, or whatever happens to exist at an
FTP login.

On September 29, the publisher and manifest builder were rebound to the current
committed ecosystem verifier, and the prepared manifest was regenerated against
content commit `67f3e06`. Deployment custody passes 23 checks (including 27 actual
PowerShell cases); the manifest builder passes 12 checks after its date fixture
was aligned with the prepared authority. Initial failures and exact identities
are in `artifacts/verification/publication-binding-review-20260929.json`.
That check of the normal publisher reached and rejected the expired catalog
before public admission; it read no credentials and changed no public files.
The later read-only host observation matched the homepage and both catalog files
over FTPS and public HTTPS. The exact nonsecret transport tuple is now bound in
the authority. The root sentinel was subsequently installed and verified as
recorded below. Reviewed website closure, real model qualification and seed
delivery remain unfinished.

The publisher now reuses the canonical TinyRustLM `Deployment.Credentials.psm1`
FTP-export reader. It imports the exact reviewed module bytes under a read lease
after source admission and immediately before credential access. No FileZilla
reader or copied credential implementation remains. `-FtpExportPath` selects an
absolute external UTF-8 account export; its default is `ftp.txt` in the canonical
repositories' parent directory. Only the `MiniModel.org` account is selected.
The file stays outside the site worktree and its values are never logged or
included in deployment receipts. This change does not bind a target or authorize
an expired catalog.

`-BootstrapSentinel` is a separate operation from `-Publish`. It requires the
same clean, pushed source gate and shared credential parser, then binds the
checked-in bootstrap manifest to the current commit. It rechecks all three
public/FTPS identities and the exact reviewed root listing before taking the
existing deployment lease. It installs only the exact 165-byte root sentinel
through an absent staging leaf, verifies readback, and releases its own lease.
An already present exact sentinel is verified without overwrite. This mode does
not publish the website, renew discovery metadata, upload models or claim a seed.

The bootstrap inventory records existing overlays solely to identify the account
root for that one marker. It does not approve those overlays for website release.
`.ftpquota` and the unreviewed historical entries remain untouched. NLST decoding
now ignores only literal `.` and `..` structural entries; unsafe names, case
aliases, unknown colon paths, invalid UTF-8 and oversized listings fail.

Bootstrap failure receipts distinguish exact-prefix cleanup, verified promotion
after a lost response, and retained or ambiguous lease state. Exact partial
staging bytes may be removed under the owned lease. Substituted bytes or a raced
final target are preserved for review. If lease creation is ambiguous before its
owner record is established, retain the failure receipt and inspect that exact
lease path; do not remove the root or claim lease ownership from a generic error.
The corrected production bootstrap completed September 29 at 22:49:22 UTC.
Receipt `receipts/site-deployment-20260929T224551Z.json` records the exact marker
readback, removed staging leaf and released lease. Its SHA-256 is
`6bac53fb169557a474356329912b100efdcb709099aeea77b17fce3d2e07143f`.
Earlier failed attempts remain preserved. No website, model or catalog bytes
were changed by the successful marker operation.

After reviewing and committing the current source, an authorized operator can
run `powershell.exe -NoProfile -File tools/deploy-site.ps1 -BootstrapSentinel`.
Its append-only receipt has schema `minimodel.root_bootstrap_receipt.v1`; the
existing bounded receipt filename is retained. A failed receipt must be reviewed
before another attempt. Normal `-Publish` retains its independent catalog,
predecessor, complete tree and final-marker gates.

Normal website publication requires all of the following before it can read credentials or
mutate the authenticated target:

- the script-derived repository root exactly equal to the Git top-level,
  branch `master`, sole remote `origin`, and exact GitHub URL;
- a passing fixed-root ecosystem source-authority gate derived from the
  canonical sibling repository before any remote observation or credential
  access;
- the reviewed Node 24.14.0 executable and exact gate/generator bytes, read-locked
  throughout each child process; inherited `NODE_*` settings are excluded from
  the child without modifying the caller's environment;
- clean equality among local `HEAD`, `origin/master`, and a fresh read-only
  `ls-remote` observation;
- the generated 1.1.2 manifest bound to the reviewed content commit and planned
  UTC publication date in `publication-authority.v1.json`;
- the immutable 1.1.1 receipt and exact cache-bypassed public release marker,
  release ledger, empty composition catalog, and empty peer feed;
- exact candidate catalog and peer-feed identities, separately bound from the
  predecessor, admitted through the current browser contract at verification
  time (including expiry, qualification, lifecycle and Ed25519 signatures);
- an authorized secret-free FTPS host/port/username-hash/root tuple; and
- an exact root sentinel plus an exact predecessor directory and file closure
  with no unknown overlay entries.

`deploy/publication-authority.v1.json` binds the independently observed nonsecret
transport tuple. The bootstrap mode above prepares the root sentinel; neither
that binding nor a verified sentinel bypasses the normal publication gates.
Never infer account ownership from a credential file or a hostname alone.

The read-only preflight is:

```powershell
.\tools\deploy-site.ps1
```

The manifest still describes its older prepared content commit. The subsequent
operator-seed validator change means current preflight first rejects the changed
`catalog.js` source identity. The candidate catalog and peer feed also expired on
2026-09-09 and contain no qualified model or peer entries. The transport tuple and
installed root sentinel are verified; complete website-root reconciliation is
pending. Metadata generation is preparation only and does not establish catalog
freshness or permission to publish. The separately admitted `-PublishSeed` path
is documented in `static-seed.md`; it does not publish discovery or bypass the
website closure checks.

The manifest generator emits JSON to stdout and accepts no output location:

```text
node tools/build-site-deployment-manifest.js <40-character-content-commit>
node tools/build-site-deployment-manifest.js <40-character-content-commit> --verify
```

Both forms run the reviewed canonical source gate and require all 22 explicitly
admitted public files to match the chosen ancestor commit. Verification reads
only the canonical deployment manifest and uses its required `publication_date`;
it also checks candidate discovery against the actual verification time. Generation
uses the current UTC date. The publisher requires that same reviewed date when
starting a publication transaction, before credential access. If publishing on a
later UTC day, regenerate and review the package first. Neither path renews catalog
authority or backdates publication to the source preparation date. Untracked and
ignored public extras,
links, unknown paths, wrong Git object bytes and stale working files are refused.
The generator writes no files. Reviewed metadata belongs in the single canonical
manifest with UTF-8/LF bytes; Windows PowerShell's default text redirection must
not be used to save a byte-bound manifest.

The publisher defaults to the installed Node executable and also accepts
`-NodeExecutablePath` for that exact reviewed binary. A different runtime or
changed tool requires reviewing and updating its pin before use. Child processes
have bounded stdout/stderr and a 120-second deadline; timeout terminates the
owned process tree. Each generator Git read has its own 30-second deadline.

Once all website publication gates are satisfied, the publication command is:

```powershell
.\tools\deploy-site.ps1 -Publish
```

The publisher completes every local and public read-only admission check
first. It then reads the committed publication files into bounded memory and
requires the current checkout to match those source identities. It materializes
the two prepared release records in memory as published 1.1.2 records using
the reviewed transaction date for `last_publish_date`. Historical ledger entries
remain unchanged. Public page copy links to these records for status and dates
instead of duplicating claims that would become stale after publication. No local
site snapshot or staging tree is created. The tracked records remain truthful
that 1.1.2 is prepared and the recorded public predecessor is 1.1.1.

Publication input is limited to 500 files, 16 MiB per file and 64 MiB total.
Each Git blob read has bounded output and a 30-second deadline. The input binds
the source commit, tree, manifest digest, operation and exact published file
identities. Every member is rechecked before upload or partial-stage rollback;
uploads consume those authenticated bytes without reopening mutable source.
New receipts record `publication_input_sha256`. Historical receipts are retained
byte for byte. These local checks do not establish a successful live deployment.

After transport admission, the publisher acquires one exact operation-bound
remote lease. Crash residue is never aged out or adopted. While holding that
lease it re-proves the complete 1.1.1 predecessor, retains exact remote backups
through all final closure, ledger, and HTTPS readback gates, and publishes the
current release marker last. Any pre-commit failure runs a bounded reverse
rollback and releases the lease only after exact predecessor restoration is
proved. Backup cleanup starts only after every final publication gate passes.

The root sentinel is authority material, not a site release artifact. It must
remain byte-identical to `deploy/publication-root-sentinel.v1.json`; the site
manifest never uploads or repairs it. Existing 1.1.1 receipts are immutable
and must not be edited.
