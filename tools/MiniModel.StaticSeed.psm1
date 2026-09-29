Set-StrictMode -Version Latest

# Bounded streaming for a separately admitted seed member. Callers own the streams
# and must keep uploaded bytes private/unadvertised until every member verifies.
function Copy-StaticSeedVerifiedStream {
    param(
        [Parameter(Mandatory)][IO.Stream]$Source,
        [AllowNull()][IO.Stream]$Destination,
        [Parameter(Mandatory)][int64]$ExpectedBytes,
        [Parameter(Mandatory)][string]$ExpectedSha256
    )

    if ($ExpectedBytes -lt 1 -or $ExpectedBytes -gt 68719476736 -or
        $ExpectedSha256.Length -ne 64 -or $ExpectedSha256 -cnotmatch '^[0-9a-f]{64}$' -or
        -not $Source.CanRead -or
        ($null -ne $Destination -and (-not $Destination.CanWrite -or [object]::ReferenceEquals($Source,$Destination)))) {
        throw 'Static seed stream contract is invalid.'
    }
    $buffer = [byte[]]::new(65536)
    $sha = [Security.Cryptography.SHA256]::Create()
    $total = [int64]0
    try {
        while ($total -lt $ExpectedBytes) {
            $wanted = [int][Math]::Min([int64]$buffer.Length,$ExpectedBytes - $total)
            $count = $Source.Read($buffer,0,$wanted)
            if ($count -le 0) { throw 'Static seed stream ended before its exact length.' }
            $null = $sha.TransformBlock($buffer,0,$count,$buffer,0)
            if ($null -ne $Destination) { $Destination.Write($buffer,0,$count) }
            $total += $count
        }
        if ($Source.ReadByte() -ne -1) { throw 'Static seed stream exceeded its exact length.' }
        $null = $sha.TransformFinalBlock([byte[]]::new(0),0,0)
        $digest = ([BitConverter]::ToString($sha.Hash)).Replace('-','').ToLowerInvariant()
        if ($digest -cne $ExpectedSha256) { throw 'Static seed stream hash differs from its admitted identity.' }
        if ($null -ne $Destination) { $Destination.Flush() }
        return [pscustomobject]@{Bytes=$total;Sha256=$digest}
    }
    finally {
        $sha.Dispose()
        [Array]::Clear($buffer,0,$buffer.Length)
    }
}

Export-ModuleMember -Function @('Copy-StaticSeedVerifiedStream')
