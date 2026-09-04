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

Once the authority is legitimately bound, the publication command is:

```powershell
.\tools\deploy-site.ps1 -Publish
```

The publisher completes every local and public read-only admission check
first. It then reconstructs canonical Git blobs into a new operation-owned
snapshot outside the checkout and materializes the two prepared release
records there as genuinely published 1.1.2 records. The tracked source records
remain truthful that 1.1.2 is prepared and public production is still 1.1.1.

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
