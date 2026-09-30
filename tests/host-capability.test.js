"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");
const test = require("node:test");
const root = path.resolve(__dirname, "..");
const probe = path.join(root, "deploy/host-capability");
const digest = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
const { createOwnedTemporaryDirectory, removeOwnedTemporaryDirectory } =
    require("../../TinyRustLM.com/tinyrustlm/tools/canonical-operation-scratch.js");

test("capability source closure binds only the four approved deployable files", () => {
    const manifest = JSON.parse(fs.readFileSync(path.join(probe, "source-manifest.v1.json")));
    assert.equal(manifest.remoteNamespace, ".minimodel-host-capability.v1");
    assert.equal(manifest.published, false);
    assert.deepEqual(manifest.members.map(m => m.path),
        [".htaccess", "private/.htaccess", "probe-core.php", "probe.php"]);
    for (const member of manifest.members) {
        const bytes = fs.readFileSync(path.join(probe, member.path));
        assert.equal(bytes.length, member.byteLength);
        assert.equal(digest(bytes), member.sha256);
    }
    assert.equal(fs.readFileSync(path.join(probe, "private/.htaccess"), "utf8"),
        "Require all denied\n");
});

test("actual cached PHP executes capability and rejection paths without network", {
    skip: process.env.MINIMODEL_HOST_CAPABILITY_PHP_DOCKER !== "1"
        ? "Explicit local PHP Docker runtime not selected; production capability unverified" : false
}, () => {
    const inspect = spawnSync("docker", ["image", "inspect", "php:8.3-cli",
        "--format", "{{.Id}}"], {encoding:"utf8", timeout:15000, windowsHide:true});
    assert.equal(inspect.status, 0, inspect.stderr);
    const image = inspect.stdout.trim();
    assert.match(image, /^sha256:[a-f0-9]{64}$/);
    const owned = createOwnedTemporaryDirectory({
        temporaryParent:path.resolve(root, "../TinyRustLM.com/artifacts/tests/minimodel-host-capability")
    });
    try {
        const args = ["run", "--rm", "-i", "--pull=never", "--network", "none",
            "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
            "--mount", "type=bind,src=" + root + ",dst=/source,readonly",
            "--mount", "type=bind,src=" + owned.rootPath + ",dst=/probe-tests",
            image, "php"];
        const result = spawnSync("docker", args, {input:PHP_HARNESS,
            encoding:"utf8", timeout:60000, maxBuffer:1024*1024, windowsHide:true});
        assert.equal(result.status, 0, result.stdout + result.stderr);
        assert.equal(result.stderr, "");
        const output = JSON.parse(result.stdout);
        assert.ok(output.phpVersionId >= 80100);
        assert.equal(output.passed.length, 19);
        console.log(JSON.stringify({schema:"minimodel.local_php_capability_test.v1",
            image, ...output, productionCapabilityVerified:false}));
    } finally {
        removeOwnedTemporaryDirectory(owned);
    }
});

const PHP_HARNESS = "\u003c?php\ndeclare(strict_types=1);\nrequire \u0027/source/deploy/host-capability/probe-core.php\u0027;\nfunction check($ok, $label) { if (!$ok) { throw new RuntimeException($label); } }\n$id = str_repeat(\u0027a\u0027, 32);\n$passed = [];\nfunction fixture(string $name): array {\n    global $id;\n    $base = \u0027/probe-tests/\u0027 . $name;\n    check(mkdir($base, 0700), \u0027fixture create\u0027);\n    $root = $base . \u0027/.minimodel-host-capability.v1\u0027;\n    mkdir($root, 0700);\n    mkdir($root . \u0027/private\u0027, 0700);\n    mkdir($base . \u0027/.minimodel-deployment-lease.v1\u0027, 0700);\n    $owner = \"{\\\"operation_id\\\":\\\"\" . $id . \"\\\"}\\n\";\n    $op = [\u0027schema\u0027=\u003e\u0027minimodel.host_capability_operation.v1\u0027,\n        \u0027operation_id\u0027=\u003e$id, \u0027lease_owner_sha256\u0027=\u003ehash(\u0027sha256\u0027, $owner)];\n    file_put_contents($root . \u0027/private/operation.json\u0027,\n        json_encode($op, JSON_UNESCAPED_SLASHES) . \"\\n\");\n    file_put_contents($root . \u0027/private/.htaccess\u0027, \"Require all denied\\n\");\n    file_put_contents($root . \u0027/private/canary.txt\u0027,\n        \u0027minimodel-host-capability-canary:\u0027 . $id . \"\\n\");\n    file_put_contents($base . \u0027/.minimodel-deployment-lease.v1/owner-\u0027 . $id . \u0027.json\u0027, $owner);\n    return [$root, $base, $op];\n}\nfunction rejection(string $name, callable $mutate, int $status=409): void {\n    global $passed, $id;\n    [$root,$base,$op] = fixture($name);\n    $mutate($root,$base,$op);\n    [$actual,$result] = mmHostCapability($root, \u0027GET\u0027, \u0027\u0027);\n    check($actual === $status \u0026\u0026 $result[\u0027ok\u0027] === false, $name);\n    check(!is_dir($root . \u0027/private/probe-\u0027 . $id), $name . \u0027 no scratch\u0027);\n    $passed[] = $name;\n}\n[$root,$base] = fixture(\u0027success\u0027);\n[$status,$result] = mmHostCapability($root, \u0027GET\u0027, \u0027\u0027);\ncheck($status === 200 \u0026\u0026 $result[\u0027ok\u0027] === true, \u0027successful operation\u0027);\ncheck($result[\u0027nonce\u0027] === $id \u0026\u0026 count($result[\u0027checks\u0027]) === 10\n    \u0026\u0026 !in_array(false,$result[\u0027checks\u0027],true), \u0027all capabilities\u0027);\ncheck(!file_exists($root . \u0027/private/probe-\u0027 . $id), \u0027exact scratch retired\u0027);\ncheck(array_keys($result) === [\u0027schema\u0027,\u0027nonce\u0027,\u0027ok\u0027,\u0027checks\u0027], \u0027public output closed\u0027);\n$passed[]=\u0027success-all-capabilities\u0027;\nmmProbeSyncDirectory($root);\n$passed[]=\u0027actual-directory-fsync\u0027;\n$regular = $root . \u0027/private/canary.txt\u0027;\n$rejected = false;\ntry { mmProbeSyncDirectory($regular); } catch (Throwable $expected) { $rejected = true; }\ncheck($rejected \u0026\u0026 is_file($regular), \u0027file cannot prove directory fsync\u0027);\n$passed[]=\u0027directory-fsync-rejects-file\u0027;\n$link = $base . \u0027/linked-directory\u0027;\ncheck(symlink($root, $link), \u0027directory link fixture\u0027);\n$rejected = false;\ntry { mmProbeSyncDirectory($link); } catch (Throwable $expected) { $rejected = true; }\ncheck($rejected \u0026\u0026 is_link($link), \u0027linked directory cannot prove custody\u0027);\nunlink($link); // Exact test-owned link, never probe cleanup.\n$passed[]=\u0027directory-fsync-rejects-link\u0027;\nforeach ([\u0027POST\u0027,\u0027HEAD\u0027,\u0027DELETE\u0027] as $method) {\n    [$status,$r] = mmHostCapability($root,$method,\u0027\u0027);\n    check($status === 405 \u0026\u0026 $r[\u0027nonce\u0027] === null, \u0027method closed\u0027);\n}\n[$status,$r] = mmHostCapability($root,\u0027GET\u0027,\u0027nonce=caller\u0027);\ncheck($status === 400 \u0026\u0026 $r[\u0027nonce\u0027] === null, \u0027query closed\u0027);\n$passed[]=\u0027methods-and-query\u0027;\nforeach ([\u0027/.minimodel-host-capability.v1/probe.php\u0027,\u0027/.minimodel-host-capability.v1/%70robe.json\u0027] as $uri) {\n    [$status,$r]=mmHostCapability($root,\u0027GET\u0027,\u0027\u0027,$uri);\n    check($status === 404 \u0026\u0026 !$r[\u0027ok\u0027],\u0027sole route\u0027);\n}\n[$status,$r]=mmHostCapability($root,\u0027GET\u0027,\u0027\u0027,\u0027/.minimodel-host-capability.v1/probe.json?\u0027);\ncheck($status === 400 \u0026\u0026 !$r[\u0027ok\u0027],\u0027empty query rejected\u0027);\n$passed[]=\u0027sole-public-route\u0027;\nrejection(\u0027extra-lease-owner\u0027,function($r,$b){file_put_contents($b.\u0027/.minimodel-deployment-lease.v1/unowned.json\u0027,\u0027extra\u0027);});\nrejection(\u0027wrong-lease\u0027,function($r,$b) use($id) {\n    file_put_contents($b.\u0027/.minimodel-deployment-lease.v1/owner-\u0027.$id.\u0027.json\u0027,\u0027replacement\u0027);\n});\nrejection(\u0027missing-denial\u0027,function($r){unlink($r.\u0027/private/.htaccess\u0027);});\nrejection(\u0027wrong-denial\u0027,function($r){file_put_contents($r.\u0027/private/.htaccess\u0027,\"Require all granted\\n\");});\nrejection(\u0027wrong-canary\u0027,function($r){file_put_contents($r.\u0027/private/canary.txt\u0027,\u0027replacement\u0027);});\nrejection(\u0027invalid-json\u0027,function($r){file_put_contents($r.\u0027/private/operation.json\u0027,\u0027{\u0027);});\nrejection(\u0027oversize-json\u0027,function($r){file_put_contents($r.\u0027/private/operation.json\u0027,str_repeat(\u0027x\u0027,513));});\nrejection(\u0027extra-field\u0027,function($r,$b,$o){$o[\u0027path\u0027]=\u0027/etc\u0027;file_put_contents($r.\u0027/private/operation.json\u0027,json_encode($o).\"\\n\");});\nrejection(\u0027noncanonical-json\u0027,function($r,$b,$o){file_put_contents($r.\u0027/private/operation.json\u0027,json_encode($o,JSON_PRETTY_PRINT).\"\\n\");});\nrejection(\u0027invalid-id\u0027,function($r,$b,$o){$o[\u0027operation_id\u0027]=\u0027../escape\u0027;file_put_contents($r.\u0027/private/operation.json\u0027,json_encode($o).\"\\n\");});\nrejection(\u0027missing-owner\u0027,function($r,$b)use($id){unlink($b.\u0027/.minimodel-deployment-lease.v1/owner-\u0027.$id.\u0027.json\u0027);});\n[$root,$base] = fixture(\u0027existing-scratch\u0027);\n$work = $root.\u0027/private/probe-\u0027.$id;\nmkdir($work,0700); file_put_contents($work.\u0027/keep\u0027,\u0027unrelated\u0027);\n[$status,$r]=mmHostCapability($root,\u0027GET\u0027,\u0027\u0027);\ncheck($status === 200 \u0026\u0026 !$r[\u0027ok\u0027] \u0026\u0026 file_get_contents($work.\u0027/keep\u0027) === \u0027unrelated\u0027,\u0027scratch retained\u0027);\n$passed[]=\u0027existing-scratch-retained\u0027;\n[$root,$base] = fixture(\u0027linked-operation\u0027);\n$target=$root.\u0027/private/original.json\u0027;\nrename($root.\u0027/private/operation.json\u0027,$target);\ncheck(symlink($target,$root.\u0027/private/operation.json\u0027),\u0027fixture symlink\u0027);\n[$status,$r]=mmHostCapability($root,\u0027GET\u0027,\u0027\u0027);\ncheck($status === 409 \u0026\u0026 !$r[\u0027ok\u0027],\u0027symlink rejected\u0027);\ncheck(is_link($root.\u0027/private/operation.json\u0027),\u0027link retained by probe\u0027);\nunlink($root.\u0027/private/operation.json\u0027); // Test-owned exact link, not probe cleanup.\n$passed[]=\u0027symlink-rejected\u0027;\necho json_encode([\u0027phpVersionId\u0027=\u003ePHP_VERSION_ID, \u0027passed\u0027=\u003e$passed], JSON_THROW_ON_ERROR), \"\\n\";\n";
