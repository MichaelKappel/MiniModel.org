<?php
declare(strict_types=1);
ini_set('display_errors', '0');
require __DIR__ . '/renewal-core.php';
[$status, $bytes] = mmRenewMetadata(__DIR__, $_SERVER['REQUEST_METHOD'] ?? '',
    $_SERVER['QUERY_STRING'] ?? '', $_SERVER['REQUEST_URI'] ?? '');
http_response_code($status);
header('Content-Type: application/json; charset=utf-8');
header('Cache-Control: no-store');
header('X-Content-Type-Options: nosniff');
header('Access-Control-Allow-Origin: https://tinyrustlm.com');
header('Access-Control-Allow-Methods: GET, HEAD');
if ($status === 503) { header('Retry-After: 60'); }
if (($_SERVER['REQUEST_METHOD'] ?? '') !== 'HEAD') { echo $bytes; }
