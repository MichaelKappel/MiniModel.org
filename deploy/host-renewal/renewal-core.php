<?php
declare(strict_types=1);

// Fixed deployment inputs only; no caller-supplied path, key, model or clock.
function mmRenewRequire(bool $ok): void {
    if (!$ok) { throw new RuntimeException('metadata-unavailable'); }
}
function mmRenewJson(mixed $value): string {
    return json_encode($value, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE
        | JSON_UNESCAPED_LINE_TERMINATORS | JSON_THROW_ON_ERROR) . "\n";
}
function mmRenewParse(string $bytes): object {
    $value = json_decode($bytes, false, 64, JSON_THROW_ON_ERROR);
    mmRenewRequire(is_object($value) && mmRenewJson($value) === $bytes);
    return $value;
}
function mmRenewKeys(object $value, array $keys): void {
    mmRenewRequire(array_keys(get_object_vars($value)) === $keys);
}
function mmRenewStat(string $path, bool $dir = false): array {
    clearstatcache(true, $path);
    $s = @lstat($path);
    mmRenewRequire(is_array($s) && !is_link($path)
        && ($s['mode'] & 0170000) === ($dir ? 0040000 : 0100000));
    if (!$dir) { mmRenewRequire($s['nlink'] === 1); }
    return $s;
}
function mmRenewSame(array $a, array $b): bool {
    return $a['dev'] === $b['dev'] && $a['ino'] === $b['ino']
        && ($a['mode'] & 0170000) === ($b['mode'] & 0170000);
}
function mmRenewRead(string $path, int $limit): string {
    $before = mmRenewStat($path);
    mmRenewRequire($before['size'] >= 0 && $before['size'] <= $limit);
    $h = @fopen($path, 'rb');
    mmRenewRequire(is_resource($h));
    try {
        mmRenewRequire(mmRenewSame($before, fstat($h)));
        $bytes = stream_get_contents($h, $limit + 1);
        $after = mmRenewStat($path);
        mmRenewRequire(is_string($bytes) && strlen($bytes) === $before['size']
            && mmRenewSame($before, $after) && $after['size'] === strlen($bytes));
        return $bytes;
    } finally { fclose($h); }
}
function mmRenewWriteNew(string $path, string $bytes): array {
    $h = @fopen($path, 'x+b');
    mmRenewRequire(is_resource($h));
    try {
        mmRenewRequire(chmod($path, 0600));
        $offset = 0;
        while ($offset < strlen($bytes)) {
            $n = fwrite($h, substr($bytes, $offset));
            mmRenewRequire(is_int($n) && $n > 0);
            $offset += $n;
        }
        mmRenewRequire(fflush($h) && fsync($h));
        $s = fstat($h);
        mmRenewRequire(mmRenewSame($s, mmRenewStat($path)));
        return $s;
    } finally { fclose($h); }
}
function mmRenewSyncDir(string $path): void {
    $before = mmRenewStat($path, true);
    $h = @fopen($path, 'rb');
    mmRenewRequire(is_resource($h));
    try {
        mmRenewRequire(mmRenewSame($before, fstat($h)) && fsync($h)
            && mmRenewSame($before, mmRenewStat($path, true)));
    } finally { fclose($h); }
}
function mmRenewUtc(string $value): int {
    mmRenewRequire(preg_match('/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/D', $value) === 1);
    $t = strtotime($value);
    mmRenewRequire(is_int($t) && gmdate('Y-m-d\TH:i:s\Z', $t) === $value);
    return $t;
}
function mmRenewB64(string $bytes): string {
    return rtrim(strtr(base64_encode($bytes), '+/', '-_'), '=');
}
function mmRenewUnb64(string $text, int $length): string {
    $bytes = base64_decode(strtr($text, '-_', '+/'), true);
    mmRenewRequire(is_string($bytes) && strlen($bytes) === $length
        && mmRenewB64($bytes) === $text);
    return $bytes;
}
function mmRenewSigningBytes(object $a): string {
    return json_encode([
        'schema' => 'tinyrustlm.peer-announcement-proof/1',
        'announcement_id' => $a->announcement_id,
        'composition_id' => $a->composition_id,
        'piece_set_sha256' => $a->piece_set_sha256,
        'peer_base_url' => $a->peer_base_url,
        'protocol_profile' => $a->protocol_profile,
        'consent_status' => $a->consent_status,
        'consent_receipt_sha256' => $a->consent_receipt_sha256,
        'outside_proof_status' => $a->outside_proof_status,
        'outside_proof_receipt_sha256' => $a->outside_proof_receipt_sha256,
        'issued_at_utc' => $a->issued_at_utc,
        'expires_utc' => $a->expires_utc,
        'verification_key_id' => $a->verification_key_id
    ], JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE
        | JSON_UNESCAPED_LINE_TERMINATORS | JSON_THROW_ON_ERROR) . "\n";
}
function mmRenewImmutable(object $catalog, object $feed): string {
    $c = clone $catalog; $f = clone $feed;
    unset($c->revision, $c->generated_at_utc, $c->valid_until_utc);
    unset($f->revision, $f->generated_at_utc, $f->valid_until_utc);
    $f->announcements = array_map(function ($a) {
        $copy = clone $a;
        unset($copy->issued_at_utc, $copy->expires_utc, $copy->signature);
        return $copy;
    }, $feed->announcements);
    return mmRenewJson([$c, $f]);
}
function mmRenewPair(object $pair, object $config, string $configHash,
    object $initialCatalog, object $initialFeed, int $now): array {
    mmRenewKeys($pair, ['schema', 'config_sha256', 'catalog', 'peer_feed']);
    mmRenewRequire($pair->schema === 'minimodel.metadata_pair.v1'
        && $pair->config_sha256 === $configHash);
    $c = mmRenewParse($pair->catalog); $f = mmRenewParse($pair->peer_feed);
    mmRenewRequire(mmRenewImmutable($c, $f) === mmRenewImmutable($initialCatalog, $initialFeed));
    foreach ([[$c, $initialCatalog], [$f, $initialFeed]] as [$doc, $initial]) {
        mmRenewRequire(is_int($doc->revision) && $doc->revision >= $initial->revision
            && $doc->revision < 9007199254740991);
    }
    $generated = mmRenewUtc($c->generated_at_utc);
    $expires = mmRenewUtc($c->valid_until_utc);
    mmRenewRequire($f->generated_at_utc === $c->generated_at_utc
        && $f->valid_until_utc === $c->valid_until_utc
        && $generated >= mmRenewUtc($initialCatalog->generated_at_utc)
        && $generated <= $now + 300 && $expires > $generated
        && $expires - $generated <= $config->ttl_seconds);
    $key = $c->peer_verification_keys[0];
    mmRenewRequire($generated >= mmRenewUtc($key->not_before_utc)
        && $expires <= mmRenewUtc($key->not_after_utc));
    foreach ($f->announcements as $a) {
        mmRenewRequire($a->issued_at_utc === $c->generated_at_utc
            && $a->expires_utc === $c->valid_until_utc
            && $a->verification_key_id === $key->key_id);
        mmRenewRequire(sodium_crypto_sign_verify_detached(
            mmRenewUnb64($a->signature, 64), mmRenewSigningBytes($a),
            mmRenewUnb64($key->public_key_base64url, 32)));
    }
    return [$c, $f, $generated, $expires];
}
// Bind the exact hosted six-member composition and its three approved evidence routes.
function mmRenewReferences(object $entry, array $refs): array {
    $names = ['model.slm2', 'tokenizer.tokenizer2', 'template.template2',
        'sampling.sampling2', 'prompt.prompt2', 'composition.acg2'];
    $caps = [2147483648, 67108864, 1048576, 65800, 512, 896];
    $composition = $entry->composition;
    mmRenewRequire(count($refs) === 9 && $composition->member_count === 6
        && is_array($composition->members) && count($composition->members) === 6
        && preg_match('/^sha256:[a-f0-9]{64}$/D', $composition->composition_id) === 1);
    $prefix = 'tinyrustlm/direct-composition/v1/compositions/'
        . $composition->composition_id . '/members/';
    $expected = []; $limits = [];
    foreach ($names as $i => $name) {
        $member = $composition->members[$i];
        mmRenewRequire($member->name === $name);
        $expected[] = [$prefix . $name, $member->bytes, $member->sha256];
        $limits[] = $caps[$i];
    }
    $a = $entry->browser_admission; $q = $entry->qualification; $l = $entry->license;
    mmRenewRequire($l->review_status === 'reviewed');
    foreach ([$a->route, $q->receipt_route, $l->route] as $route) {
        mmRenewRequire(is_string($route) && strlen($route) <= 512
            && preg_match('/^\/[A-Za-z0-9_-][A-Za-z0-9._\/-]*$/D', $route) === 1);
        $parts = explode('/', substr($route, 1));
        mmRenewRequire(!in_array('..', $parts, true) && !in_array('.', $parts, true)
            && !in_array('', $parts, true));
        mmRenewRequire(!str_starts_with($route, '/tinyrustlm/direct-composition/'));
    }
    $expected[] = [substr($a->route, 1), $a->bytes, $a->sha256];
    $expected[] = [substr($q->receipt_route, 1), $q->receipt_bytes, $q->receipt_sha256];
    // License bytes/hash come from the committed, reviewed publisher input, not a caller.
    $expected[] = [substr($l->route, 1), $refs[8]->bytes, 'sha256:' . $refs[8]->sha256];
    array_push($limits, 1048576, 1048576, 1048576);
    foreach ($refs as $i => $ref) {
        mmRenewKeys($ref, ['path', 'bytes', 'sha256']);
        [$path, $bytes, $digest] = $expected[$i];
        mmRenewRequire(is_string($digest) && preg_match('/^sha256:[a-f0-9]{64}$/D', $digest) === 1
            && is_int($bytes) && $bytes > 0 && $bytes <= $limits[$i]
            && $ref->path === $path && $ref->bytes === $bytes
            && $ref->sha256 === substr($digest, 7));
    }
    mmRenewRequire($refs[5]->bytes === 896
        && count(array_unique(array_column($refs, 'path'))) === 9);
    return $limits;
}
// Stream one already policy-bound reference, preserving path/inode and size/hash identity.
function mmRenewSource(string $site, object $ref, int $limit): void {
    mmRenewRequire($ref->bytes <= $limit);
    $parts = explode('/', $ref->path); $path = $site; $dirs = [];
    array_pop($parts);
    foreach ($parts as $part) {
        $path .= '/' . $part;
        $dirs[$path] = mmRenewStat($path, true);
    }
    $path = $site . '/' . $ref->path;
    $before = mmRenewStat($path);
    mmRenewRequire($before['size'] === $ref->bytes);
    $h = @fopen($path, 'rb');
    mmRenewRequire(is_resource($h));
    try {
        mmRenewRequire(mmRenewSame($before, fstat($h)));
        $hash = hash_init('sha256');
        mmRenewRequire(hash_update_stream($hash, $h) === $ref->bytes
            && hash_equals($ref->sha256, hash_final($hash)));
        $after = mmRenewStat($path);
        mmRenewRequire(mmRenewSame($before, $after) && $after['size'] === $ref->bytes
            && $after['mtime'] === $before['mtime'] && $after['ctime'] === $before['ctime']);
        foreach ($dirs as $dir => $s) {
            mmRenewRequire(mmRenewSame($s, mmRenewStat($dir, true)));
        }
    } finally { fclose($h); }
}
// A current verified pair may return immediately; expired concurrent GETs wait at most 30s.
function mmRenewLock(mixed $lock, bool $waitForRenewal): void {
    $deadline = hrtime(true) + 30000000000;
    do {
        if (flock($lock, LOCK_EX | LOCK_NB)) { return; }
        mmRenewRequire($waitForRenewal && hrtime(true) < $deadline);
        usleep(50000);
    } while (true);
}
function mmRenewMetadata(string $root, string $method, string $query, string $uri): array {
    $routes = ['/catalog/compositions.v1.json' => 'catalog',
        '/catalog/peer-announcements.v1.json' => 'peer_feed'];
    $error = "{\"error\":\"metadata-temporarily-unavailable\"}\n";
    if (!isset($routes[explode('?', $uri, 2)[0]])) { return [404, $error]; }
    if ($method !== 'GET' && $method !== 'HEAD') { return [405, $error]; }
    if ($query !== '' || strpos($uri, '?') !== false) { return [400, $error]; }
    $field = $routes[$uri]; $lock = null; $fallback = null; $fallbackExpires = 0;
    try {
        mmRenewRequire(PHP_VERSION_ID >= 80100 && PHP_INT_SIZE >= 8 && function_exists('fsync')
            && function_exists('sodium_crypto_sign_detached'));
        $site = dirname($root); $private = $root . '/private';
        $siteStat = mmRenewStat($site, true);
        $rootStat = mmRenewStat($root, true); $privateStat = mmRenewStat($private, true);
        $configBytes = mmRenewRead($private . '/config.json', 1048576);
        $configHash = hash('sha256', $configBytes);
        $config = mmRenewParse($configBytes);
        mmRenewKeys($config, ['schema', 'initial_catalog', 'initial_peer_feed',
            'release_owner', 'source_references', 'ttl_seconds', 'refresh_before_seconds']);
        mmRenewRequire($config->schema === 'minimodel.metadata_renewal_config.v1'
            && $config->ttl_seconds === 86400 && $config->refresh_before_seconds === 21600
            && is_array($config->source_references) && count($config->source_references) === 9);
        mmRenewRequire(count(array_unique(array_column($config->source_references, 'path')))
            === count($config->source_references));
        mmRenewKeys($config->release_owner, ['site', 'release_id', 'source_commit',
            'publisher_commit', 'manifest_sha256', 'authority_sha256']);
        mmRenewRequire($config->release_owner->site === 'minimodel.org');
        $initialCatalog = mmRenewParse($config->initial_catalog);
        $initialFeed = mmRenewParse($config->initial_peer_feed);
        mmRenewRequire(count($initialCatalog->entries) === 1
            && count($initialCatalog->peer_verification_keys) === 1
            && count($initialFeed->announcements) === 1
            && $initialFeed->announcements[0]->peer_base_url === 'https://minimodel.org/'
            && $initialFeed->announcements[0]->outside_proof_status === 'not-required-operator-seed'
            && $initialFeed->announcements[0]->outside_proof_receipt_sha256 === null
            && $initialCatalog->entries[0]->qualification->status === 'qualified');
        $referenceLimits = mmRenewReferences($initialCatalog->entries[0], $config->source_references);
        $binding = function () use ($site, $siteStat, $root, $rootStat, $private,
            $privateStat, $configBytes): void {
            mmRenewRequire(mmRenewSame($siteStat, mmRenewStat($site, true))
                && mmRenewSame($rootStat, mmRenewStat($root, true))
                && mmRenewSame($privateStat, mmRenewStat($private, true)));
            mmRenewRequire(mmRenewRead($private . '/.htaccess', 64) === "Require all denied\n"
                && mmRenewRead($private . '/config.json', 1048576) === $configBytes);
        };
        $binding();
        $pairPath = $private . '/pair.json';
        $pairBytes = mmRenewRead($pairPath, 1048576); $pair = mmRenewParse($pairBytes);
        $now = time();
        [$c, $f, $generated, $expires] = mmRenewPair($pair, $config, $configHash,
            $initialCatalog, $initialFeed, $now);
        if ($expires > $now) { $fallback = $pair->$field; $fallbackExpires = $expires; }
        if ($expires - $now > $config->refresh_before_seconds) { return [200, $pair->$field]; }
        $lockPath = $private . '/renewal.lock'; $lockStat = mmRenewStat($lockPath);
        mmRenewRequire($lockStat['size'] === 0);
        $lock = @fopen($lockPath, 'rb'); // Lock-only descriptor; the file is never modified.
        mmRenewRequire(is_resource($lock) && mmRenewSame($lockStat, fstat($lock)));
        mmRenewLock($lock, $expires <= time());
        $binding();
        // Another request may have completed while this request opened the lock.
        $pairBytes = mmRenewRead($pairPath, 1048576); $pair = mmRenewParse($pairBytes);
        $now = time();
        [$c, $f, $generated, $expires] = mmRenewPair($pair, $config, $configHash,
            $initialCatalog, $initialFeed, $now);
        if ($expires > $now) { $fallback = $pair->$field; $fallbackExpires = $expires; }
        if ($expires - $now > $config->refresh_before_seconds) { return [200, $pair->$field]; }
        mmRenewRequire($now > $generated);
        // Existing ambiguous journals/stages are operator recovery, never automatic overwrite.
        mmRenewRequire(@lstat($private . '/pending.json') === false
            && @lstat($private . '/pair.next') === false);
        $lease = $site . '/.minimodel-deployment-lease.v1';
        mmRenewRequire(@lstat($lease) === false && @mkdir($lease, 0700));
        $leaseStat = mmRenewStat($lease, true);
        $id = bin2hex(random_bytes(16));
        $owner = (object)array_merge(['schema' => 'minimodel.deployment_lease.v1',
            'operation_id' => $id], get_object_vars($config->release_owner));
        $ownerBytes = mmRenewJson($owner); $ownerPath = $lease . '/owner-' . $id . '.json';
        $ownerStat = mmRenewWriteNew($ownerPath, $ownerBytes);
        mmRenewSyncDir($lease); mmRenewSyncDir($site);
        $leaseBinding = function () use ($binding, $lease, $leaseStat, $ownerPath,
            $ownerStat, $ownerBytes, $lockPath, $lockStat): void {
            $binding();
            mmRenewRequire(mmRenewSame($leaseStat, mmRenewStat($lease, true))
                && array_values(array_diff(scandir($lease), ['.', '..'])) === [basename($ownerPath)]
                && mmRenewSame($ownerStat, mmRenewStat($ownerPath))
                && mmRenewRead($ownerPath, 8192) === $ownerBytes
                && mmRenewSame($lockStat, mmRenewStat($lockPath)));
        };
        $leaseBinding();
        foreach ($config->source_references as $i => $ref) {
            mmRenewSource($site, $ref, $referenceLimits[$i]);
        }
        $leaseBinding();
        $seed = mmRenewRead($private . '/signing-key.seed', 32);
        mmRenewRequire(strlen($seed) === 32);
        $keys = sodium_crypto_sign_seed_keypair($seed); sodium_memzero($seed);
        $secret = sodium_crypto_sign_secretkey($keys);
        $public = sodium_crypto_sign_publickey($keys); sodium_memzero($keys);
        mmRenewRequire(mmRenewB64($public) === $c->peer_verification_keys[0]->public_key_base64url);
        $now = time();
        $until = min($now + $config->ttl_seconds,
            mmRenewUtc($c->peer_verification_keys[0]->not_after_utc));
        mmRenewRequire($now > $generated && $until - $now > $config->refresh_before_seconds);
        $c->revision++; $f->revision++;
        $c->generated_at_utc = $f->generated_at_utc = gmdate('Y-m-d\TH:i:s\Z', $now);
        $c->valid_until_utc = $f->valid_until_utc = gmdate('Y-m-d\TH:i:s\Z', $until);
        foreach ($f->announcements as $a) {
            $a->issued_at_utc = $c->generated_at_utc; $a->expires_utc = $c->valid_until_utc;
            $a->signature = mmRenewB64(sodium_crypto_sign_detached(mmRenewSigningBytes($a), $secret));
        }
        sodium_memzero($secret);
        $next = (object)['schema' => 'minimodel.metadata_pair.v1',
            'config_sha256' => $configHash, 'catalog' => mmRenewJson($c), 'peer_feed' => mmRenewJson($f)];
        mmRenewPair($next, $config, $configHash, $initialCatalog, $initialFeed, $now);
        $nextBytes = mmRenewJson($next); $nextPath = $private . '/pair.next';
        $journal = mmRenewJson((object)['schema' => 'minimodel.metadata_renewal_pending.v1',
            'operation_id' => $id, 'old_pair_sha256' => hash('sha256', $pairBytes),
            'new_pair_sha256' => hash('sha256', $nextBytes)]);
        $leaseBinding();
        $journalPath = $private . '/pending.json';
        $journalStat = mmRenewWriteNew($journalPath, $journal);
        mmRenewSyncDir($private);
        $nextStat = mmRenewWriteNew($nextPath, $nextBytes);
        mmRenewSyncDir($private);
        $leaseBinding();
        mmRenewRequire(mmRenewRead($pairPath, 1048576) === $pairBytes
            && mmRenewSame($nextStat, mmRenewStat($nextPath))
            && mmRenewRead($nextPath, 1048576) === $nextBytes);
        mmRenewRequire(@rename($nextPath, $pairPath)); mmRenewSyncDir($private);
        $leaseBinding();
        mmRenewRequire(mmRenewSame($nextStat, mmRenewStat($pairPath))
            && mmRenewRead($pairPath, 1048576) === $nextBytes
            && mmRenewSame($journalStat, mmRenewStat($journalPath))
            && mmRenewRead($journalPath, 1024) === $journal);
        mmRenewRequire(@unlink($journalPath)); mmRenewSyncDir($private);
        $leaseBinding();
        mmRenewRequire(@unlink($ownerPath));
        mmRenewRequire(mmRenewSame($leaseStat, mmRenewStat($lease, true))
            && count(array_diff(scandir($lease), ['.', '..'])) === 0 && @rmdir($lease));
        mmRenewSyncDir($site);
        return [200, $next->$field];
    } catch (Throwable $ignored) {
        // Serve only an already signature-verified, still-current pair. Never expose private errors.
        return $fallback !== null && $fallbackExpires > time() ? [200, $fallback] : [503, $error];
    } finally {
        if (is_resource($lock)) { flock($lock, LOCK_UN); fclose($lock); }
    }
}
