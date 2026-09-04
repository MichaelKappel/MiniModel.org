Set-StrictMode -Version Latest

if (-not ("MiniModelDeploymentCustody.NativeMethods" -as [type])) {
    Add-Type -TypeDefinition @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace MiniModelDeploymentCustody {
    public static class NativeMethods {
        [StructLayout(LayoutKind.Sequential)]
        private struct ByHandleFileInformation {
            public uint FileAttributes;
            public System.Runtime.InteropServices.ComTypes.FILETIME CreationTime;
            public System.Runtime.InteropServices.ComTypes.FILETIME LastAccessTime;
            public System.Runtime.InteropServices.ComTypes.FILETIME LastWriteTime;
            public uint VolumeSerialNumber;
            public uint FileSizeHigh;
            public uint FileSizeLow;
            public uint NumberOfLinks;
            public uint FileIndexHigh;
            public uint FileIndexLow;
        }

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFile(
            string fileName,
            uint desiredAccess,
            uint shareMode,
            IntPtr securityAttributes,
            uint creationDisposition,
            uint flagsAndAttributes,
            IntPtr templateFile);

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool GetFileInformationByHandle(
            SafeFileHandle file,
            out ByHandleFileInformation information);

        public static string GetPathIdentity(string path) {
            const uint FileShareRead = 1;
            const uint FileShareWrite = 2;
            const uint FileShareDelete = 4;
            const uint OpenExisting = 3;
            const uint FileFlagBackupSemantics = 0x02000000;
            const uint FileFlagOpenReparsePoint = 0x00200000;
            using (SafeFileHandle handle = CreateFile(
                path,
                0,
                FileShareRead | FileShareWrite | FileShareDelete,
                IntPtr.Zero,
                OpenExisting,
                FileFlagBackupSemantics | FileFlagOpenReparsePoint,
                IntPtr.Zero)) {
                if (handle.IsInvalid) {
                    throw new Win32Exception(Marshal.GetLastWin32Error(), "Unable to open custody path identity");
                }
                ByHandleFileInformation information;
                if (!GetFileInformationByHandle(handle, out information)) {
                    throw new Win32Exception(Marshal.GetLastWin32Error(), "Unable to read custody path identity");
                }
                return String.Format(
                    "{0:x8}:{1:x8}{2:x8}",
                    information.VolumeSerialNumber,
                    information.FileIndexHigh,
                    information.FileIndexLow);
            }
        }
    }
}
"@
}

function Test-CustodyPathHasReparsePoint {
    param([Parameter(Mandatory)][string]$Path)

    $current = Get-Item -LiteralPath ([IO.Path]::GetFullPath($Path)) -Force
    while ($null -ne $current) {
        if (($current.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { return $true }
        $parent = if ($current -is [IO.DirectoryInfo]) { $current.Parent } else { $current.Directory }
        if ($null -eq $parent -or $parent.FullName -ceq $current.FullName) { break }
        $current = $parent
    }
    return $false
}

function Get-CustodyPathIdentity {
    param([Parameter(Mandatory)][string]$Path)

    $full = [IO.Path]::GetFullPath($Path)
    if (Test-CustodyPathHasReparsePoint -Path $full) {
        throw "Custody path contains a reparse point."
    }
    return [MiniModelDeploymentCustody.NativeMethods]::GetPathIdentity($full)
}

function Get-CustodyByteSha256 {
    param([Parameter(Mandatory)][byte[]]$Bytes)

    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
}

function Get-CustodyFileSha256 {
    param([Parameter(Mandatory)][string]$Path)

    $stream = [IO.File]::OpenRead($Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
        $stream.Dispose()
    }
}

function Assert-CustodyExactKeys {
    param(
        [Parameter(Mandatory)]$Value,
        [Parameter(Mandatory)][string[]]$Expected,
        [Parameter(Mandatory)][string]$Label
    )

    if ($null -eq $Value -or $Value -isnot [psobject]) {
        throw "$Label must be an object."
    }
    $actual = @($Value.PSObject.Properties.Name | Sort-Object -CaseSensitive)
    $wanted = @($Expected | Sort-Object -CaseSensitive)
    if ($actual.Count -ne $wanted.Count) {
        throw "$Label has an unexpected field set."
    }
    for ($index = 0; $index -lt $wanted.Count; $index++) {
        if ($actual[$index] -cne $wanted[$index]) {
            throw "$Label has an unexpected field set."
        }
    }
}

function Assert-CustodyRelativePath {
    param([Parameter(Mandatory)][string]$Path, [switch]$AllowRootDot)

    if ($AllowRootDot -and $Path -ceq ".") { return $Path }
    if ([string]::IsNullOrWhiteSpace($Path) -or
        $Path.Length -gt 512 -or
        $Path.StartsWith("/") -or
        $Path.StartsWith("\") -or
        $Path.EndsWith("/") -or
        $Path.Contains("\") -or
        $Path -match "(^|/)\.\.?(/|$)" -or
        $Path -notmatch "^[A-Za-z0-9._/-]+$" -or
        $Path -match "//") {
        throw "Unsafe relative path."
    }
    return $Path
}

function ConvertFrom-CustodyJsonBytes {
    param([Parameter(Mandatory)][byte[]]$Bytes, [Parameter(Mandatory)][string]$Label)

    if ($Bytes.Length -ge 3 -and $Bytes[0] -eq 0xef -and $Bytes[1] -eq 0xbb -and $Bytes[2] -eq 0xbf) {
        throw "$Label must not contain a UTF-8 byte-order mark."
    }
    try {
        $text = [Text.UTF8Encoding]::new($false, $true).GetString($Bytes)
        return $text | ConvertFrom-Json
    }
    catch {
        throw "$Label is not strict UTF-8 JSON."
    }
}

function Read-CustodyJsonFile {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][int64]$MaximumBytes,
        [Parameter(Mandatory)][string]$Label
    )

    $item = Get-Item -LiteralPath $Path
    if (-not $item -or $item.PSIsContainer -or $item.Length -lt 1 -or $item.Length -gt $MaximumBytes) {
        throw "$Label has an invalid size or type."
    }
    $bytes = [IO.File]::ReadAllBytes($item.FullName)
    return [pscustomobject]@{
        Path = $item.FullName
        Bytes = $bytes
        Sha256 = Get-CustodyByteSha256 -Bytes $bytes
        Value = ConvertFrom-CustodyJsonBytes -Bytes $bytes -Label $Label
    }
}

function Assert-CustodyPublicationAuthority {
    param(
        [Parameter(Mandatory)]$Authority,
        [Parameter(Mandatory)][string]$RepositoryRoot,
        [Parameter(Mandatory)][string]$AuthorityPath
    )

    Assert-CustodyExactKeys -Value $Authority -Expected @(
        "schema", "site", "origin", "repository", "candidate", "predecessor", "public_invariants", "transport"
    ) -Label "publication authority"
    Assert-CustodyExactKeys -Value $Authority.repository -Expected @(
        "root_policy", "branch", "remote_name", "remote_url"
    ) -Label "publication authority repository"
    Assert-CustodyExactKeys -Value $Authority.candidate -Expected @(
        "version", "release_id", "manifest_path", "source_commit", "source_tree"
    ) -Label "publication authority candidate"
    Assert-CustodyExactKeys -Value $Authority.predecessor -Expected @(
        "version", "release_id", "receipt_path", "receipt_sha256", "release_path", "release_bytes",
        "release_sha256", "history_path", "history_bytes", "history_sha256", "ledger_release_ids"
    ) -Label "publication authority predecessor"
    Assert-CustodyExactKeys -Value $Authority.public_invariants -Expected @(
        "composition_catalog", "peer_feed"
    ) -Label "publication authority public invariants"
    foreach ($name in @("composition_catalog", "peer_feed")) {
        Assert-CustodyExactKeys -Value $Authority.public_invariants.$name -Expected @(
            "path", "bytes", "sha256", "array_property"
        ) -Label "publication authority $name"
    }
    Assert-CustodyExactKeys -Value $Authority.transport -Expected @(
        "status", "profile_name", "protocol", "host", "port", "username_sha256", "remote_root",
        "root_sentinel_path", "root_sentinel_source_path", "root_sentinel_bytes", "root_sentinel_sha256",
        "lease_directory_path"
    ) -Label "publication authority transport"

    $actualRoot = [IO.Path]::GetFullPath($RepositoryRoot).TrimEnd(
        [IO.Path]::DirectorySeparatorChar,
        [IO.Path]::AltDirectorySeparatorChar
    )
    $canonicalAuthorityPath = [IO.Path]::GetFullPath((Join-Path $actualRoot "deploy\publication-authority.v1.json"))
    if ($Authority.schema -cne "minimodel.publication_authority.v1" -or
        $Authority.site -cne "minimodel.org" -or
        $Authority.origin -cne "https://minimodel.org" -or
        (Test-CustodyPathHasReparsePoint -Path $actualRoot) -or
        -not [string]::Equals([IO.Path]::GetFullPath($AuthorityPath), $canonicalAuthorityPath, [StringComparison]::OrdinalIgnoreCase) -or
        $Authority.repository.root_policy -cne "script-parent-equals-git-toplevel" -or
        $Authority.repository.branch -cne "master" -or
        $Authority.repository.remote_name -cne "origin" -or
        $Authority.repository.remote_url -cne "https://github.com/MichaelKappel/MiniModel.org.git" -or
        $Authority.candidate.version -cne "1.1.2" -or
        $Authority.candidate.release_id -cne "minimodel.org@1.1.2" -or
        $Authority.candidate.manifest_path -cne "deploy/site-deployment-manifest.v1.json" -or
        [string]$Authority.candidate.source_commit -notmatch "^[0-9a-f]{40}$" -or
        [string]$Authority.candidate.source_tree -notmatch "^[0-9a-f]{40}$" -or
        $Authority.predecessor.version -cne "1.1.1" -or
        $Authority.predecessor.release_id -cne "minimodel.org@1.1.1" -or
        [string]$Authority.predecessor.receipt_sha256 -notmatch "^[0-9a-f]{64}$" -or
        [string]$Authority.predecessor.release_sha256 -notmatch "^[0-9a-f]{64}$" -or
        [string]$Authority.predecessor.history_sha256 -notmatch "^[0-9a-f]{64}$") {
        throw "Publication authority identity is invalid."
    }

    foreach ($relative in @(
        [string]$Authority.candidate.manifest_path,
        [string]$Authority.predecessor.receipt_path,
        [string]$Authority.predecessor.release_path,
        [string]$Authority.predecessor.history_path,
        [string]$Authority.public_invariants.composition_catalog.path,
        [string]$Authority.public_invariants.peer_feed.path,
        [string]$Authority.transport.root_sentinel_path,
        [string]$Authority.transport.root_sentinel_source_path,
        [string]$Authority.transport.lease_directory_path
    )) {
        Assert-CustodyRelativePath -Path $relative | Out-Null
    }

    $sentinelPath = [IO.Path]::GetFullPath((Join-Path $actualRoot (([string]$Authority.transport.root_sentinel_source_path) -replace "/", "\")))
    if (-not (Test-Path -LiteralPath $sentinelPath -PathType Leaf) -or
        (Test-CustodyPathHasReparsePoint -Path $sentinelPath) -or
        (Get-Item -LiteralPath $sentinelPath).Length -ne [int64]$Authority.transport.root_sentinel_bytes -or
        (Get-CustodyFileSha256 -Path $sentinelPath) -cne [string]$Authority.transport.root_sentinel_sha256) {
        throw "Publication root sentinel source identity is invalid."
    }

    if ($Authority.transport.status -ceq "unbound") {
        if ($null -ne $Authority.transport.host -or
            $null -ne $Authority.transport.port -or
            $null -ne $Authority.transport.username_sha256 -or
            $null -ne $Authority.transport.remote_root) {
            throw "Unbound transport authority must not contain a partial tuple."
        }
    }
    elseif ($Authority.transport.status -ceq "bound") {
        if ([Uri]::CheckHostName([string]$Authority.transport.host) -eq [UriHostNameType]::Unknown -or
            [string]$Authority.transport.host -cne ([string]$Authority.transport.host).ToLowerInvariant() -or
            [int]$Authority.transport.port -lt 1 -or [int]$Authority.transport.port -gt 65535 -or
            [string]$Authority.transport.username_sha256 -notmatch "^[0-9a-f]{64}$" -or
            [string]::IsNullOrWhiteSpace([string]$Authority.transport.remote_root)) {
            throw "Bound transport authority has an invalid tuple."
        }
        Assert-CustodyRelativePath -Path ([string]$Authority.transport.remote_root) -AllowRootDot | Out-Null
    }
    else {
        throw "Transport authority status is invalid."
    }
    if ($Authority.transport.profile_name -cne "MiniModel.org" -or
        $Authority.transport.protocol -cne "ftps-explicit" -or
        $Authority.transport.lease_directory_path -cne ".minimodel-deployment-lease.v1" -or
        [string]$Authority.transport.root_sentinel_sha256 -notmatch "^[0-9a-f]{64}$") {
        throw "Transport authority profile or sentinel identity is invalid."
    }
}

function Assert-CustodyRepositoryIdentity {
    param(
        [Parameter(Mandatory)]$Authority,
        [Parameter(Mandatory)]$Observed,
        [Parameter(Mandatory)][string]$ExpectedRepositoryRoot
    )

    Assert-CustodyExactKeys -Value $Observed -Expected @(
        "canonical_root", "root_has_reparse_point", "branch", "upstream", "remotes", "fetch_url", "push_url",
        "head", "tracking_head", "remote_head", "status_count", "source_is_ancestor", "source_tree",
        "manifest_matches_head", "authority_matches_head"
    ) -Label "observed repository identity"

    $expectedRoot = [IO.Path]::GetFullPath($ExpectedRepositoryRoot).TrimEnd(
        [IO.Path]::DirectorySeparatorChar,
        [IO.Path]::AltDirectorySeparatorChar
    )
    $actualRoot = [IO.Path]::GetFullPath([string]$Observed.canonical_root).TrimEnd(
        [IO.Path]::DirectorySeparatorChar,
        [IO.Path]::AltDirectorySeparatorChar
    )
    $remotes = @($Observed.remotes)
    if (-not [string]::Equals($actualRoot, $expectedRoot, [StringComparison]::OrdinalIgnoreCase) -or
        [bool]$Observed.root_has_reparse_point -or
        [string]$Observed.branch -cne [string]$Authority.repository.branch -or
        [string]$Observed.upstream -cne ("{0}/{1}" -f $Authority.repository.remote_name, $Authority.repository.branch) -or
        $remotes.Count -ne 1 -or [string]$remotes[0] -cne [string]$Authority.repository.remote_name -or
        [string]$Observed.fetch_url -cne [string]$Authority.repository.remote_url -or
        [string]$Observed.push_url -cne [string]$Authority.repository.remote_url -or
        [string]$Observed.head -notmatch "^[0-9a-f]{40}$" -or
        [string]$Observed.head -cne [string]$Observed.tracking_head -or
        [string]$Observed.head -cne [string]$Observed.remote_head -or
        [int]$Observed.status_count -ne 0 -or
        -not [bool]$Observed.source_is_ancestor -or
        [string]$Observed.source_tree -cne [string]$Authority.candidate.source_tree -or
        -not [bool]$Observed.manifest_matches_head -or
        -not [bool]$Observed.authority_matches_head) {
        throw "Repository is not the exact clean canonical pushed publication checkout."
    }
}

function Assert-CustodyTransportAuthority {
    param([Parameter(Mandatory)]$Authority, [Parameter(Mandatory)]$Observed)

    if ($Authority.transport.status -cne "bound") {
        throw "Publication transport authority is not bound; credential access is forbidden."
    }
    Assert-CustodyExactKeys -Value $Observed -Expected @(
        "profile_name", "protocol", "host", "port", "username", "remote_root"
    ) -Label "observed transport authority"
    $usernameBytes = [Text.UTF8Encoding]::new($false, $true).GetBytes([string]$Observed.username)
    $usernameSha256 = Get-CustodyByteSha256 -Bytes $usernameBytes
    if ([string]$Observed.profile_name -cne [string]$Authority.transport.profile_name -or
        [string]$Observed.protocol -cne [string]$Authority.transport.protocol -or
        ([string]$Observed.host).ToLowerInvariant() -cne [string]$Authority.transport.host -or
        [int]$Observed.port -ne [int]$Authority.transport.port -or
        $usernameSha256 -cne [string]$Authority.transport.username_sha256 -or
        [string]$Observed.remote_root -cne [string]$Authority.transport.remote_root) {
        throw "Credential profile does not match the immutable publication transport tuple."
    }
}

function Assert-CustodyMutationAllowed {
    param(
        [Parameter(Mandatory)][string]$Phase,
        [Parameter(Mandatory)][ValidateSet("local", "remote")][string]$MutationKind
    )

    $localPhases = @("public-admitted", "snapshot-sealed", "transport-admitted", "leased", "mutating", "rolling-back", "finalizing", "complete")
    $remotePhases = @("transport-admitted", "leased", "mutating", "rolling-back", "finalizing")
    $allowed = if ($MutationKind -ceq "local") { $localPhases -contains $Phase } else { $remotePhases -contains $Phase }
    if (-not $allowed) {
        throw "$MutationKind mutation is forbidden during the $Phase phase."
    }
}

function Resolve-CustodyRemoteFileObservation {
    param(
        [Parameter(Mandatory)][bool]$Listed,
        [Parameter(Mandatory)][ValidateSet("not-probed", "success", "ftp-550", "inaccessible")][string]$SizeStatus,
        [Nullable[int64]]$Size
    )

    if ($SizeStatus -in @("ftp-550", "inaccessible")) {
        throw "Remote SIZE observation is inaccessible or ambiguous."
    }
    if (-not $Listed) {
        if ($SizeStatus -cne "not-probed" -or $null -ne $Size) {
            throw "Remote SIZE and LIST observations contradict each other."
        }
        return [pscustomobject]@{ State = "absent"; Size = $null }
    }
    if ($SizeStatus -cne "success" -or $null -eq $Size -or [int64]$Size -lt 0) {
        throw "A listed remote file lacks one exact SIZE observation."
    }
    return [pscustomobject]@{ State = "present"; Size = [int64]$Size }
}

function Get-CustodyBoundedRollbackOrder {
    param([Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Journal)

    if ($Journal.Count -gt 500) { throw "Remote rollback journal exceeds its bound." }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $Journal) {
        $path = Assert-CustodyRelativePath -Path ([string]$entry.Path)
        if (-not $seen.Add($path)) { throw "Remote rollback journal contains a duplicate target." }
    }
    $ordered = @()
    for ($index = $Journal.Count - 1; $index -ge 0; $index--) {
        $ordered += $Journal[$index]
    }
    return $ordered
}

function Assert-CustodyPredecessorReceipt {
    param([Parameter(Mandatory)]$Authority, [Parameter(Mandatory)][byte[]]$ReceiptBytes)

    if ((Get-CustodyByteSha256 -Bytes $ReceiptBytes) -cne [string]$Authority.predecessor.receipt_sha256) {
        throw "Immutable predecessor receipt identity mismatch."
    }
    $receipt = ConvertFrom-CustodyJsonBytes -Bytes $ReceiptBytes -Label "predecessor receipt"
    if ($receipt.schema -cne "uaix.site_deployment_receipt.v1" -or
        $receipt.site -cne [string]$Authority.site -or
        $receipt.origin -cne [string]$Authority.origin -or
        $receipt.version -cne [string]$Authority.predecessor.version -or
        $receipt.status -cne "published-and-readback-verified" -or
        [string]$receipt.manifest_sha256 -notmatch "^[0-9a-f]{64}$" -or
        @($receipt.files).Count -lt 1 -or
        @($receipt.files).Count -ne [int]$receipt.file_count) {
        throw "Immutable predecessor receipt semantics are invalid."
    }
    $seen = @{}
    foreach ($file in @($receipt.files)) {
        $path = Assert-CustodyRelativePath -Path ([string]$file.path)
        if ($seen.ContainsKey($path) -or [int64]$file.bytes -lt 0 -or [string]$file.sha256 -notmatch "^[0-9a-f]{64}$") {
            throw "Immutable predecessor receipt file closure is invalid."
        }
        $seen[$path] = $true
    }
    foreach ($binding in @(
        $Authority.predecessor.release_path,
        $Authority.predecessor.history_path,
        $Authority.public_invariants.composition_catalog.path,
        $Authority.public_invariants.peer_feed.path
    )) {
        if (-not $seen.ContainsKey([string]$binding)) {
            throw "Immutable predecessor receipt omits a required public binding."
        }
    }
    return $receipt
}

function Assert-CustodyEmptyJsonArrayArtifact {
    param(
        [Parameter(Mandatory)]$Binding,
        [Parameter(Mandatory)][byte[]]$Bytes,
        [Parameter(Mandatory)][string]$Label
    )

    if ($Bytes.Length -ne [int64]$Binding.bytes -or
        (Get-CustodyByteSha256 -Bytes $Bytes) -cne [string]$Binding.sha256) {
        throw "$Label public identity is stale or substituted."
    }
    $value = ConvertFrom-CustodyJsonBytes -Bytes $Bytes -Label $Label
    $property = $value.PSObject.Properties[[string]$Binding.array_property]
    if ($null -eq $property -or @($property.Value).Count -ne 0) {
        throw "$Label must remain truthfully empty."
    }
}

function Assert-CustodyPublicPredecessor {
    param(
        [Parameter(Mandatory)]$Authority,
        [Parameter(Mandatory)][byte[]]$PublicReleaseBytes,
        [Parameter(Mandatory)][byte[]]$PublicHistoryBytes,
        [Parameter(Mandatory)][byte[]]$PublicCatalogBytes,
        [Parameter(Mandatory)][byte[]]$PublicPeerBytes,
        [Parameter(Mandatory)][byte[]]$CandidateReleaseBytes,
        [Parameter(Mandatory)][byte[]]$CandidateHistoryBytes,
        [Parameter(Mandatory)][byte[]]$CandidateCatalogBytes,
        [Parameter(Mandatory)][byte[]]$CandidatePeerBytes
    )

    if ($PublicReleaseBytes.Length -ne [int64]$Authority.predecessor.release_bytes -or
        (Get-CustodyByteSha256 -Bytes $PublicReleaseBytes) -cne [string]$Authority.predecessor.release_sha256) {
        throw "Public release predecessor is not exact 1.1.1."
    }
    if ($PublicHistoryBytes.Length -ne [int64]$Authority.predecessor.history_bytes -or
        (Get-CustodyByteSha256 -Bytes $PublicHistoryBytes) -cne [string]$Authority.predecessor.history_sha256) {
        throw "Public release ledger predecessor is not the exact 1.1.1 ledger."
    }

    $publicRelease = ConvertFrom-CustodyJsonBytes -Bytes $PublicReleaseBytes -Label "public predecessor release"
    $publicHistory = ConvertFrom-CustodyJsonBytes -Bytes $PublicHistoryBytes -Label "public predecessor ledger"
    $candidateRelease = ConvertFrom-CustodyJsonBytes -Bytes $CandidateReleaseBytes -Label "candidate release"
    $candidateHistory = ConvertFrom-CustodyJsonBytes -Bytes $CandidateHistoryBytes -Label "candidate release ledger"
    if ($publicRelease.release_id -cne [string]$Authority.predecessor.release_id -or
        $publicRelease.version -cne [string]$Authority.predecessor.version -or
        $publicRelease.status -cne "published" -or
        $publicHistory.current_release_id -cne [string]$Authority.predecessor.release_id -or
        $candidateRelease.release_id -cne [string]$Authority.candidate.release_id -or
        $candidateRelease.version -cne [string]$Authority.candidate.version -or
        $candidateRelease.status -cne "prepared" -or
        $candidateHistory.current_release_id -cne [string]$Authority.candidate.release_id) {
        throw "Public predecessor or candidate release semantics are invalid."
    }

    $expectedIds = @($Authority.predecessor.ledger_release_ids)
    $publicEntries = @($publicHistory.releases)
    $candidateEntries = @($candidateHistory.releases)
    if ($publicEntries.Count -ne $expectedIds.Count -or $candidateEntries.Count -ne ($expectedIds.Count + 1) -or
        [string]$candidateEntries[0].release_id -cne [string]$Authority.candidate.release_id) {
        throw "Release ledger cardinality or candidate prefix is invalid."
    }
    for ($index = 0; $index -lt $expectedIds.Count; $index++) {
        if ([string]$publicEntries[$index].release_id -cne [string]$expectedIds[$index]) {
            throw "Public predecessor ledger order is invalid."
        }
        $publicEntry = $publicEntries[$index] | ConvertTo-Json -Depth 32 -Compress
        $candidateEntry = $candidateEntries[$index + 1] | ConvertTo-Json -Depth 32 -Compress
        if ($candidateEntry -cne $publicEntry) {
            throw "Candidate release ledger does not preserve the exact public predecessor prefix."
        }
    }

    Assert-CustodyEmptyJsonArrayArtifact -Binding $Authority.public_invariants.composition_catalog -Bytes $PublicCatalogBytes -Label "public composition catalog"
    Assert-CustodyEmptyJsonArrayArtifact -Binding $Authority.public_invariants.peer_feed -Bytes $PublicPeerBytes -Label "public peer feed"
    Assert-CustodyEmptyJsonArrayArtifact -Binding $Authority.public_invariants.composition_catalog -Bytes $CandidateCatalogBytes -Label "candidate composition catalog"
    Assert-CustodyEmptyJsonArrayArtifact -Binding $Authority.public_invariants.peer_feed -Bytes $CandidatePeerBytes -Label "candidate peer feed"
}

function ConvertTo-CustodyPublishedArtifactBytes {
    param(
        [Parameter(Mandatory)][ValidateSet("prepared-release-to-published-v1", "prepared-history-to-published-v1")][string]$Kind,
        [Parameter(Mandatory)][byte[]]$SourceBytes,
        [Parameter(Mandatory)]$Authority
    )

    $value = ConvertFrom-CustodyJsonBytes -Bytes $SourceBytes -Label "prepared publication artifact"
    if ($Kind -ceq "prepared-release-to-published-v1") {
        if ($value.schema -cne "uaix.site_release.v1" -or
            $value.site_id -cne [string]$Authority.site -or
            $value.origin -cne [string]$Authority.origin -or
            $value.release_id -cne [string]$Authority.candidate.release_id -or
            $value.version -cne [string]$Authority.candidate.version -or
            $value.release_date -cne "2026-08-31" -or
            $value.last_publish_date -cne "2026-08-29" -or
            $value.status -cne "prepared") {
            throw "Prepared release marker is not the truthful governed candidate."
        }
        $value.last_publish_date = [string]$value.release_date
        $value.status = "published"
    }
    else {
        $entries = @($value.releases)
        if ($value.schema -cne "uaix.site_release_history.v1" -or
            $value.site_id -cne [string]$Authority.site -or
            $value.origin -cne [string]$Authority.origin -or
            $value.current_release_id -cne [string]$Authority.candidate.release_id -or
            $entries.Count -ne (@($Authority.predecessor.ledger_release_ids).Count + 1) -or
            $entries[0].release_id -cne [string]$Authority.candidate.release_id -or
            $entries[0].version -cne [string]$Authority.candidate.version -or
            $entries[0].release_date -cne "2026-08-31" -or
            $null -ne $entries[0].last_publish_date -or
            $entries[0].status -cne "prepared") {
            throw "Prepared release ledger is not the truthful governed candidate."
        }
        for ($index = 0; $index -lt @($Authority.predecessor.ledger_release_ids).Count; $index++) {
            if ($entries[$index + 1].release_id -cne [string]$Authority.predecessor.ledger_release_ids[$index]) {
                throw "Prepared release ledger changed predecessor order."
            }
        }
        $entries[0].last_publish_date = [string]$entries[0].release_date
        $entries[0].status = "published"
    }

    $text = ($value | ConvertTo-Json -Depth 64 -Compress) + "`n"
    return [Text.UTF8Encoding]::new($false, $true).GetBytes($text)
}

function Assert-CustodyPublishedReleasePair {
    param(
        [Parameter(Mandatory)]$Authority,
        [Parameter(Mandatory)][byte[]]$PreparedReleaseBytes,
        [Parameter(Mandatory)][byte[]]$PreparedHistoryBytes,
        [Parameter(Mandatory)][byte[]]$PublishedReleaseBytes,
        [Parameter(Mandatory)][byte[]]$PublishedHistoryBytes
    )

    $expectedRelease = ConvertTo-CustodyPublishedArtifactBytes `
        -Kind "prepared-release-to-published-v1" `
        -SourceBytes $PreparedReleaseBytes `
        -Authority $Authority
    $expectedHistory = ConvertTo-CustodyPublishedArtifactBytes `
        -Kind "prepared-history-to-published-v1" `
        -SourceBytes $PreparedHistoryBytes `
        -Authority $Authority
    if ($PublishedReleaseBytes.Length -ne $expectedRelease.Length -or
        (Get-CustodyByteSha256 -Bytes $PublishedReleaseBytes) -cne (Get-CustodyByteSha256 -Bytes $expectedRelease) -or
        $PublishedHistoryBytes.Length -ne $expectedHistory.Length -or
        (Get-CustodyByteSha256 -Bytes $PublishedHistoryBytes) -cne (Get-CustodyByteSha256 -Bytes $expectedHistory)) {
        throw "Published release marker or ledger was not exactly materialized from the prepared candidate."
    }

    $release = ConvertFrom-CustodyJsonBytes -Bytes $PublishedReleaseBytes -Label "published release marker"
    $history = ConvertFrom-CustodyJsonBytes -Bytes $PublishedHistoryBytes -Label "published release ledger"
    $entries = @($history.releases)
    if ($release.status -cne "published" -or
        $release.last_publish_date -cne $release.release_date -or
        $history.current_release_id -cne [string]$Authority.candidate.release_id -or
        $entries[0].status -cne "published" -or
        $entries[0].last_publish_date -cne $entries[0].release_date) {
        throw "Published release marker or ledger lacks final publication semantics."
    }
}

function Get-CustodyExpectedDirectoryTree {
    param([Parameter(Mandatory)][string[]]$FilePaths)

    $tree = @{}
    $tree[""] = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($filePathValue in $FilePaths) {
        $filePath = Assert-CustodyRelativePath -Path ([string]$filePathValue)
        $parts = $filePath -split "/"
        $parent = ""
        for ($index = 0; $index -lt $parts.Count; $index++) {
            $tree[$parent].Add($parts[$index]) | Out-Null
            if ($index -lt $parts.Count - 1) {
                $parent = if ([string]::IsNullOrEmpty($parent)) { $parts[$index] } else { "$parent/$($parts[$index])" }
                if (-not $tree.ContainsKey($parent)) {
                    $tree[$parent] = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
                }
            }
        }
    }
    return $tree
}

function Assert-CustodyRemoteDirectoryTree {
    param(
        [Parameter(Mandatory)][string[]]$ExpectedFilePaths,
        [Parameter(Mandatory)][Collections.IDictionary]$ObservedTree
    )

    $expectedTree = Get-CustodyExpectedDirectoryTree -FilePaths $ExpectedFilePaths
    $expectedDirectories = @($expectedTree.Keys | Sort-Object -CaseSensitive)
    $observedDirectories = @($ObservedTree.Keys | ForEach-Object { [string]$_ } | Sort-Object -CaseSensitive)
    if ($expectedDirectories.Count -ne $observedDirectories.Count) {
        throw "Remote publication root directory closure is not exact."
    }
    for ($index = 0; $index -lt $expectedDirectories.Count; $index++) {
        if ($expectedDirectories[$index] -cne $observedDirectories[$index]) {
            throw "Remote publication root contains an unknown or missing directory."
        }
        $directory = $expectedDirectories[$index]
        $expectedChildren = @($expectedTree[$directory] | Sort-Object -CaseSensitive)
        $observedChildren = @($ObservedTree[$directory] | ForEach-Object { [string]$_ } | Sort-Object -CaseSensitive)
        if ($expectedChildren.Count -ne $observedChildren.Count) {
            throw "Remote publication root contains an overlay or missing entry."
        }
        for ($childIndex = 0; $childIndex -lt $expectedChildren.Count; $childIndex++) {
            if ($expectedChildren[$childIndex] -cne $observedChildren[$childIndex]) {
                throw "Remote publication root contains an overlay or missing entry."
            }
        }
    }
}

function New-CustodyLeaseOwnerBytes {
    param(
        [Parameter(Mandatory)]$Authority,
        [Parameter(Mandatory)][string]$OperationId,
        [Parameter(Mandatory)][string]$PublisherCommit,
        [Parameter(Mandatory)][string]$ManifestSha256,
        [Parameter(Mandatory)][string]$AuthoritySha256
    )

    if ($OperationId -notmatch "^[0-9a-f]{32}$" -or
        $PublisherCommit -notmatch "^[0-9a-f]{40}$" -or
        $ManifestSha256 -notmatch "^[0-9a-f]{64}$" -or
        $AuthoritySha256 -notmatch "^[0-9a-f]{64}$") {
        throw "Remote deployment lease identity is invalid."
    }
    $owner = [ordered]@{
        schema = "minimodel.deployment_lease.v1"
        operation_id = $OperationId
        site = [string]$Authority.site
        release_id = [string]$Authority.candidate.release_id
        source_commit = [string]$Authority.candidate.source_commit
        publisher_commit = $PublisherCommit
        manifest_sha256 = $ManifestSha256
        authority_sha256 = $AuthoritySha256
    }
    return [Text.UTF8Encoding]::new($false, $true).GetBytes((($owner | ConvertTo-Json -Depth 8 -Compress) + "`n"))
}

function Assert-CustodyLeaseOwnership {
    param(
        [Parameter(Mandatory)][byte[]]$ExpectedOwnerBytes,
        [Parameter(Mandatory)][byte[]]$ObservedOwnerBytes,
        [Parameter(Mandatory)][string]$OperationId
    )

    if ($OperationId -notmatch "^[0-9a-f]{32}$" -or
        $ExpectedOwnerBytes.Length -lt 1 -or
        $ExpectedOwnerBytes.Length -gt 8192 -or
        $ObservedOwnerBytes.Length -ne $ExpectedOwnerBytes.Length -or
        (Get-CustodyByteSha256 -Bytes $ObservedOwnerBytes) -cne (Get-CustodyByteSha256 -Bytes $ExpectedOwnerBytes)) {
        throw "Remote deployment lease is not owned by this exact operation."
    }
    $owner = ConvertFrom-CustodyJsonBytes -Bytes $ObservedOwnerBytes -Label "remote deployment lease owner"
    Assert-CustodyExactKeys -Value $owner -Expected @(
        "schema", "operation_id", "site", "release_id", "source_commit", "publisher_commit",
        "manifest_sha256", "authority_sha256"
    ) -Label "remote deployment lease owner"
    if ($owner.schema -cne "minimodel.deployment_lease.v1" -or $owner.operation_id -cne $OperationId) {
        throw "Remote deployment lease is not owned by this exact operation."
    }
}

function Get-CustodyGitBlobBytes {
    param(
        [Parameter(Mandatory)][string]$RepositoryRoot,
        [Parameter(Mandatory)][string]$Commit,
        [Parameter(Mandatory)][string]$RelativePath
    )

    $path = Assert-CustodyRelativePath -Path $RelativePath
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = "git"
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.CreateNoWindow = $true
    if ($start.PSObject.Properties.Name -contains "ArgumentList") {
        $start.ArgumentList.Add("--no-optional-locks")
        $start.ArgumentList.Add("-C")
        $start.ArgumentList.Add($RepositoryRoot)
        $start.ArgumentList.Add("cat-file")
        $start.ArgumentList.Add("blob")
        $start.ArgumentList.Add("${Commit}:$path")
    }
    else {
        $escapedRoot = $RepositoryRoot.Replace('"', '\"')
        $escapedObject = ("${Commit}:$path").Replace('"', '\"')
        $start.Arguments = "--no-optional-locks -C `"$escapedRoot`" cat-file blob `"$escapedObject`""
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) { throw "Unable to start Git blob reader." }
    $memory = [IO.MemoryStream]::new()
    try {
        $process.StandardOutput.BaseStream.CopyTo($memory)
        $errorText = $process.StandardError.ReadToEnd()
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) { throw "Git blob read failed: $errorText" }
        return $memory.ToArray()
    }
    finally {
        $memory.Dispose()
        $process.Dispose()
    }
}

function New-CustodySealedSnapshot {
    param(
        [Parameter(Mandatory)][string]$Phase,
        [Parameter(Mandatory)][string]$RepositoryRoot,
        [Parameter(Mandatory)]$Manifest,
        [Parameter(Mandatory)][string]$ManifestSha256,
        [Parameter(Mandatory)][string]$SnapshotParent,
        [Parameter(Mandatory)][string]$OperationId,
        $Authority,
        [scriptblock]$BlobReader
    )

    Assert-CustodyMutationAllowed -Phase $Phase -MutationKind local
    if ($OperationId -notmatch "^[0-9a-f]{32}$" -or $ManifestSha256 -notmatch "^[0-9a-f]{64}$") {
        throw "Snapshot operation or manifest identity is invalid."
    }
    $repositoryFull = [IO.Path]::GetFullPath($RepositoryRoot).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    $parentFull = [IO.Path]::GetFullPath($SnapshotParent).TrimEnd([IO.Path]::DirectorySeparatorChar)
    $snapshotRoot = [IO.Path]::GetFullPath((Join-Path $parentFull $OperationId))
    if ($snapshotRoot.StartsWith($repositoryFull, [StringComparison]::OrdinalIgnoreCase) -or
        -not $snapshotRoot.StartsWith($parentFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
        (Test-Path -LiteralPath $snapshotRoot)) {
        throw "Snapshot root is occupied or not operation-owned outside the checkout."
    }
    [IO.Directory]::CreateDirectory($parentFull) | Out-Null
    if (Test-CustodyPathHasReparsePoint -Path $parentFull) {
        throw "Snapshot parent path must not contain a reparse point."
    }
    [IO.Directory]::CreateDirectory($snapshotRoot) | Out-Null
    $rootIdentity = Get-CustodyPathIdentity -Path $snapshotRoot

    $materializations = @{}
    $materializationProperty = $Manifest.PSObject.Properties["materializations"]
    if ($null -ne $materializationProperty) {
        foreach ($materialization in @($materializationProperty.Value)) {
            Assert-CustodyExactKeys -Value $materialization -Expected @(
                "path", "kind", "source_bytes", "source_sha256"
            ) -Label "deployment materialization"
            $materializedPath = Assert-CustodyRelativePath -Path ([string]$materialization.path)
            if ($materializations.ContainsKey($materializedPath) -or
                [string]$materialization.kind -notin @(
                    "prepared-release-to-published-v1",
                    "prepared-history-to-published-v1"
                ) -or
                [int64]$materialization.source_bytes -lt 1 -or
                [string]$materialization.source_sha256 -notmatch "^[0-9a-f]{64}$") {
                throw "Deployment materialization identity is invalid."
            }
            $materializations[$materializedPath] = $materialization
        }
    }
    if ($materializations.Count -gt 0 -and $null -eq $Authority) {
        throw "Publication authority is required for governed materialization."
    }

    $sealedFiles = @()
    $usedMaterializations = @{}
    try {
        foreach ($file in @($Manifest.files)) {
            $relative = Assert-CustodyRelativePath -Path ([string]$file.path)
            $destination = [IO.Path]::GetFullPath((Join-Path $snapshotRoot ($relative -replace "/", "\")))
            if (-not $destination.StartsWith($snapshotRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
                throw "Snapshot member escaped the operation root."
            }
            [IO.Directory]::CreateDirectory((Split-Path -Parent $destination)) | Out-Null
            $bytes = if ($BlobReader) {
                & $BlobReader ([string]$Manifest.source_commit) $relative
            }
            else {
                Get-CustodyGitBlobBytes -RepositoryRoot $RepositoryRoot -Commit ([string]$Manifest.source_commit) -RelativePath $relative
            }
            if ($bytes -isnot [byte[]]) { $bytes = [byte[]]$bytes }
            if ($materializations.ContainsKey($relative)) {
                $materialization = $materializations[$relative]
                if ($bytes.Length -ne [int64]$materialization.source_bytes -or
                    (Get-CustodyByteSha256 -Bytes $bytes) -cne [string]$materialization.source_sha256) {
                    throw "Prepared Git blob does not match its governed materialization source: $relative"
                }
                $bytes = ConvertTo-CustodyPublishedArtifactBytes `
                    -Kind ([string]$materialization.kind) `
                    -SourceBytes $bytes `
                    -Authority $Authority
                $usedMaterializations[$relative] = $true
            }
            if ($bytes.Length -ne [int64]$file.bytes -or (Get-CustodyByteSha256 -Bytes $bytes) -cne [string]$file.sha256) {
                throw "Governed publication bytes do not match the deployment manifest: $relative"
            }
            [IO.File]::WriteAllBytes($destination, $bytes)
            (Get-Item -LiteralPath $destination).IsReadOnly = $true
            $sealedFiles += [ordered]@{ path = $relative; bytes = [int64]$file.bytes; sha256 = [string]$file.sha256 }
        }
        if ($usedMaterializations.Count -ne $materializations.Count) {
            throw "Deployment manifest contains an unused materialization."
        }
        $seal = [ordered]@{
            schema = "minimodel.publication_snapshot.v1"
            operation_id = $OperationId
            root_identity = $rootIdentity
            manifest_sha256 = $ManifestSha256
            source_commit = [string]$Manifest.source_commit
            source_tree = [string]$Manifest.source_tree
            files = $sealedFiles
        }
        $sealPath = Join-Path $snapshotRoot ".minimodel-publication-snapshot.v1.json"
        [IO.File]::WriteAllText($sealPath, (($seal | ConvertTo-Json -Depth 8) + "`n"), [Text.UTF8Encoding]::new($false))
        (Get-Item -LiteralPath $sealPath).IsReadOnly = $true
        return [pscustomobject]@{
            Root = $snapshotRoot
            SealPath = $sealPath
            SealSha256 = Get-CustodyFileSha256 -Path $sealPath
            OperationId = $OperationId
            RootIdentity = $rootIdentity
        }
    }
    catch {
        throw
    }
}

function Assert-CustodySealedSnapshot {
    param(
        [Parameter(Mandatory)][string]$SnapshotRoot,
        [Parameter(Mandatory)][string]$SnapshotParent,
        [Parameter(Mandatory)]$Manifest,
        [Parameter(Mandatory)][string]$SealSha256,
        [Parameter(Mandatory)][string]$OperationId,
        [Parameter(Mandatory)][string]$RootIdentity
    )

    $root = [IO.Path]::GetFullPath($SnapshotRoot)
    $parent = [IO.Path]::GetFullPath($SnapshotParent).TrimEnd([IO.Path]::DirectorySeparatorChar)
    $expectedRoot = [IO.Path]::GetFullPath((Join-Path $parent $OperationId))
    if ($OperationId -notmatch "^[0-9a-f]{32}$" -or
        $RootIdentity -notmatch "^[0-9a-f]{8}:[0-9a-f]{16}$" -or
        $SealSha256 -notmatch "^[0-9a-f]{64}$" -or
        -not [string]::Equals($root, $expectedRoot, [StringComparison]::OrdinalIgnoreCase) -or
        -not (Test-Path -LiteralPath $root -PathType Container) -or
        (Test-CustodyPathHasReparsePoint -Path $root) -or
        (Get-CustodyPathIdentity -Path $root) -cne $RootIdentity) {
        throw "Publication snapshot root was replaced or is not exactly operation-owned."
    }
    $sealPath = Join-Path $root ".minimodel-publication-snapshot.v1.json"
    if (-not (Test-Path -LiteralPath $sealPath -PathType Leaf) -or
        (Test-CustodyPathHasReparsePoint -Path $sealPath) -or
        (Get-CustodyFileSha256 -Path $sealPath) -cne $SealSha256) {
        throw "Publication snapshot seal changed."
    }
    $sealRecord = Read-CustodyJsonFile -Path $sealPath -MaximumBytes 2097152 -Label "publication snapshot seal"
    $seal = $sealRecord.Value
    Assert-CustodyExactKeys -Value $seal -Expected @(
        "schema", "operation_id", "root_identity", "manifest_sha256", "source_commit", "source_tree", "files"
    ) -Label "publication snapshot seal"
    if ($seal.schema -cne "minimodel.publication_snapshot.v1" -or
        $seal.operation_id -cne $OperationId -or
        $seal.root_identity -cne $RootIdentity -or
        $seal.source_commit -cne [string]$Manifest.source_commit -or
        $seal.source_tree -cne [string]$Manifest.source_tree -or
        @($seal.files).Count -ne @($Manifest.files).Count) {
        throw "Publication snapshot seal ownership changed."
    }

    $expectedEntries = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $expectedEntries.Add("f:.minimodel-publication-snapshot.v1.json") | Out-Null
    foreach ($file in @($Manifest.files)) {
        $relative = Assert-CustodyRelativePath -Path ([string]$file.path)
        $expectedEntries.Add("f:$relative") | Out-Null
        $parts = $relative -split "/"
        if ($parts.Count -gt 1) {
            $directoryParts = @()
            for ($partIndex = 0; $partIndex -lt $parts.Count - 1; $partIndex++) {
                $directoryParts += $parts[$partIndex]
                $expectedEntries.Add("d:$($directoryParts -join '/')") | Out-Null
            }
        }
        $path = [IO.Path]::GetFullPath((Join-Path $root ($relative -replace "/", "\")))
        if (-not $path.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
            -not (Test-Path -LiteralPath $path -PathType Leaf) -or
            (Test-CustodyPathHasReparsePoint -Path $path) -or
            (Get-Item -LiteralPath $path).Length -ne [int64]$file.bytes -or
            (Get-CustodyFileSha256 -Path $path) -cne [string]$file.sha256) {
            throw "Publication snapshot member changed: $relative"
        }
    }

    for ($index = 0; $index -lt @($Manifest.files).Count; $index++) {
        $manifestFile = @($Manifest.files)[$index]
        $sealedFile = @($seal.files)[$index]
        if ($sealedFile.path -cne $manifestFile.path -or
            [int64]$sealedFile.bytes -ne [int64]$manifestFile.bytes -or
            $sealedFile.sha256 -cne $manifestFile.sha256) {
            throw "Publication snapshot seal file closure changed."
        }
    }

    $actualEntries = @(Get-ChildItem -LiteralPath $root -Recurse -Force | ForEach-Object {
        if (($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Publication snapshot contains a reparse point."
        }
        $kind = if ($_.PSIsContainer) { "d:" } else { "f:" }
        $kind + $_.FullName.Substring($root.Length + 1).Replace("\", "/")
    })
    if ($actualEntries.Count -ne $expectedEntries.Count) {
        throw "Publication snapshot contains an overlay or missing entry."
    }
    foreach ($entry in $actualEntries) {
        if (-not $expectedEntries.Contains([string]$entry)) {
            throw "Publication snapshot contains an overlay or missing entry."
        }
    }
}

function Remove-CustodySealedSnapshot {
    param(
        [Parameter(Mandatory)][string]$Phase,
        [Parameter(Mandatory)][string]$SnapshotRoot,
        [Parameter(Mandatory)][string]$SnapshotParent,
        [Parameter(Mandatory)][string]$RepositoryRoot,
        [Parameter(Mandatory)]$Manifest,
        [Parameter(Mandatory)][string]$SealSha256,
        [Parameter(Mandatory)][string]$OperationId,
        [Parameter(Mandatory)][string]$RootIdentity
    )

    Assert-CustodyMutationAllowed -Phase $Phase -MutationKind local
    $root = [IO.Path]::GetFullPath($SnapshotRoot)
    $parent = [IO.Path]::GetFullPath($SnapshotParent).TrimEnd([IO.Path]::DirectorySeparatorChar)
    $repository = [IO.Path]::GetFullPath($RepositoryRoot).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $root.StartsWith($parent + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
        $root.StartsWith($repository, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove an unowned publication snapshot path."
    }
    Assert-CustodySealedSnapshot `
        -SnapshotRoot $root `
        -SnapshotParent $parent `
        -Manifest $Manifest `
        -SealSha256 $SealSha256 `
        -OperationId $OperationId `
        -RootIdentity $RootIdentity
    if ((Get-CustodyPathIdentity -Path $root) -cne $RootIdentity) {
        throw "Refusing cleanup after publication snapshot root replacement."
    }
    $directories = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($file in @($Manifest.files)) {
        $relative = Assert-CustodyRelativePath -Path ([string]$file.path)
        $memberPath = [IO.Path]::GetFullPath((Join-Path $root ($relative -replace "/", "\")))
        if (Test-CustodyPathHasReparsePoint -Path $memberPath) {
            throw "Refusing cleanup after publication snapshot member replacement."
        }
        $member = Get-Item -LiteralPath $memberPath -Force
        if ($member.Length -ne [int64]$file.bytes -or
            (Get-CustodyFileSha256 -Path $memberPath) -cne [string]$file.sha256) {
            throw "Refusing cleanup after publication snapshot member change."
        }
        $member.IsReadOnly = $false
        [IO.File]::Delete($memberPath)

        $parentPath = [IO.Path]::GetDirectoryName($memberPath)
        while (-not [string]::Equals($parentPath, $root, [StringComparison]::OrdinalIgnoreCase)) {
            $directories.Add($parentPath) | Out-Null
            $parentPath = [IO.Path]::GetDirectoryName($parentPath)
        }
    }
    $sealPath = Join-Path $root ".minimodel-publication-snapshot.v1.json"
    if ((Test-CustodyPathHasReparsePoint -Path $sealPath) -or
        (Get-CustodyFileSha256 -Path $sealPath) -cne $SealSha256) {
        throw "Refusing cleanup after publication snapshot seal replacement."
    }
    (Get-Item -LiteralPath $sealPath -Force).IsReadOnly = $false
    [IO.File]::Delete($sealPath)

    foreach ($directory in @($directories | Sort-Object { $_.Length } -Descending)) {
        if (Test-CustodyPathHasReparsePoint -Path $directory) {
            throw "Refusing cleanup after publication snapshot directory replacement."
        }
        [IO.Directory]::Delete($directory, $false)
    }
    if ((Get-CustodyPathIdentity -Path $root) -cne $RootIdentity) {
        throw "Refusing cleanup after publication snapshot root replacement."
    }
    [IO.Directory]::Delete($root, $false)
    if (Test-Path -LiteralPath $root) { throw "Publication snapshot cleanup did not complete." }
}

Export-ModuleMember -Function @(
    "Get-CustodyByteSha256",
    "Get-CustodyFileSha256",
    "Get-CustodyPathIdentity",
    "Read-CustodyJsonFile",
    "Assert-CustodyPublicationAuthority",
    "Assert-CustodyRepositoryIdentity",
    "Assert-CustodyTransportAuthority",
    "Assert-CustodyMutationAllowed",
    "Resolve-CustodyRemoteFileObservation",
    "Get-CustodyBoundedRollbackOrder",
    "Assert-CustodyPredecessorReceipt",
    "Assert-CustodyPublicPredecessor",
    "ConvertTo-CustodyPublishedArtifactBytes",
    "Assert-CustodyPublishedReleasePair",
    "Get-CustodyExpectedDirectoryTree",
    "Assert-CustodyRemoteDirectoryTree",
    "New-CustodyLeaseOwnerBytes",
    "Assert-CustodyLeaseOwnership",
    "Get-CustodyGitBlobBytes",
    "New-CustodySealedSnapshot",
    "Assert-CustodySealedSnapshot",
    "Remove-CustodySealedSnapshot"
)
