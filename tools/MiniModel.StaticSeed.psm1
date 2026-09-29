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

# Reject link redirection anywhere in the canonical source ancestry before opening a payload.
function Assert-StaticSeedPlainPath {
    param([Parameter(Mandatory)][string]$Path)
    $current=Get-Item -LiteralPath $Path -Force
    while ($null -ne $current) {
        if (($current.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Static seed source contains a reparse point.' }
        $current=if ($current -is [IO.DirectoryInfo]) {$current.Parent} else {$current.Directory}
    }
}

# Keep the exact canonical file open without permitting replacement or writes during publication.
function Open-StaticSeedSourceFile {
    param([Parameter(Mandatory)][string]$Path)
    Assert-StaticSeedPlainPath -Path $Path
    return [IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
}

# Own and hash all six source handles before any transport is admitted; never create a source copy.
function Open-StaticSeedSourceSet {
    param([Parameter(Mandatory)][string]$Directory,[Parameter(Mandatory)][object[]]$Members)
    $canonicalRoot='D:\LLMs\TinyRustLM'
    if (-not [IO.Path]::IsPathRooted($Directory)) { throw 'Static seed source directory must be canonical and absolute.' }
    $resolved=[IO.Path]::GetFullPath($Directory).TrimEnd([IO.Path]::DirectorySeparatorChar)
    if ((-not [string]::Equals($resolved,$canonicalRoot,[StringComparison]::OrdinalIgnoreCase) -and
        -not $resolved.StartsWith($canonicalRoot+'\',[StringComparison]::OrdinalIgnoreCase)) -or
        $resolved.Substring(3) -match '[<>:"|?*\x00-\x1f]') { throw 'Static seed source is outside the canonical model store.' }
    $names=@('model.slm2','tokenizer.tokenizer2','template.template2','sampling.sampling2','prompt.prompt2','composition.acg2')
    if ($Members.Count -ne $names.Count) { throw 'Static seed source requires exactly six current members.' }
    for($index=0;$index -lt $names.Count;$index++) {
        $member=$Members[$index]
        if ([string]$member.name -cne $names[$index] -or [int64]$member.bytes -lt 1 -or
            [int64]$member.bytes -gt 68719476736 -or ([string]$member.sha256).Length -ne 64 -or
            [string]$member.sha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'Static seed source member identity is invalid.' }
    }
    Assert-StaticSeedPlainPath -Path $resolved
    $opened=[Collections.Generic.List[object]]::new()
    try {
        foreach($member in $Members) {
            $path=Join-Path $resolved ([string]$member.name)
            $stream=Open-StaticSeedSourceFile -Path $path
            $opened.Add([pscustomobject]@{Name=[string]$member.name;Bytes=[int64]$member.bytes;Sha256=[string]$member.sha256;Stream=$stream})
            if (-not $stream.CanRead -or -not $stream.CanSeek -or $stream.Length -ne [int64]$member.bytes) {
                throw 'Static seed source file has a different length or stream type.'
            }
            $null=Copy-StaticSeedVerifiedStream -Source $stream -ExpectedBytes ([int64]$member.bytes) -ExpectedSha256 ([string]$member.sha256)
            $stream.Position=0
        }
        return [pscustomobject]@{Directory=$resolved;Members=$opened.ToArray()}
    }
    catch {
        foreach($member in $opened){$member.Stream.Dispose()}
        throw
    }
}

# The caller ends custody only after success or a fully recorded failed operation; payloads are never deleted here.
function Close-StaticSeedSourceSet {
    param([Parameter(Mandatory)]$SourceSet)
    foreach($member in $SourceSet.Members){$member.Stream.Dispose()}
}

Export-ModuleMember -Function @('Copy-StaticSeedVerifiedStream','Open-StaticSeedSourceSet','Close-StaticSeedSourceSet')
