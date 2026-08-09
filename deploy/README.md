# MiniModel.org Publication

The release manifest binds one immutable content commit to canonical Git blob
bytes and excludes every `.slm` artifact. The generator reconstructs exact
bytes into ignored staging instead of trusting platform-dependent worktree line
endings.

```powershell
node tools/build-site-deployment-manifest.js <content-commit>
.\tools\deploy-site.ps1
.\tools\deploy-site.ps1 -Publish
```

Publishing requires a clean pushed checkout and the exact external FileZilla
profile named by the manifest. It uses FTPS, bounded verified chunks, per-file
staging and rollback, authenticated byte verification, exact public HTTPS
readback, route smoke checks, and append-only receipts. Credentials remain in
the external profile. `/.well-known/site-release.json` is deliberately
published last as the public release marker.
