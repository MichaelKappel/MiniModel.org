<?php
declare(strict_types=1);

// Called only by the fixed HTTP endpoint or the canonical CLI test harness.
function mmProbeRequire(bool $condition): void {
    if (!$condition) { throw new RuntimeException('host-capability-check'); }
}
function mmProbeStat(string $path, bool $directory = false): array {
    clearstatcache(true, $path);
    $s = @lstat($path);
    mmProbeRequire(is_array($s) && !is_link($path));
    mmProbeRequire(($s['mode'] & 0170000) === ($directory ? 0040000 : 0100000));
    if (!$directory) { mmProbeRequire($s['nlink'] === 1); }
    return $s;
}
function mmProbeSame(array $a, array $b): bool {
    return $a['dev'] === $b['dev'] && $a['ino'] === $b['ino']
        && ($a['mode'] & 0170000) === ($b['mode'] & 0170000);
}
function mmProbeRead(string $path, int $limit): string {
    $before = mmProbeStat($path);
    mmProbeRequire($before['size'] >= 0 && $before['size'] <= $limit);
    $stream = @fopen($path, 'rb');
    mmProbeRequire(is_resource($stream));
    try {
        mmProbeRequire(mmProbeSame($before, fstat($stream)));
        $bytes = stream_get_contents($stream, $limit + 1);
        mmProbeRequire(is_string($bytes) && strlen($bytes) === $before['size']);
        $after = mmProbeStat($path);
        mmProbeRequire(mmProbeSame($before, $after) && $after['size'] === strlen($bytes));
        return $bytes;
    } finally { fclose($stream); }
}
function mmProbeSyncDirectory(string $path): void {
    $before = mmProbeStat($path, true);
    $stream = @fopen($path, 'rb');
    mmProbeRequire(is_resource($stream));
    try {
        mmProbeRequire(mmProbeSame($before, fstat($stream)) && @fsync($stream));
        mmProbeRequire(mmProbeSame($before, mmProbeStat($path, true)));
    } finally { fclose($stream); }
}
function mmHostCapability(string $root, string $method, string $query,
    string $uri = '/.minimodel-host-capability.v1/probe.json'): array {
    $checks = array_fill_keys(['php', 'sodium', 'leaseBinding', 'privateRead',
        'privateWrite', 'flush', 'atomicRename', 'readback', 'flock', 'cleanup'], false);
    $result = ['schema' => 'minimodel.host_capability_result.v1',
        'nonce' => null, 'ok' => false, 'checks' => $checks];
    if (explode('?', $uri, 2)[0] !== '/.minimodel-host-capability.v1/probe.json') {
        return [404, $result];
    }
    if ($method !== 'GET') { return [405, $result]; }
    if ($query !== '' || strpos($uri, '?') !== false) { return [400, $result]; }
    $result['checks']['php'] = PHP_VERSION_ID >= 80100 && function_exists('fsync');
    $lock = null;
    $write = null;
    $bound = false;
    try {
        mmProbeRequire($result['checks']['php']);
        $rootStat = mmProbeStat($root, true);
        $private = $root . '/private';
        $privateStat = mmProbeStat($private, true);
        $operationBytes = mmProbeRead($private . '/operation.json', 512);
        $operation = json_decode($operationBytes, true, 8, JSON_THROW_ON_ERROR);
        mmProbeRequire(is_array($operation) && array_keys($operation)
            === ['schema', 'operation_id', 'lease_owner_sha256']);
        mmProbeRequire($operation['schema'] === 'minimodel.host_capability_operation.v1');
        mmProbeRequire(is_string($operation['operation_id'])
            && preg_match('/^[a-f0-9]{32}$/D', $operation['operation_id']) === 1);
        mmProbeRequire(is_string($operation['lease_owner_sha256'])
            && preg_match('/^[a-f0-9]{64}$/D', $operation['lease_owner_sha256']) === 1);
        mmProbeRequire(json_encode($operation, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR)
            . "\n" === $operationBytes);
        $id = $operation['operation_id'];
        $lease = dirname($root) . '/.minimodel-deployment-lease.v1';
        $leaseStat = mmProbeStat($lease, true);
        $owner = $lease . '/owner-' . $id . '.json';
        $deny = "Require all denied\n";
        $canary = 'minimodel-host-capability-canary:' . $id . "\n";
        $binding = function () use ($root, $rootStat, $private, $privateStat,
            $operationBytes, $lease, $leaseStat, $owner, $operation, $deny, $canary): void {
            mmProbeRequire(mmProbeSame($rootStat, mmProbeStat($root, true)));
            mmProbeRequire(mmProbeSame($privateStat, mmProbeStat($private, true)));
            mmProbeRequire(mmProbeSame($leaseStat, mmProbeStat($lease, true)));
            $leaseEntries = array_values(array_diff(scandir($lease), ['.', '..']));
            mmProbeRequire($leaseEntries === [basename($owner)]);
            mmProbeRequire(mmProbeRead($private . '/operation.json', 512) === $operationBytes);
            mmProbeRequire(mmProbeRead($private . '/.htaccess', 64) === $deny);
            mmProbeRequire(mmProbeRead($private . '/canary.txt', 128) === $canary);
            mmProbeRequire(hash_equals($operation['lease_owner_sha256'],
                hash('sha256', mmProbeRead($owner, 16384))));
        };
        $binding();
        $bound = true;
        $result['nonce'] = $id;
        $result['checks']['leaseBinding'] = true;
        $result['checks']['privateRead'] = true;
        mmProbeRequire(function_exists('sodium_crypto_sign_seed_keypair')
            && function_exists('sodium_crypto_sign_detached')
            && function_exists('sodium_crypto_sign_verify_detached'));
        // Public fixed test material; never a production signing key.
        $pair = sodium_crypto_sign_seed_keypair(str_repeat("\x01", 32));
        $message = 'minimodel-host-capability-v1';
        $signature = sodium_crypto_sign_detached($message, sodium_crypto_sign_secretkey($pair));
        mmProbeRequire(sodium_crypto_sign_verify_detached($signature, $message,
            sodium_crypto_sign_publickey($pair)));
        $result['checks']['sodium'] = true;
        $work = $private . '/probe-' . $id;
        $binding();
        clearstatcache(true, $work);
        mmProbeRequire(@lstat($work) === false && @mkdir($work, 0700));
        $workStat = mmProbeStat($work, true);
        $assertWork = function () use ($binding, $work, $workStat): void {
            $binding();
            mmProbeRequire(mmProbeSame($workStat, mmProbeStat($work, true)));
        };
        $assertWork();
        $lockPath = $work . '/lease.lock';
        $lock = @fopen($lockPath, 'x+b');
        mmProbeRequire(is_resource($lock) && flock($lock, LOCK_EX | LOCK_NB));
        $lockStat = fstat($lock);
        $second = @fopen($lockPath, 'rb');
        mmProbeRequire(is_resource($second));
        try {
            $contended = flock($second, LOCK_EX | LOCK_NB);
            if ($contended) { flock($second, LOCK_UN); }
            mmProbeRequire(!$contended);
        } finally { fclose($second); }
        $result['checks']['flock'] = true;
        $assertWork();
        $pending = $work . '/pending';
        $final = $work . '/committed';
        $payload = 'minimodel-host-capability-write:' . $id . "\n";
        $write = @fopen($pending, 'x+b');
        mmProbeRequire(is_resource($write));
        mmProbeRequire(fwrite($write, $payload) === strlen($payload));
        $result['checks']['privateWrite'] = true;
        mmProbeRequire(fflush($write) && fsync($write));
        $fileStat = fstat($write);
        fclose($write);
        $write = null;
        $assertWork();
        mmProbeRequire(mmProbeSame($fileStat, mmProbeStat($pending))
            && mmProbeRead($pending, 128) === $payload && @lstat($final) === false);
        mmProbeRequire(@rename($pending, $final));
        $result['checks']['atomicRename'] = true;
        $assertWork();
        mmProbeSyncDirectory($work);
        $result['checks']['flush'] = true; // Both file contents and directory entry were flushed.
        mmProbeRequire(mmProbeSame($fileStat, mmProbeStat($final))
            && mmProbeRead($final, 128) === $payload && @lstat($pending) === false);
        $result['checks']['readback'] = true;
        // Only retire these exact new files; any ambiguity retains the work directory.
        $assertWork();
        $entries = array_values(array_diff(scandir($work), ['.', '..']));
        sort($entries);
        mmProbeRequire($entries === ['committed', 'lease.lock']);
        mmProbeRequire(mmProbeSame($lockStat, mmProbeStat($lockPath))
            && mmProbeRead($lockPath, 0) === '');
        mmProbeRequire(mmProbeSame($fileStat, mmProbeStat($final))
            && mmProbeRead($final, 128) === $payload);
        mmProbeRequire(@unlink($final));
        $assertWork();
        mmProbeRequire(flock($lock, LOCK_UN));
        fclose($lock);
        $lock = null;
        mmProbeRequire(mmProbeSame($lockStat, mmProbeStat($lockPath))
            && mmProbeRead($lockPath, 0) === '' && @unlink($lockPath));
        $assertWork();
        mmProbeRequire(count(array_diff(scandir($work), ['.', '..'])) === 0 && @rmdir($work));
        $binding();
        $result['checks']['cleanup'] = true;
        $result['ok'] = !in_array(false, $result['checks'], true);
        return [200, $result];
    } catch (Throwable $ignored) {
        // No path, environment value, key or exception text is exposed.
        return [$bound ? 200 : 409, $result];
    } finally {
        if (is_resource($write)) { fclose($write); }
        if (is_resource($lock)) { flock($lock, LOCK_UN); fclose($lock); }
    }
}
