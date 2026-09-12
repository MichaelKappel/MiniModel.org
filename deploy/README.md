# MiniModel.org Publication

The prepared 1.1.2 release is not published. Publication is a fail-closed
transition from the exact public 1.1.1 predecessor; it is not an overlay of a
checkout, an ignored staging directory, or whatever happens to exist at an
FTP login.

The publisher requires all of the following before it can read credentials or
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
- the generated 1.1.2 manifest bound to content commit
  `7e0a6eb6bf292ff69e3030adf1d229c811606ce9`;
- the immutable 1.1.1 receipt and exact cache-bypassed public release marker,
  release ledger, empty composition catalog, and empty peer feed;
- an authorized secret-free FTPS host/port/username-hash/root tuple; and
- an exact root sentinel plus an exact predecessor directory and file closure
  with no unknown overlay entries.

`deploy/publication-authority.v1.json` deliberately leaves the transport tuple
`unbound`. `-Publish` therefore fails before reading the FileZilla profile.
An authorized operator must independently record the real nonsecret tuple,
install the already-bound sentinel bytes at the intended remote root, and land
that authority through the normal reviewed commit and push flow. Never infer
those values from a credential file or guess them from a hostname.

The read-only preflight is:

```powershell
.\tools\deploy-site.ps1
```

The current manifest still names an older content commit. Preflight now rejects
its disagreement with current public files before any public-site observation.
Transport also remains unbound. Neither this cleanup nor successful metadata
generation makes the candidate publishable.

The manifest generator emits JSON to stdout and accepts no output location:

```text
node tools/build-site-deployment-manifest.js <40-character-content-commit>
node tools/build-site-deployment-manifest.js <40-character-content-commit> --verify
```

Both forms run the reviewed canonical source gate and require all 22 explicitly
admitted public files to match the chosen ancestor commit. Verification reads
only the canonical deployment manifest. Untracked and ignored public extras,
links, unknown paths, wrong Git object bytes and stale working files are refused.
The generator writes no files. Reviewed metadata belongs in the single canonical
manifest with UTF-8/LF bytes; Windows PowerShell's default text redirection must
not be used to save a byte-bound manifest.

The publisher defaults to the installed Node executable and also accepts
`-NodeExecutablePath` for that exact reviewed binary. A different runtime or
changed tool requires reviewing and updating its pin before use. Child processes
have bounded stdout/stderr and a 120-second deadline; timeout terminates the
owned process tree. Each generator Git read has its own 30-second deadline.

Once the authority is legitimately bound, the publication command is:

```powershell
.\tools\deploy-site.ps1 -Publish
```

The publisher completes every local and public read-only admission check
first. It then reads the committed publication files into bounded memory and
requires the current checkout to match those source identities. It materializes
the two prepared release records in memory as published 1.1.2 records. No local
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
