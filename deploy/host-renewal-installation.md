# Hosted renewal installation contract

Prepared implementation sequence; installation is not implemented or published by this document. The actual capability receipt is `deploy/receipts/site-deployment-20260930T002007Z.json`: all ten checks passed and the temporary namespace was removed. That operation installed no signing key, model or discovery metadata. The root identity sentinel remains installed. There is still no qualified default seed.

The existing account supports the required PHP/libsodium, private HTTP denial, file and directory fsync, replacement of an existing file, shared lease binding and local locking. This removes the need for a developer scheduler or a new hosting service. The actual renewal namespace must independently prove private denial before receiving a secret.

## Inputs and source closure

`tools/build-host-renewal-input.js` accepts bounded owned bytes for the existing seed descriptor, catalog, signed peer feed, browser admission, qualification receipt and reviewed license, plus the exact six-field release owner. It calls `admitStaticSeedPublication`, which uses the canonical TinyRust browser readers. It does not accept a source path, a signing key, a caller-defined reference list or model bytes. Its result remains `publicationAllowed: false`.

The producer preserves admitted public bytes exactly. Catalog/feed generation and expiry, announcement issue/expiry, and key validity use the current whole-second UTC contract. The pair shares one generation and expiry. Configuration fixes the existing 86,400-second maximum lifetime and 21,600-second renewal threshold; it cannot extend key validity or alter model, qualification, consent or revocation facts.

The exact nine ordered source references are:

1. `model.slm2`, at most 2 GiB.
2. `tokenizer.tokenizer2`, at most 64 MiB.
3. `template.template2`, at most 1 MiB.
4. `sampling.sampling2`, at most 65,800 bytes.
5. `prompt.prompt2`, at most 512 bytes.
6. `composition.acg2`, exactly 896 bytes.
7. The catalog entry's browser admission route and exact admitted bytes/hash.
8. Its qualification receipt route and exact admitted bytes/hash.
9. Its reviewed license route and exact committed license bytes/hash.

The first six paths are derived exclusively as `tinyrustlm/direct-composition/v1/compositions/sha256:<64 lowercase hex>/members/<member name>`. The colon exception applies only there. Evidence routes cannot overlap the seed namespace, either discovery route, a hidden control path, an executable PHP path or another evidence route. The catalog currently carries no license byte identity, so the publisher must capture and bind the committed reviewed license; it cannot invent one from the catalog label.

The operation must retain distinct identities for the approved release source commit and the actual publisher commit. Its runtime source manifest covers only the four checked-in PHP/denial members. Configuration and initial-pair digests are separate receipt fields, avoiding a circular manifest/configuration hash. The captured runtime manifest, authority, producer and complete Node dependency closure must remain bound through execution, using the existing verified tool launcher and read leases.

## Private key preparation

Initial public metadata must already be signed before it can pass the actual admission reader. After a real model qualifies, a separate bounded preparation step may create one Ed25519 seed in the owner's private credential store and emit only its public key and signed public metadata. The seed is never a Git file, shell argument, log field, public fixture or caller-controlled HTTP input. No production key has been created for the current work.

The eventual installer reads the same 32-byte seed only after the new namespace's HTTP denial is proved, verifies that its derived public key equals the admitted catalog key, and transfers it privately through the existing strict FTPS account. The initial public signatures and final server-side key correspondence are both required. A key mismatch fails before routing requests to the handler.

## Exact installation order

The new explicit operation will be exclusive with ordinary website, sentinel, seed and probe modes. It must use their existing canonical source gate, transport authority, sentinel and shared lease; it must not introduce a second credential parser or transport.

1. Capture and validate all public inputs, runtime source bytes and the reviewed catalog rewrite from committed source before credentials. Fail while the descriptor is unregistered, qualification fails, the public pair has expired, the key validity is inadequate or the expected source closure differs.
2. Admit the exact existing target and sentinel. Verify the already published qualified seed's six bodies and generated configuration through the existing bounded stream checker. The current static-seed receipt alone is insufficient to establish current remote identity.
3. Verify the three evidence destinations are absent or already contain the exact intended public bytes. Refuse unrelated occupants. Capture the precise predecessor `catalog/.htaccess` and its recovery identity; do not replace an arbitrary current file.
4. Acquire the existing exclusive `.minimodel-deployment-lease.v1` and repeat target, absence/identity and freshness checks. Require `.minimodel-metadata-renewal.v1` to be absent for first installation; an incomplete namespace is recovery work, not permission to overwrite.
5. Create only that namespace and its `private` child. Install exact `private/.htaccess` plus a nonsecret operation-bound canary, read them back over FTPS, and prove the canary and directory are denied over no-follow HTTPS before any key or private configuration transfer.
6. Install the four manifest-bound runtime/denial members and exact `private/config.json`, `private/pair.json`, zero-byte `private/renewal.lock` and verified `private/signing-key.seed`. Use absent stages, bounded exact readback and existing owner checks. Require the closed namespace inventory and deny every private URL. Remove only the exact owned canary after verification.
7. Install/read back the three approved public evidence files before discovery becomes reachable. Revalidate public input freshness and private key/public key correspondence. Do not rewrite or renew qualification evidence.
8. Promote the reviewed `catalog/.htaccess` last. It internally maps only `/catalog/compositions.v1.json` and `/catalog/peer-announcements.v1.json` to the fixed handler. Preserve ordinary catalog resource behavior. Clear conflicting CORS/cache headers in both Apache header tables for those two routes and set their intended values explicitly; do not leave the inherited wildcard plus PHP's explicit origin as duplicate headers.
9. With the publication lease still held, fetch and validate both public metadata routes and all three evidence documents through the actual browser readers, matching the prepared initial pair. The core serves its verified still-current pair while another publisher holds the shared lease. Check GET/HEAD, no-store, JSON type, CORS, method/query/direct-handler rejection and private denial.
10. Read back the final exact owned closure and release only the exact owned lease. Record installation separately from later actual timestamp renewal. No full website release marker may claim this scoped operation published the whole site.

The handler changes only revisions, timestamps and the signed announcement when less than six hours remain. It hashes the nine immutable references before renewal, takes its local lock and the same shared publisher lease, writes a pending journal, fsyncs and replaces the private pair atomically. The browser's bounded pair reload handles a renewal between two separate metadata GETs. Disk atomicity does not make separate HTTP requests atomic.

## Recovery and release integration

Before the final rewrite promotion, a failure leaves discovery on the exact predecessor. After an ambiguous rewrite response, determine the exact old/new identity once under the retained owned lease; do not blindly repeat promotion. Any substituted bytes, unknown occupants, pending journal or ambiguous lease remain for explicit recovery. Rollback must use only the captured exact predecessor and verify its restoration; it cannot delete a parent tree or overwrite another writer.

The ordinary website publisher must recognize the installed operational namespace through its explicit closure and preserve it under the same lease. Its current static discovery-byte checks cannot claim that a deliberately renewed HTTP response equals the old Git snapshot. The coherent website release therefore needs an explicit hosted-renewal discovery mode and actual reader validation, while retaining the existing website payload budgets, root inventory custody and final release-marker ordering. Refresh the admitted site's current `catalog.js` along with that release; the older public script must not be treated as proof that the operator-seed disposition is live.

Installation tests must cover exact source/owner/reference binding, retained buffer custody, denial before key read, public-key mismatch, closed inventories, old/new rewrite identity, failures before and after promotion, unrelated state preservation and lease ownership. Use actual PHP fixtures and canonical JavaScript readers for the generated pair. Mocked FTPS tests establish ordering and bounded custody; they do not replace actual host readback. No large fixture or real model is needed for these transaction tests.

Release evidence still requires a genuinely qualified model, actual six-member delivery, public installation, and a later observed metadata renewal with an incremented revision and preserved immutable fields. Record key expiration and the exact recovery owner. Current 24-hour admission expiry still limits offline activation/generation; hosted metadata renewal does not create unrestricted offline use.
