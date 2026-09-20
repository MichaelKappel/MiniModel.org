# MiniModel verification

Run the current catalog, cross-project contract, publication and browser checks
from the canonical MiniModel.org repository:

```powershell
node --test tests/catalog.test.js tests/catalog-contract.test.js tests/deployment-custody.test.js tests/build-site-deployment-manifest_test.js tests/browser-smoke_test.js
node --test tests/build-static-seed-plan_test.js
```

The publication checks require the installed reviewed Node binary, PowerShell
with Pester, Git, and the unchanged canonical TinyRustLM source-authority gate.
The service account may need its established process-local Git safe-directory
settings for the seven canonical repositories. Do not alter the gate or global
Git settings to make an observation pass.

The browser check uses installed Microsoft Edge and the existing playwright-core
package under the canonical TinyRustLM.com/tinyrustlm/node_modules directory.
It downloads no browser or dependency. To emit its standalone observation:

```powershell
node tests/browser-smoke.js
```

No target URL or output path is accepted. The tool serves exactly six canonical
public inputs from bounded memory on its own ephemeral loopback listener. Private
files, source tools and external origins are excluded. Profiles, browser temporary
files and automation output stay in one exclusively created operation directory
under MiniModel.org/artifacts/tests. Screenshots are hashed in memory. The tool
closes its browsers and listener, validates directory/marker ownership and the
bounded ordinary output tree, then proves removal. Cleanup failure is an error;
it is never silently deferred or treated as a passing run. Do not adopt or erase
an abandoned run through this tool.

Each viewport uses a fresh browser. Current expired-catalog rendering is checked
with its real current clock. Separately labeled historical fixtures set the
browser clock to the checked-in catalog's issuance time and exercise its empty
state, an HTTP 503 reload failure and recovery. That clock and those empty
metadata fixtures prove UI behavior only. They do not renew catalog authority,
qualify a model, establish a seed or prove deployment. The tool fails for review
when the source ceases to match the declared empty/expired fixture assumptions.

Keep experiment lessons and compact evidence in the existing ecosystem record.
Do not retain source copies, browser profiles or screenshots as another archive.

The static-seed preparation tests consume the canonical companion's exact wire
fixture and ACG2 profile bytes in place. They check descriptor identity, all six
URLs/headers, bounded canonical parsing, injection refusal and the real stdin
CLI. They do not run Apache/LiteSpeed or upload model bytes. Host admission and
publication requirements are in `deploy/static-seed.md`.

Run the separate installed-server integration check with:

```powershell
node --test tests/build-static-seed-plan-apache_test.js
```

This requires the existing OSM-Ubuntu WSL distribution and its installed Apache
modules; absence is a failure, not a skipped verification. It starts its own
loopback Apache instance and leaves the existing site configuration untouched.
Bounded synthetic non-model bytes, generated configuration, PID and logs stay in
one owned canonical artifact directory. Shutdown binds the PID to that exact
configuration, proves process exit, then verifies output removal. No package,
Docker image, source copy, model payload or remote publication is created.

GET/HEAD assertions verify all six complete bodies and response identities;
method/path/query failures and compression/override controls test enforcement.
The generated server prerequisite disables TRACE, which bypasses directory rules
on Apache. These Node HTTP checks do not establish TLS, LiteSpeed, public
availability or model qualification. The Alias used for the colon-bearing URL
does not establish the actual remote filesystem's path support.

The same check now launches installed Edge and verifies six real cross-origin
Apache responses from a synthetic TinyRustLM-origin page. The page navigation
is intercepted; model-route Fetch responses are not. A deliberately unexposed
header control and a foreign-origin rejection prove browser enforcement. The
fixture grants local-network access for loopback without disabling CORS.
Browser profiles and temporary output share the owned directory and are removed
after the browser and server close. These additional checks establish local
browser CORS only; public TLS, actual hosting and model qualification remain open.

Node's experimental coverage collector creates its own temporary directory before
loading tests. When collecting coverage, launch the test process with TMP, TEMP
and TMPDIR pointing to an exclusively owned directory under this repository's
artifacts/tests, and verify removal after the process closes. Do not assume the
browser helper's scoped environment also contains its parent test runner. Cleanup
assertions must identify that operation's directory, not require the shared
artifacts/tests parent to be empty while independent checks run.
