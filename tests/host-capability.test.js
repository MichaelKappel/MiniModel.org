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
        assert.equal(output.passed.length, 16);
        console.log(JSON.stringify({schema:"minimodel.local_php_capability_test.v1",
            image, ...output, productionCapabilityVerified:false}));
    } finally {
        removeOwnedTemporaryDirectory(owned);
    }
});

const PHP_HARNESS = "<?php\ndeclare(strict_types=1);\nrequire '/source/deploy/host-capability/probe-core.php';\nfunction check($ok, $label) { if (!$ok) { throw new RuntimeException($label); } }\n$id = str_repeat('a', 32);\n$passed = [];\nfunction fixture(string $name): array {\n    global $id;\n    $base = '/probe-tests/' . $name;\n    check(mkdir($base, 0700), 'fixture create');\n    $root = $base . '/.minimodel-host-capability.v1';\n    mkdir($root, 0700);\n    mkdir($root . '/private', 0700);\n    mkdir($base . '/.minimodel-deployment-lease.v1', 0700);\n    $owner = \"{\\\"operation_id\\\":\\\"\" . $id . \"\\\"}\\n\";\n    $op = ['schema'=>'minimodel.host_capability_operation.v1',\n        'operation_id'=>$id, 'lease_owner_sha256'=>hash('sha256', $owner)];\n    file_put_contents($root . '/private/operation.json',\n        json_encode($op, JSON_UNESCAPED_SLASHES) . \"\\n\");\n    file_put_contents($root . '/private/.htaccess', \"Require all denied\\n\");\n    file_put_contents($root . '/private/canary.txt',\n        'minimodel-host-capability-canary:' . $id . \"\\n\");\n    file_put_contents($base . '/.minimodel-deployment-lease.v1/owner-' . $id . '.json', $owner);\n    return [$root, $base, $op];\n}\nfunction rejection(string $name, callable $mutate, int $status=409): void {\n    global $passed, $id;\n    [$root,$base,$op] = fixture($name);\n    $mutate($root,$base,$op);\n    [$actual,$result] = mmHostCapability($root, 'GET', '');\n    check($actual === $status && $result['ok'] === false, $name);\n    check(!is_dir($root . '/private/probe-' . $id), $name . ' no scratch');\n    $passed[] = $name;\n}\n[$root,$base] = fixture('success');\n[$status,$result] = mmHostCapability($root, 'GET', '');\ncheck($status === 200 && $result['ok'] === true, 'successful operation');\ncheck($result['nonce'] === $id && count($result['checks']) === 10\n    && !in_array(false,$result['checks'],true), 'all capabilities');\ncheck(!file_exists($root . '/private/probe-' . $id), 'exact scratch retired');\ncheck(array_keys($result) === ['schema','nonce','ok','checks'], 'public output closed');\n$passed[]='success-all-capabilities';\nforeach (['POST','HEAD','DELETE'] as $method) {\n    [$status,$r] = mmHostCapability($root,$method,'');\n    check($status === 405 && $r['nonce'] === null, 'method closed');\n}\n[$status,$r] = mmHostCapability($root,'GET','nonce=caller');\ncheck($status === 400 && $r['nonce'] === null, 'query closed');\n$passed[]='methods-and-query';\nforeach (['/.minimodel-host-capability.v1/probe.php','/.minimodel-host-capability.v1/%70robe.json'] as $uri) {\n    [$status,$r]=mmHostCapability($root,'GET','',$uri);\n    check($status === 404 && !$r['ok'],'sole route');\n}\n[$status,$r]=mmHostCapability($root,'GET','','/.minimodel-host-capability.v1/probe.json?');\ncheck($status === 400 && !$r['ok'],'empty query rejected');\n$passed[]='sole-public-route';\nrejection('extra-lease-owner',function($r,$b){file_put_contents($b.'/.minimodel-deployment-lease.v1/unowned.json','extra');});\nrejection('wrong-lease',function($r,$b) use($id) {\n    file_put_contents($b.'/.minimodel-deployment-lease.v1/owner-'.$id.'.json','replacement');\n});\nrejection('missing-denial',function($r){unlink($r.'/private/.htaccess');});\nrejection('wrong-denial',function($r){file_put_contents($r.'/private/.htaccess',\"Require all granted\\n\");});\nrejection('wrong-canary',function($r){file_put_contents($r.'/private/canary.txt','replacement');});\nrejection('invalid-json',function($r){file_put_contents($r.'/private/operation.json','{');});\nrejection('oversize-json',function($r){file_put_contents($r.'/private/operation.json',str_repeat('x',513));});\nrejection('extra-field',function($r,$b,$o){$o['path']='/etc';file_put_contents($r.'/private/operation.json',json_encode($o).\"\\n\");});\nrejection('noncanonical-json',function($r,$b,$o){file_put_contents($r.'/private/operation.json',json_encode($o,JSON_PRETTY_PRINT).\"\\n\");});\nrejection('invalid-id',function($r,$b,$o){$o['operation_id']='../escape';file_put_contents($r.'/private/operation.json',json_encode($o).\"\\n\");});\nrejection('missing-owner',function($r,$b)use($id){unlink($b.'/.minimodel-deployment-lease.v1/owner-'.$id.'.json');});\n[$root,$base] = fixture('existing-scratch');\n$work = $root.'/private/probe-'.$id;\nmkdir($work,0700); file_put_contents($work.'/keep','unrelated');\n[$status,$r]=mmHostCapability($root,'GET','');\ncheck($status === 200 && !$r['ok'] && file_get_contents($work.'/keep') === 'unrelated','scratch retained');\n$passed[]='existing-scratch-retained';\n[$root,$base] = fixture('linked-operation');\n$target=$root.'/private/original.json';\nrename($root.'/private/operation.json',$target);\ncheck(symlink($target,$root.'/private/operation.json'),'fixture symlink');\n[$status,$r]=mmHostCapability($root,'GET','');\ncheck($status === 409 && !$r['ok'],'symlink rejected');\ncheck(is_link($root.'/private/operation.json'),'link retained by probe');\nunlink($root.'/private/operation.json'); // Test-owned exact link, not probe cleanup.\n$passed[]='symlink-rejected';\necho json_encode(['phpVersionId'=>PHP_VERSION_ID, 'passed'=>$passed], JSON_THROW_ON_ERROR), \"\\n\";\n";
