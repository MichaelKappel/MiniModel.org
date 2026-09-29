<?php
declare(strict_types=1);
ini_set('display_errors', '0');
header('Content-Type: application/json; charset=utf-8');
header('Cache-Control: no-store, max-age=0');
header('X-Content-Type-Options: nosniff');
require __DIR__ . '/probe-core.php';
[$status, $result] = mmHostCapability(__DIR__,
    $_SERVER['REQUEST_METHOD'] ?? '', $_SERVER['QUERY_STRING'] ?? '',
    $_SERVER['REQUEST_URI'] ?? '');
http_response_code($status);
if ($status === 405) { header('Allow: GET'); }
echo json_encode($result, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
