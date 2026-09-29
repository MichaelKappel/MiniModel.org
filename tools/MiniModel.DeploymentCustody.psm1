Set-StrictMode -Version Latest

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

function Get-CustodyByteSha256 {
    param([Parameter(Mandatory)][AllowEmptyCollection()][byte[]]$Bytes)

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

# A publication date is a reviewed UTC transaction date, separate from source preparation.
function Assert-CustodyPublicationDate {
    param([Parameter(Mandatory)]$Date)
    $parsed = [DateTime]::MinValue
    if ($Date -isnot [string] -or $Date.Length -ne 10 -or
        -not [DateTime]::TryParseExact($Date, 'yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::None, [ref]$parsed) -or $Date -clt '2026-08-31') {
        throw 'Publication date must be an exact UTC calendar date on or after source preparation.'
    }
}

function Assert-CustodyPublicationAuthority {
    param(
        [Parameter(Mandatory)]$Authority,
        [Parameter(Mandatory)][string]$RepositoryRoot,
        [Parameter(Mandatory)][string]$AuthorityPath
    )

    Assert-CustodyExactKeys -Value $Authority -Expected @(
        "schema", "site", "origin", "repository", "candidate", "predecessor", "predecessor_discovery", "candidate_discovery", "transport"
    ) -Label "publication authority"
    Assert-CustodyExactKeys -Value $Authority.repository -Expected @(
        "root_policy", "branch", "remote_name", "remote_url"
    ) -Label "publication authority repository"
    Assert-CustodyExactKeys -Value $Authority.candidate -Expected @(
        "version", "release_id", "manifest_path", "source_commit", "source_tree", "publication_date"
    ) -Label "publication authority candidate"
    Assert-CustodyExactKeys -Value $Authority.predecessor -Expected @(
        "version", "release_id", "receipt_path", "receipt_sha256", "release_path", "release_bytes",
        "release_sha256", "history_path", "history_bytes", "history_sha256", "ledger_release_ids"
    ) -Label "publication authority predecessor"
    Assert-CustodyPublicationDate -Date $Authority.candidate.publication_date
    foreach ($group in @("predecessor_discovery", "candidate_discovery")) {
        Assert-CustodyExactKeys -Value $Authority.$group -Expected @("composition_catalog", "peer_feed") -Label "publication authority $group"
        foreach ($name in @("composition_catalog", "peer_feed")) {
            $binding = $Authority.$group.$name
            Assert-CustodyExactKeys -Value $binding -Expected @("path", "bytes", "sha256", "array_property") -Label "publication authority $group $name"
            $expectedPath = if ($name -ceq 'composition_catalog') { 'catalog/compositions.v1.json' } else { 'catalog/peer-announcements.v1.json' }
            $expectedArray = if ($name -ceq 'composition_catalog') { 'entries' } else { 'announcements' }
            if ($binding.path -cne $expectedPath -or $binding.array_property -cne $expectedArray -or
                $binding.bytes -isnot [int] -or $binding.bytes -lt 1 -or $binding.bytes -gt 16777216 -or
                $binding.sha256 -isnot [string] -or $binding.sha256.Length -ne 64 -or $binding.sha256 -cnotmatch '^[0-9a-f]{64}$') {
                throw "Publication discovery binding is invalid."
            }
        }
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
        [string]$Authority.predecessor_discovery.composition_catalog.path,
        [string]$Authority.predecessor_discovery.peer_feed.path,
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

    $localPhases = @("public-admitted", "input-verified", "transport-admitted", "leased", "mutating", "rolling-back", "finalizing", "complete")
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
        $Authority.predecessor_discovery.composition_catalog.path,
        $Authority.predecessor_discovery.peer_feed.path
    )) {
        if (-not $seen.ContainsKey([string]$binding)) {
            throw "Immutable predecessor receipt omits a required public binding."
        }
    }
    return $receipt
}

function Assert-CustodyJsonArrayArtifact {
    param(
        [Parameter(Mandatory)]$Binding,
        [Parameter(Mandatory)][byte[]]$Bytes,
        [Parameter(Mandatory)][string]$Label,
        [switch]$RequireEmpty
    )

    if ($Bytes.Length -ne [int64]$Binding.bytes -or
        (Get-CustodyByteSha256 -Bytes $Bytes) -cne [string]$Binding.sha256) {
        throw "$Label public identity is stale or substituted."
    }
    $value = ConvertFrom-CustodyJsonBytes -Bytes $Bytes -Label $Label
    $property = $value.PSObject.Properties[[string]$Binding.array_property]
    if ($null -eq $property -or $property.Value -isnot [Array] -or ($RequireEmpty -and @($property.Value).Count -ne 0)) {
        throw "$Label must contain its declared JSON array (empty for the predecessor)."
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

    Assert-CustodyJsonArrayArtifact -Binding $Authority.predecessor_discovery.composition_catalog -Bytes $PublicCatalogBytes -Label "public composition catalog" -RequireEmpty
    Assert-CustodyJsonArrayArtifact -Binding $Authority.predecessor_discovery.peer_feed -Bytes $PublicPeerBytes -Label "public peer feed" -RequireEmpty
    Assert-CustodyJsonArrayArtifact -Binding $Authority.candidate_discovery.composition_catalog -Bytes $CandidateCatalogBytes -Label "candidate composition catalog"
    Assert-CustodyJsonArrayArtifact -Binding $Authority.candidate_discovery.peer_feed -Bytes $CandidatePeerBytes -Label "candidate peer feed"
}

function ConvertTo-CustodyPublishedArtifactBytes {
    param(
        [Parameter(Mandatory)][ValidateSet("prepared-release-to-published-v1", "prepared-history-to-published-v1")][string]$Kind,
        [Parameter(Mandatory)][byte[]]$SourceBytes,
        [Parameter(Mandatory)]$Authority
    )

    $value = ConvertFrom-CustodyJsonBytes -Bytes $SourceBytes -Label "prepared publication artifact"
    Assert-CustodyPublicationDate -Date $Authority.candidate.publication_date
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
        $value.last_publish_date = [string]$Authority.candidate.publication_date
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
        $entries[0].last_publish_date = [string]$Authority.candidate.publication_date
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
        $release.last_publish_date -cne $Authority.candidate.publication_date -or
        $history.current_release_id -cne [string]$Authority.candidate.release_id -or
        $entries[0].status -cne "published" -or
        $entries[0].last_publish_date -cne $Authority.candidate.publication_date) {
        throw "Published release marker or ledger lacks final publication semantics."
    }
}

# A colon is admitted only for this one descriptor-derived six-member namespace.
# This does not expand the local-source or general website path grammar.
function Assert-CustodyRemotePath {
    param(
        [Parameter(Mandatory)][string]$Path,
        [AllowEmptyCollection()][string[]]$SeedFilePaths = @(),
        [string]$SeedOperationId = '',
        [switch]$AllowAncestor
    )

    if ($SeedOperationId.Length -gt 0 -and
        ($SeedOperationId.Length -ne 32 -or $SeedOperationId -cnotmatch '^[0-9a-f]{32}$' -or $SeedFilePaths.Count -ne 7)) {
        throw 'Static seed staging operation identity is invalid.'
    }
    $allowed = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    if ($SeedFilePaths.Count -ne 0) {
        if ($SeedFilePaths.Count -ne 7 -or
            $SeedFilePaths[0] -cnotmatch '^(tinyrustlm/direct-composition/v1/compositions/sha256:[0-9a-f]{64}/members)/model\.slm2$') {
            throw 'Registered static seed namespace is invalid.'
        }
        $directory = $Matches[1]
        $names = @('model.slm2','tokenizer.tokenizer2','template.template2','sampling.sampling2','prompt.prompt2','composition.acg2','.htaccess')
        for ($index = 0; $index -lt $names.Count; $index++) {
            if ($SeedFilePaths[$index] -cne "$directory/$($names[$index])") {
                throw 'Registered static seed namespace is not the exact member closure.'
            }
            $null = $allowed.Add($SeedFilePaths[$index])
            if ($SeedOperationId.Length -gt 0) {
                $null = $allowed.Add("$($SeedFilePaths[$index]).seed-$SeedOperationId.tmp")
            }
        }
        if ($AllowAncestor) {
            $ancestor = $directory
            while ($ancestor.Contains('/')) {
                $null = $allowed.Add($ancestor)
                $ancestor = $ancestor.Substring(0,$ancestor.LastIndexOf('/'))
            }
            $null = $allowed.Add($ancestor)
        }
    }
    if ($Path.Contains(':')) {
        if (-not $allowed.Contains($Path)) { throw 'Remote colon path is outside the registered static seed namespace.' }
        return $Path
    }
    return Assert-CustodyRelativePath -Path $Path
}

function Get-CustodyExpectedDirectoryTree {
    param(
        [Parameter(Mandatory)][string[]]$FilePaths,
        [AllowEmptyCollection()][string[]]$SeedFilePaths = @(),
        [string]$SeedOperationId = ''
    )

    $tree = @{}
    $tree[""] = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($filePathValue in $FilePaths) {
        $filePath = Assert-CustodyRemotePath -Path ([string]$filePathValue) -SeedFilePaths $SeedFilePaths -SeedOperationId $SeedOperationId
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

# Decode the bounded NLST response; only exact self/parent records are structural.
function ConvertFrom-CustodyDirectoryListing {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][byte[]]$Bytes,
        [string]$DirectoryPath = '',
        [AllowEmptyCollection()][string[]]$SeedFilePaths = @(),
        [string]$SeedOperationId = ''
    )
    if ($Bytes.Length -gt 65536) { throw 'Remote directory listing exceeds its byte bound.' }
    if ($DirectoryPath.Length -gt 0) {
        $null = Assert-CustodyRemotePath -Path $DirectoryPath -SeedFilePaths $SeedFilePaths -SeedOperationId $SeedOperationId -AllowAncestor
    }
    try { $text = [Text.UTF8Encoding]::new($false,$true).GetString($Bytes) }
    catch { throw 'Remote directory listing is not UTF-8.' }
    $entries = [Collections.Generic.List[string]]::new()
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in @($text -split '\r?\n')) {
        if ($entry -ceq '' -or $entry -ceq '.' -or $entry -ceq '..') { continue }
        if ($entry.Length -gt 512 -or $entry -cnotmatch '^[A-Za-z0-9._:-]+$') {
            throw 'Remote directory listing contains an unsafe entry.'
        }
        $logical = if ($DirectoryPath.Length -eq 0) { $entry } else { "$DirectoryPath/$entry" }
        $null = Assert-CustodyRemotePath -Path $logical -SeedFilePaths $SeedFilePaths -SeedOperationId $SeedOperationId -AllowAncestor
        if (-not $seen.Add($entry)) { throw 'Remote directory listing contains a duplicate or case alias.' }
        $entries.Add($entry)
        if ($entries.Count -gt 500) { throw 'Remote directory listing exceeds its entry bound.' }
    }
    return $entries.ToArray()
}

function Assert-CustodyBootstrapManifest {
    param([Parameter(Mandatory)]$Manifest,[Parameter(Mandatory)]$Authority)
    Assert-CustodyExactKeys -Value $Manifest -Expected @('schema','observed_at_utc','root_entries','public_matches') -Label 'root bootstrap manifest'
    if ($Manifest.schema -cne 'minimodel.root-bootstrap.v1' -or
        [string]$Manifest.observed_at_utc -cnotmatch '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$') {
        throw 'Root bootstrap manifest identity is invalid.'
    }
    $entries = @($Manifest.root_entries)
    if ($entries.Count -lt 3 -or $entries.Count -gt 64) { throw 'Root bootstrap inventory is outside its bound.' }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in $entries) {
        if ($entry -isnot [string] -or $entry -cnotmatch '^[A-Za-z0-9._-]{1,128}$' -or
            $entry -cin @('.','..',[string]$Authority.transport.root_sentinel_path,[string]$Authority.transport.lease_directory_path) -or
            -not $seen.Add($entry)) { throw 'Root bootstrap inventory contains an unsafe or occupied control entry.' }
    }
    $publicMatches = @($Manifest.public_matches)
    $paths = @('index.html','catalog/compositions.v1.json','catalog/peer-announcements.v1.json')
    if ($publicMatches.Count -ne $paths.Count) { throw 'Root bootstrap public evidence is incomplete.' }
    for ($index=0;$index -lt $paths.Count;$index++) {
        $item=$publicMatches[$index]
        Assert-CustodyExactKeys -Value $item -Expected @('path','bytes','sha256') -Label 'bootstrap public match'
        if ($item.path -cne $paths[$index] -or $item.bytes -isnot [int] -or $item.bytes -lt 1 -or $item.bytes -gt 2097152 -or
            ([string]$item.sha256).Length -ne 64 -or [string]$item.sha256 -cnotmatch '^[0-9a-f]{64}$') {
            throw 'Root bootstrap public evidence identity is invalid.'
        }
    }
}

function Assert-CustodyRemoteDirectoryTree {
    param(
        [Parameter(Mandatory)][string[]]$ExpectedFilePaths,
        [Parameter(Mandatory)][Collections.IDictionary]$ObservedTree,
        [AllowEmptyCollection()][string[]]$SeedFilePaths = @(),
        [string]$SeedOperationId = ''
    )

    $expectedTree = Get-CustodyExpectedDirectoryTree -FilePaths $ExpectedFilePaths -SeedFilePaths $SeedFilePaths -SeedOperationId $SeedOperationId
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
        [Parameter(Mandatory)][string]$RelativePath,
        [ValidateRange(0,16777216)][int]$MaximumBytes = 16777216
    )

    $root = Assert-CustodyCanonicalRepositoryRoot -RepositoryRoot $RepositoryRoot
    if ($Commit -cnotmatch '^[0-9a-f]{40}$') { throw 'Git publication commit must be an exact object identity.' }
    $path = Assert-CustodyRelativePath -Path $RelativePath
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = "git"
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.CreateNoWindow = $true
    $start.EnvironmentVariables['GIT_OPTIONAL_LOCKS'] = '0'
    $start.EnvironmentVariables['GIT_TERMINAL_PROMPT'] = '0'
    if ($start.PSObject.Properties.Name -contains "ArgumentList") {
        $start.ArgumentList.Add("--no-optional-locks")
        $start.ArgumentList.Add("-c")
        $start.ArgumentList.Add("safe.directory=$root")
        $start.ArgumentList.Add("-C")
        $start.ArgumentList.Add($root)
        $start.ArgumentList.Add("cat-file")
        $start.ArgumentList.Add("blob")
        $start.ArgumentList.Add("${Commit}:$path")
    }
    else {
        $escapedRoot = $root.Replace('"', '\"')
        $escapedObject = ("${Commit}:$path").Replace('"', '\"')
        $start.Arguments = "--no-optional-locks -c `"safe.directory=$escapedRoot`" -C `"$escapedRoot`" cat-file blob `"$escapedObject`""
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    $memory = [IO.MemoryStream]::new([byte[]]::new($MaximumBytes + 1), $true)
    $errors = [IO.MemoryStream]::new([byte[]]::new(65537), $true)
    $memory.SetLength(0)
    $errors.SetLength(0)
    $started = $false
    try {
        $started = $process.Start()
        if (-not $started) { throw 'Unable to start Git blob reader.' }
        $deadline = [Diagnostics.Stopwatch]::StartNew()
        $outputTask = $process.StandardOutput.BaseStream.CopyToAsync($memory)
        $errorTask = $process.StandardError.BaseStream.CopyToAsync($errors)
        if (-not [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($outputTask,$errorTask),30000) -or
            -not $process.WaitForExit([Math]::Max(0,30000 - [int]$deadline.ElapsedMilliseconds))) {
            throw 'Git publication read exceeded its deadline.'
        }
        if ($process.ExitCode -ne 0 -or $memory.Length -gt $MaximumBytes -or $errors.Length -gt 65536) {
            throw 'Git publication read failed or exceeded its byte bound.'
        }
        return ,$memory.ToArray()
    }
    catch {
        throw 'Git publication read failed within its bounded process contract.'
    }
    finally {
        if ($started -and -not $process.HasExited) {
            $process.Kill()
            $null = $process.WaitForExit(5000)
        }
        $memory.Dispose()
        $errors.Dispose()
        $process.Dispose()
    }
}

# Permit Git and working-file reads only from this direct canonical repository.
function Assert-CustodyCanonicalRepositoryRoot {
    param([Parameter(Mandatory)][string]$RepositoryRoot)
    $expected = 'E:\Source\Rust\MiniModel.org'
    $resolved = [IO.Path]::GetFullPath($RepositoryRoot).TrimEnd('\','/')
    $moduleRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd('\','/')
    if (-not [string]::Equals($resolved,$expected,[StringComparison]::OrdinalIgnoreCase) -or
        -not [string]::Equals($moduleRoot,$expected,[StringComparison]::OrdinalIgnoreCase) -or
        (Test-CustodyPathHasReparsePoint -Path $resolved)) {
        throw 'Publication input requires the canonical MiniModel repository.'
    }
    return $resolved
}

# Read bounded current bytes in place; callers independently compare the committed identity.
function Read-CustodyPublicationSourceBytes {
    param([Parameter(Mandatory)][string]$RepositoryRoot,[Parameter(Mandatory)][string]$RelativePath,[Parameter(Mandatory)][int]$MaximumBytes)
    $root = Assert-CustodyCanonicalRepositoryRoot -RepositoryRoot $RepositoryRoot
    $relative = Assert-CustodyRelativePath -Path $RelativePath
    $file = Join-Path $root ($relative -replace '/', '\')
    if ($MaximumBytes -lt 0 -or $MaximumBytes -gt 16777216 -or (Test-CustodyPathHasReparsePoint -Path $file)) {
        throw 'Publication source path or bound is invalid.'
    }
    $item = Get-Item -LiteralPath $file -Force
    if ($item.PSIsContainer -or $item.Length -gt $MaximumBytes) { throw 'Publication source is not a bounded regular file.' }
    $stream = [IO.File]::Open($file,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    try {
        if ($stream.Length -gt $MaximumBytes) { throw 'Publication source exceeded its read bound.' }
        $bytes = [byte[]]::new([int]$stream.Length)
        $offset = 0
        while ($offset -lt $bytes.Length) {
            $read = $stream.Read($bytes,$offset,$bytes.Length - $offset)
            if ($read -eq 0) { throw 'Publication source ended before its declared length.' }
            $offset += $read
        }
        if ($stream.ReadByte() -ne -1) { throw 'Publication source exceeded its read bound.' }
        return ,$bytes
    }
    finally { $stream.Dispose() }
}

# Bind one bounded publication closure in memory, with no source or control files written.
function New-CustodyPublicationInput {
    param(
        [Parameter(Mandatory)][string]$Phase,
        [Parameter(Mandatory)][string]$RepositoryRoot,
        [Parameter(Mandatory)]$Manifest,
        [Parameter(Mandatory)][string]$ManifestSha256,
        [Parameter(Mandatory)][string]$OperationId,
        [Parameter(Mandatory)]$Authority
    )
    Assert-CustodyMutationAllowed -Phase $Phase -MutationKind local
    $root = Assert-CustodyCanonicalRepositoryRoot -RepositoryRoot $RepositoryRoot
    if ($OperationId -cnotmatch '^[0-9a-f]{32}$' -or $ManifestSha256 -cnotmatch '^[0-9a-f]{64}$' -or
        [string]$Manifest.source_commit -cnotmatch '^[0-9a-f]{40}$' -or [string]$Manifest.source_tree -cnotmatch '^[0-9a-f]{40}$') {
        throw 'Publication operation or source identity is invalid.'
    }
    $files = @($Manifest.files)
    if ($files.Count -lt 1 -or $files.Count -gt 500) { throw 'Publication closure count is invalid.' }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $total = [int64]0
    foreach ($file in $files) {
        $relative = Assert-CustodyRelativePath -Path ([string]$file.path)
        $length = [int64]$file.bytes
        if (-not $seen.Add($relative) -or $file.bytes -ne $length -or $length -lt 0 -or $length -gt 16777216 -or
            [string]$file.sha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'Publication member identity is invalid.' }
        $total += $length
        if ($total -gt 67108864) { throw 'Publication closure exceeds the total byte budget.' }
    }
    $materializations = @{}
    foreach ($row in @($Manifest.materializations)) {
        Assert-CustodyExactKeys -Value $row -Expected @('path','kind','source_bytes','source_sha256') -Label 'deployment materialization'
        $relative = Assert-CustodyRelativePath -Path ([string]$row.path)
        if ($materializations.ContainsKey($relative) -or
            [string]$row.kind -cnotin @('prepared-release-to-published-v1','prepared-history-to-published-v1') -or
            [int64]$row.source_bytes -lt 1 -or [int64]$row.source_bytes -gt 16777216 -or
            [string]$row.source_sha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'Deployment materialization identity is invalid.' }
        $materializations[$relative]=$row
    }
    $payloads = [Collections.Generic.Dictionary[string,byte[]]]::new([StringComparer]::Ordinal)
    $identities = [Collections.Generic.List[object]]::new()
    $used = @{}
    foreach ($file in $files) {
        $relative = Assert-CustodyRelativePath -Path ([string]$file.path)
        $length = [int64]$file.bytes
        $materialization = $materializations[$relative]
        $sourceLength = if ($null -ne $materialization) { [int]$materialization.source_bytes } else { [int]$length }
        $sourceHash = if ($null -ne $materialization) { [string]$materialization.source_sha256 } else { [string]$file.sha256 }
        $bytes = Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit ([string]$Manifest.source_commit) -RelativePath $relative -MaximumBytes $sourceLength
        if ($bytes -isnot [byte[]] -or $bytes.Length -ne $sourceLength -or (Get-CustodyByteSha256 -Bytes $bytes) -cne $sourceHash) {
            throw 'Committed publication source identity drifted.'
        }
        $working = Read-CustodyPublicationSourceBytes -RepositoryRoot $root -RelativePath $relative -MaximumBytes $sourceLength
        if ($working.Length -ne $sourceLength -or (Get-CustodyByteSha256 -Bytes $working) -cne $sourceHash) {
            throw 'Canonical working source differs from the bound publication commit.'
        }
        if ($null -ne $materialization) {
            $bytes = [byte[]](ConvertTo-CustodyPublishedArtifactBytes -Kind ([string]$materialization.kind) -SourceBytes $bytes -Authority $Authority)
            $used[$relative]=$true
        }
        if ($bytes.Length -ne $length -or (Get-CustodyByteSha256 -Bytes $bytes) -cne [string]$file.sha256) { throw 'Materialized publication identity drifted.' }
        $payloads.Add($relative,[byte[]]$bytes.Clone())
        $identities.Add([pscustomobject][ordered]@{path=$relative;bytes=$length;sha256=[string]$file.sha256})
    }
    if ($used.Count -ne $materializations.Count) { throw 'Deployment manifest contains an unused materialization.' }
    $binding = [pscustomobject][ordered]@{schema='minimodel.publication_input.v1';operation_id=$OperationId;manifest_sha256=$ManifestSha256;source_commit=[string]$Manifest.source_commit;source_tree=[string]$Manifest.source_tree;files=$identities.ToArray()}
    $bindingBytes = [Text.UTF8Encoding]::new($false).GetBytes(($binding | ConvertTo-Json -Depth 8 -Compress))
    return [pscustomobject]@{Binding=$binding;BindingSha256=(Get-CustodyByteSha256 -Bytes $bindingBytes);Payloads=[Collections.ObjectModel.ReadOnlyDictionary[string,byte[]]]::new($payloads)}
}

# Recheck operation binding, exact member closure and every payload before its use.
function Assert-CustodyPublicationInput {
    param([Parameter(Mandatory)]$InputRecord,[Parameter(Mandatory)][string]$InputSha256,[Parameter(Mandatory)][string]$ManifestSha256,[Parameter(Mandatory)][string]$OperationId)
    Assert-CustodyExactKeys -Value $InputRecord -Expected @('Binding','BindingSha256','Payloads') -Label 'publication input'
    $binding=$InputRecord.Binding
    Assert-CustodyExactKeys -Value $binding -Expected @('schema','operation_id','manifest_sha256','source_commit','source_tree','files') -Label 'publication binding'
    if ($OperationId -cnotmatch '^[0-9a-f]{32}$' -or $ManifestSha256 -cnotmatch '^[0-9a-f]{64}$' -or
        [string]$binding.source_commit -cnotmatch '^[0-9a-f]{40}$' -or [string]$binding.source_tree -cnotmatch '^[0-9a-f]{40}$') {
        throw 'Publication input operation or source identity is invalid.'
    }
    $encoded=[Text.UTF8Encoding]::new($false).GetBytes(($binding | ConvertTo-Json -Depth 8 -Compress))
    if ($InputSha256 -cnotmatch '^[0-9a-f]{64}$' -or (Get-CustodyByteSha256 -Bytes $encoded) -cne $InputSha256 -or
        $InputRecord.BindingSha256 -cne $InputSha256 -or $binding.schema -cne 'minimodel.publication_input.v1' -or
        $binding.operation_id -cne $OperationId -or $binding.manifest_sha256 -cne $ManifestSha256 -or
        $InputRecord.Payloads -isnot [Collections.ObjectModel.ReadOnlyDictionary[string,byte[]]]) { throw 'Publication input binding drifted.' }
    $files=@($binding.files)
    if ($files.Count -lt 1 -or $files.Count -gt 500 -or $InputRecord.Payloads.Count -ne $files.Count) { throw 'Publication input closure drifted.' }
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $total=[int64]0
    foreach ($file in $files) {
        Assert-CustodyExactKeys -Value $file -Expected @('path','bytes','sha256') -Label 'publication member'
        $relative=Assert-CustodyRelativePath -Path ([string]$file.path)
        $bytes=$InputRecord.Payloads[$relative]
        if (-not $seen.Add($relative) -or $null -eq $bytes -or $bytes.Length -gt 16777216 -or $bytes.Length -ne [int64]$file.bytes -or
            (Get-CustodyByteSha256 -Bytes $bytes) -cne [string]$file.sha256) { throw 'Publication input bytes drifted.' }
        $total += $bytes.Length
        if ($total -gt 67108864) { throw 'Publication input exceeds the total byte budget.' }
    }
}

# Return an authenticated member from the bounded input without reopening mutable source.
function Get-CustodyPublicationFileBytes {
    param([Parameter(Mandatory)]$InputRecord,[Parameter(Mandatory)][string]$InputSha256,[Parameter(Mandatory)][string]$ManifestSha256,[Parameter(Mandatory)][string]$OperationId,[Parameter(Mandatory)][string]$RelativePath)
    Assert-CustodyPublicationInput -InputRecord $InputRecord -InputSha256 $InputSha256 -ManifestSha256 $ManifestSha256 -OperationId $OperationId
    $relative=Assert-CustodyRelativePath -Path $RelativePath
    if (-not $InputRecord.Payloads.ContainsKey($relative)) { throw 'Publication input member is absent.' }
    return ,$InputRecord.Payloads[$relative]
}

Export-ModuleMember -Function @(
    "Get-CustodyByteSha256",
    "Get-CustodyFileSha256",
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
    "Assert-CustodyRemotePath",
    "ConvertFrom-CustodyDirectoryListing",
    "Assert-CustodyBootstrapManifest",
    "Assert-CustodyRemoteDirectoryTree",
    "New-CustodyLeaseOwnerBytes",
    "Assert-CustodyLeaseOwnership",
    "Get-CustodyGitBlobBytes",
    "New-CustodyPublicationInput",
    "Assert-CustodyPublicationInput",
    "Get-CustodyPublicationFileBytes"
)
