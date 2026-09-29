param(
    [string]$AuthorityPath = (Join-Path (Split-Path -Parent $PSScriptRoot) "deploy\publication-authority.v1.json"),
    [string]$ManifestPath = (Join-Path (Split-Path -Parent $PSScriptRoot) "deploy\site-deployment-manifest.v1.json"),
    [string]$FtpExportPath = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "ftp.txt"),
    [string]$ReportPath = (Join-Path (Split-Path -Parent $PSScriptRoot) ("deploy\receipts\site-deployment-{0}.json" -f (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ"))),
    [string]$NodeExecutablePath = (Join-Path $env:ProgramFiles 'nodejs\node.exe'),
    [switch]$Publish
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Net.Http

$repositoryRoot = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$modulePath = Join-Path $PSScriptRoot "MiniModel.DeploymentCustody.psm1"

function Test-BootstrapPathHasReparsePoint {
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

# Execute only the reviewed gate or manifest tool with pinned, read-leased files and bounded pipes.
function Invoke-VerifiedNode {
    param(
        [Parameter(Mandatory)][ValidateSet('source-authority','manifest')][string]$Tool,
        [AllowEmptyCollection()][string[]]$Arguments = @(),
        [ValidateRange(1,120000)][int]$TimeoutMilliseconds = 120000
    )

    if ($Tool -ceq 'source-authority') {
        if (@($Arguments | Where-Object { $_ -cnotin @('--require-clean','--require-remote-exact') }).Count -gt 0 -or
            @($Arguments | Select-Object -Unique).Count -ne $Arguments.Count) { throw 'Invalid source-authority arguments.' }
        $toolPath = Join-Path (Split-Path -Parent $repositoryRoot) 'TinyRustLM.com\tinyrustlm\tools\verify-single-source-authority.js'
        $toolBytes = 144521
        $toolSha256 = 'e8867b01d00399f319852449ff80c249edab30a8bcee4c09fd3789300f65aaf7'
        $maximumOutput = 4096
    }
    else {
        if ($Arguments.Count -lt 1 -or $Arguments.Count -gt 2 -or $Arguments[0].Length -ne 40 -or $Arguments[0] -cnotmatch '^[0-9a-f]{40}$' -or
            ($Arguments.Count -eq 2 -and $Arguments[1] -cne '--verify')) { throw 'Invalid manifest arguments.' }
        $toolPath = Join-Path $repositoryRoot 'tools\build-site-deployment-manifest.js'
        $toolBytes = 17615
        $toolSha256 = 'bd9276bc4a1296db550b7fb6511a6d2ba257b481b08d212c1f85724a3015e305'
        $maximumOutput = 2097152
    }
    if (-not [IO.Path]::IsPathRooted($NodeExecutablePath)) { throw 'Node executable path must be absolute.' }
    $nodePath = [IO.Path]::GetFullPath($NodeExecutablePath)
    $leases = [Collections.Generic.List[IDisposable]]::new()
    $process = [Diagnostics.Process]::new()
    $output = [IO.MemoryStream]::new([byte[]]::new($maximumOutput + 1),$true)
    $errors = [IO.MemoryStream]::new([byte[]]::new(65537),$true)
    $output.SetLength(0); $errors.SetLength(0)
    $started = $false
    try {
        foreach ($identity in @(
            @{Path=$nodePath;Bytes=91380224;Sha256='63c259c81e5d472b5f11c8d506070130cb04a1ecf84b80377a34ed6ec9048088'},
            @{Path=$toolPath;Bytes=$toolBytes;Sha256=$toolSha256}
        )) {
            if (Test-BootstrapPathHasReparsePoint -Path $identity.Path) { throw 'Verified Node input contains a reparse point.' }
            $file = Get-Item -LiteralPath $identity.Path -Force
            if ($file -isnot [IO.FileInfo] -or $file.Length -ne $identity.Bytes) { throw 'Verified Node file identity drifted.' }
            $lease = [IO.File]::Open($file.FullName,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
            $leases.Add($lease)
            $sha = [Security.Cryptography.SHA256]::Create()
            try { $actual=([BitConverter]::ToString($sha.ComputeHash($lease))).Replace('-','').ToLowerInvariant() }
            finally { $sha.Dispose() }
            if ($actual -cne $identity.Sha256) { throw 'Verified Node file identity drifted.' }
        }
        $start=[Diagnostics.ProcessStartInfo]::new()
        $start.FileName=$nodePath
        $start.WorkingDirectory=$repositoryRoot
        $start.UseShellExecute=$false
        $start.CreateNoWindow=$true
        $start.RedirectStandardOutput=$true
        $start.RedirectStandardError=$true
        # Arguments are fixed canonical paths, exact object IDs and admitted literal flags, never shell code.
        $start.Arguments=(@($toolPath)+@($Arguments) | ForEach-Object { '"'+$_+'"' }) -join ' '
        foreach ($name in @($start.EnvironmentVariables.Keys)) {
            if ([string]$name -like 'NODE_*') { $start.EnvironmentVariables.Remove([string]$name) }
        }
        $process.StartInfo=$start
        $started=$process.Start()
        if (-not $started) { throw 'Unable to start verified Node.' }
        $watch=[Diagnostics.Stopwatch]::StartNew()
        $stdoutTask=$process.StandardOutput.BaseStream.CopyToAsync($output)
        $stderrTask=$process.StandardError.BaseStream.CopyToAsync($errors)
        if (-not [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdoutTask,$stderrTask),$TimeoutMilliseconds) -or
            -not $process.WaitForExit([Math]::Max(0,$TimeoutMilliseconds-[int]$watch.ElapsedMilliseconds))) {
            throw 'Verified Node exceeded its deadline.'
        }
        if ($output.Length -gt $maximumOutput -or $errors.Length -gt 65536) { throw 'Verified Node exceeded its output budget.' }
        $utf8=[Text.UTF8Encoding]::new($false,$true)
        return [pscustomobject]@{ExitCode=$process.ExitCode;Stdout=$utf8.GetString($output.ToArray());Stderr=$utf8.GetString($errors.ToArray())}
    }
    finally {
        try {
            if ($started -and -not $process.HasExited) {
                # Native stderr can race with successful process exit. Always inspect the owned handle afterward.
                $nativePreference = $ErrorActionPreference
                try {
                    $ErrorActionPreference = 'Continue'
                    & (Join-Path $env:SystemRoot 'System32\taskkill.exe') /PID $process.Id /T /F *> $null
                }
                finally { $ErrorActionPreference = $nativePreference }
                if (-not $process.WaitForExit(5000)) { throw 'Verified Node child did not terminate.' }
            }
        }
        finally {
            $output.Dispose(); $errors.Dispose(); $process.Dispose()
            foreach ($lease in $leases) { $lease.Dispose() }
        }
    }
}

function Assert-EcosystemSingleSourceAuthority {
    $gitRootLines = @(& git --no-optional-locks -C $repositoryRoot rev-parse --show-toplevel 2>&1)
    if ($LASTEXITCODE -ne 0 -or $gitRootLines.Count -ne 1) {
        throw "Unable to derive the MiniModel repository root from Git."
    }
    $gitRoot = [IO.Path]::GetFullPath(([string]$gitRootLines[0]).Trim()).TrimEnd(
        [IO.Path]::DirectorySeparatorChar,
        [IO.Path]::AltDirectorySeparatorChar
    )
    $scriptRoot = $repositoryRoot.TrimEnd(
        [IO.Path]::DirectorySeparatorChar,
        [IO.Path]::AltDirectorySeparatorChar
    )
    if (-not [string]::Equals($gitRoot, $scriptRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "The deploy script parent does not equal the Git top-level."
    }

    $rustRoot = [IO.Path]::GetFullPath((Split-Path -Parent $gitRoot))
    $authorityRoot = [IO.Path]::GetFullPath((Join-Path $rustRoot "TinyRustLM.com\tinyrustlm"))
    $authorityGate = [IO.Path]::GetFullPath((Join-Path $authorityRoot "tools\verify-single-source-authority.js"))
    if (-not (Test-Path -LiteralPath $authorityGate -PathType Leaf) -or
        (Test-BootstrapPathHasReparsePoint -Path $authorityRoot) -or
        (Test-BootstrapPathHasReparsePoint -Path $authorityGate)) {
        throw "The canonical ecosystem source-authority gate is unavailable or unsafe."
    }

    $gateArguments = @()
    if ($Publish) {
        $gateArguments += @("--require-clean", "--require-remote-exact")
    }
    $gateResult = Invoke-VerifiedNode -Tool source-authority -Arguments $gateArguments
    $gateOutput = @($gateResult.Stdout.TrimEnd("`r","`n") -split "\r?\n")
    if ($gateResult.ExitCode -ne 0 -or $gateResult.Stderr.Length -ne 0 -or $gateOutput.Count -ne 1) {
        throw "The ecosystem source-authority gate failed closed."
    }
    try {
        $verdict = ([string]$gateOutput[0]) | ConvertFrom-Json
    }
    catch {
        throw "The ecosystem source-authority gate returned an invalid verdict."
    }
    if ($verdict.verdict -cne "pass" -or
        -not [bool]$verdict.canonicalLayoutAvailable -or
        -not [bool]$verdict.localScanPerformed -or
        $verdict.coverageClass -cne "registered-governed-roots-only") {
        throw "The ecosystem source-authority gate did not prove the fixed governed roots."
    }
}

Assert-EcosystemSingleSourceAuthority
$script:sourceAuthorityAdmitted = $true
Import-Module -Name $modulePath -Force

$script:phase = "read-only"
$script:credential = $null
$script:ftpHost = $null
$script:ftpPort = 0
$script:remoteRoot = $null
$script:publicationInput = $null
$script:publicationInputSha256 = $null
$script:publicationOperationId = $null
$script:lease = $null
$script:transactionJournal = [Collections.Generic.List[object]]::new()
$script:commitCleanupStarted = $false

function Assert-RelativePath {
    param([Parameter(Mandatory)][string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path) -or
        $Path.Length -gt 512 -or
        $Path.StartsWith("/") -or
        $Path.StartsWith("\") -or
        $Path.EndsWith("/") -or
        $Path.Contains("\") -or
        $Path -match "(^|/)\.\.?(/|$)" -or
        $Path -notmatch "^[A-Za-z0-9._/-]+$" -or
        $Path -match "//") {
        throw "Deployment manifest contains an unsafe relative path."
    }
    return $Path
}

function Get-EncodedRemotePath {
    param([string]$LogicalPath)

    $targetPath = if ($script:remoteRoot -ceq ".") {
        $LogicalPath
    }
    elseif ([string]::IsNullOrEmpty($LogicalPath)) {
        $script:remoteRoot
    }
    else {
        "$($script:remoteRoot)/$LogicalPath"
    }
    if ([string]::IsNullOrEmpty($targetPath)) { return "" }
    return (($targetPath -split "/") | ForEach-Object { [Uri]::EscapeDataString($_) }) -join "/"
}

function Test-CanonicalPathHasReparsePoint {
    param([Parameter(Mandatory)][string]$Path)

    $current = Get-Item -LiteralPath ([IO.Path]::GetFullPath($Path))
    while ($null -ne $current) {
        if (($current.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { return $true }
        $parent = $current.Parent
        if ($null -eq $parent -or $parent.FullName -ceq $current.FullName) { break }
        $current = $parent
    }
    return $false
}

function Invoke-GitText {
    param([Parameter(Mandatory)][string[]]$Arguments)

    $output = @(& git --no-optional-locks -C $repositoryRoot @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "Git read-only identity command failed."
    }
    return @($output | ForEach-Object { [string]$_ })
}

function Test-TrackedPathMatchesHead {
    param([Parameter(Mandatory)][string]$RelativePath)

    $tracked = @(& git --no-optional-locks -C $repositoryRoot ls-files -- $RelativePath 2>$null)
    if ($LASTEXITCODE -ne 0 -or $tracked.Count -ne 1 -or ([string]$tracked[0]).Trim() -cne $RelativePath) { return $false }
    $headBytes = Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit "HEAD" -RelativePath $RelativePath
    $workingPath = [IO.Path]::GetFullPath((Join-Path $repositoryRoot ($RelativePath -replace "/", "\")))
    if (-not (Test-Path -LiteralPath $workingPath -PathType Leaf)) { return $false }
    $workingBytes = [IO.File]::ReadAllBytes($workingPath)
    return $workingBytes.Length -eq $headBytes.Length -and
        (Get-CustodyByteSha256 -Bytes $workingBytes) -ceq (Get-CustodyByteSha256 -Bytes $headBytes)
}

function Get-RepositoryObservation {
    param([Parameter(Mandatory)]$Authority)

    if (-not $script:sourceAuthorityAdmitted) {
        throw "Remote repository observation is forbidden before the ecosystem source-authority gate."
    }
    $topLevelLines = @(Invoke-GitText -Arguments @("rev-parse", "--show-toplevel"))
    $branchLines = @(Invoke-GitText -Arguments @("symbolic-ref", "--quiet", "--short", "HEAD"))
    $upstreamLines = @(Invoke-GitText -Arguments @("rev-parse", "--abbrev-ref", "@{upstream}"))
    $topLevel = ([string]$topLevelLines[0]).Trim()
    $branch = ([string]$branchLines[0]).Trim()
    $upstream = ([string]$upstreamLines[0]).Trim()
    $remotes = @(Invoke-GitText -Arguments @("remote") | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    $fetchUrlLines = @(Invoke-GitText -Arguments @("remote", "get-url", [string]$Authority.repository.remote_name))
    $pushUrlLines = @(Invoke-GitText -Arguments @("remote", "get-url", "--push", [string]$Authority.repository.remote_name))
    $headLines = @(Invoke-GitText -Arguments @("rev-parse", "HEAD"))
    $trackingHeadLines = @(Invoke-GitText -Arguments @("rev-parse", "refs/remotes/$($Authority.repository.remote_name)/$($Authority.repository.branch)"))
    $fetchUrl = ([string]$fetchUrlLines[0]).Trim()
    $pushUrl = ([string]$pushUrlLines[0]).Trim()
    $head = ([string]$headLines[0]).Trim()
    $trackingHead = ([string]$trackingHeadLines[0]).Trim()
    $remoteLines = @(Invoke-GitText -Arguments @(
        "-c", "credential.interactive=never", "ls-remote", "--exit-code", "--heads",
        [string]$Authority.repository.remote_name, "refs/heads/$($Authority.repository.branch)"
    ))
    if ($remoteLines.Count -ne 1 -or $remoteLines[0] -notmatch "^([0-9a-f]{40})\s+refs/heads/") {
        throw "Direct remote branch observation was not singular and exact."
    }
    $remoteHead = $Matches[1]
    $status = @(Invoke-GitText -Arguments @("status", "--porcelain", "--untracked-files=all"))

    & git --no-optional-locks -C $repositoryRoot merge-base --is-ancestor ([string]$Authority.candidate.source_commit) HEAD 2>$null
    $sourceIsAncestor = $LASTEXITCODE -eq 0
    $sourceTreeLines = @(Invoke-GitText -Arguments @("rev-parse", "$($Authority.candidate.source_commit)^{tree}"))
    $sourceTree = ([string]$sourceTreeLines[0]).Trim()
    return [pscustomobject]@{
        canonical_root = $topLevel
        root_has_reparse_point = Test-CanonicalPathHasReparsePoint -Path $topLevel
        branch = $branch
        upstream = $upstream
        remotes = $remotes
        fetch_url = $fetchUrl
        push_url = $pushUrl
        head = $head
        tracking_head = $trackingHead
        remote_head = $remoteHead
        status_count = $status.Count
        source_is_ancestor = $sourceIsAncestor
        source_tree = $sourceTree
        manifest_matches_head = Test-TrackedPathMatchesHead -RelativePath ([string]$Authority.candidate.manifest_path)
        authority_matches_head = Test-TrackedPathMatchesHead -RelativePath "deploy/publication-authority.v1.json"
    }
}

function Get-PublicBytes {
    param(
        [Parameter(Mandatory)][Net.Http.HttpClient]$Client,
        [Parameter(Mandatory)][string]$Origin,
        [Parameter(Mandatory)][string]$RelativePath,
        [Parameter(Mandatory)][string]$AuditNonce,
        [int64]$MaximumBytes = 2097152
    )

    if (-not $script:sourceAuthorityAdmitted) {
        throw "Public remote observation is forbidden before the ecosystem source-authority gate."
    }
    $relative = Assert-RelativePath -Path $RelativePath
    $request = [Net.Http.HttpRequestMessage]::new(
        [Net.Http.HttpMethod]::Get,
        ("{0}/{1}?publication_preflight={2}" -f $Origin, $relative, $AuditNonce)
    )
    $request.Headers.CacheControl = [Net.Http.Headers.CacheControlHeaderValue]::new()
    $request.Headers.CacheControl.NoCache = $true
    $request.Headers.CacheControl.NoStore = $true
    $request.Headers.Pragma.ParseAdd("no-cache")
    $request.Headers.AcceptEncoding.ParseAdd("identity")
    try {
        $response = $Client.SendAsync($request, [Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
        try {
            if ([int]$response.StatusCode -ne 200) { throw "Public predecessor read returned a non-200 status." }
            $contentLength = $response.Content.Headers.ContentLength
            if ($null -ne $contentLength -and [int64]$contentLength -gt $MaximumBytes) {
                throw "Public predecessor read exceeded its byte bound."
            }
            $bytes = $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
            if ($bytes.Length -lt 1 -or $bytes.Length -gt $MaximumBytes) {
                throw "Public predecessor read exceeded its byte bound."
            }
            return $bytes
        }
        finally {
            $response.Dispose()
        }
    }
    finally {
        $request.Dispose()
    }
}

# Selects only the admitted site's account from the sole shared bounded FTP-export reader.
function Get-CredentialProfile {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$ProfileName)

    if (-not $script:sourceAuthorityAdmitted) {
        throw "Credential access is forbidden before the ecosystem source-authority gate."
    }
    return Get-DeploymentCredentialProfile -ProfilePath $Path -SourceRoot $repositoryRoot -ExpectedProfileName $ProfileName
}

# Verifies a read-leased module stream against the reviewed shared reader; callers retain the lease.
function Assert-MiniModelCredentialModuleIdentity {
    param([Parameter(Mandatory)][IO.Stream]$Stream)

    if (-not $Stream.CanRead -or -not $Stream.CanSeek -or $Stream.Length -ne 6525) {
        throw "The canonical credential module identity drifted."
    }
    $Stream.Position = 0
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $actual = ([BitConverter]::ToString($sha.ComputeHash($Stream))).Replace('-', '').ToLowerInvariant()
        if ($actual -cne '26a07d33724c8bf7e9f02c3dc4e6fe8f812167c0b1db5d9d736c989da88e17a7') {
            throw "The canonical credential module identity drifted."
        }
    }
    finally { $sha.Dispose() }
}

# Imports the canonical sibling credential module only after source admission and exact byte verification.
function Import-MiniModelCredentialModule {
    if (-not $script:sourceAuthorityAdmitted) {
        throw "Credential module access is forbidden before the ecosystem source-authority gate."
    }
    $path = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $repositoryRoot) 'TinyRustLM.com\tinyrustlm\tools\Deployment.Credentials.psm1'))
    if (Test-BootstrapPathHasReparsePoint -Path $path) {
        throw "The canonical credential module ancestry is unsafe."
    }
    $lease = $null
    try {
        $lease = [IO.File]::Open($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        Assert-MiniModelCredentialModuleIdentity -Stream $lease
        Import-Module -Name $path -Global -Force -ErrorAction Stop
    }
    finally {
        if ($lease) { $lease.Dispose() }
    }
}

function New-FtpRequest {
    param(
        [string]$LogicalPath,
        [Parameter(Mandatory)][string]$Method,
        [switch]$Mutating
    )

    if (-not $script:sourceAuthorityAdmitted) {
        throw "FTP access is forbidden before the ecosystem source-authority gate."
    }
    if ($Mutating) {
        Assert-CustodyMutationAllowed -Phase $script:phase -MutationKind remote
    }
    $encoded = Get-EncodedRemotePath -LogicalPath $LogicalPath
    $uri = [Uri]("ftp://{0}:{1}/{2}" -f $script:ftpHost, $script:ftpPort, $encoded)
    $request = [Net.FtpWebRequest]::Create($uri)
    $request.Method = $Method
    $request.EnableSsl = $true
    $request.UseBinary = $true
    $request.UsePassive = $true
    $request.KeepAlive = $false
    $request.Timeout = 300000
    $request.ReadWriteTimeout = 300000
    $request.Credentials = $script:credential
    return $request
}

function Test-Ftp550Error {
    param([Exception]$Exception)

    $current = $Exception
    while ($current.InnerException) { $current = $current.InnerException }
    return $current -is [Net.WebException] -and
        $current.Response -is [Net.FtpWebResponse] -and
        [int]$current.Response.StatusCode -eq 550
}

function Get-RemoteSize {
    param([Parameter(Mandatory)][string]$LogicalPath)

    try {
        $request = New-FtpRequest -LogicalPath $LogicalPath -Method ([Net.WebRequestMethods+Ftp]::GetFileSize)
        $response = $request.GetResponse()
        try { return [int64]$response.ContentLength } finally { $response.Dispose() }
    }
    catch {
        if (Test-Ftp550Error -Exception $_.Exception) {
            throw [InvalidOperationException]::new("Remote SIZE returned FTP 550; accessibility and existence are ambiguous.", $_.Exception)
        }
        throw
    }
}

function Get-RemoteBytes {
    param(
        [Parameter(Mandatory)][string]$LogicalPath,
        [Parameter(Mandatory)][int64]$MaximumBytes
    )

    if ($MaximumBytes -lt 1 -or $MaximumBytes -gt 16777216) { throw "Invalid remote download byte bound." }
    $request = New-FtpRequest -LogicalPath $LogicalPath -Method ([Net.WebRequestMethods+Ftp]::DownloadFile)
    $response = $request.GetResponse()
    $stream = $response.GetResponseStream()
    $memory = [IO.MemoryStream]::new()
    $buffer = New-Object byte[] 65536
    try {
        while ($true) {
            $read = $stream.Read($buffer, 0, $buffer.Length)
            if ($read -eq 0) { break }
            if ($memory.Length + $read -gt $MaximumBytes) { throw "Remote download exceeded its exact byte bound." }
            $memory.Write($buffer, 0, $read)
        }
        return $memory.ToArray()
    }
    finally {
        $memory.Dispose()
        $stream.Dispose()
        $response.Dispose()
    }
}

function Get-RemoteDirectoryEntries {
    param([string]$LogicalPath)

    $request = New-FtpRequest -LogicalPath $LogicalPath -Method ([Net.WebRequestMethods+Ftp]::ListDirectory)
    $response = $request.GetResponse()
    $stream = $response.GetResponseStream()
    $reader = [IO.StreamReader]::new($stream, [Text.UTF8Encoding]::new($false, $true), $true, 4096, $true)
    try {
        $entries = @()
        while (-not $reader.EndOfStream) {
            $entry = $reader.ReadLine()
            if ([string]::IsNullOrEmpty($entry)) { continue }
            if ($entry.Length -gt 512 -or $entry -in @(".", "..") -or $entry.Contains("/") -or $entry.Contains("\") -or
                $entry -notmatch "^[A-Za-z0-9._-]+$") {
                throw "Remote directory listing contains an unsafe entry."
            }
            if ($entries -ccontains $entry) { throw "Remote directory listing contains a duplicate entry." }
            $entries += $entry
            if ($entries.Count -gt 500) { throw "Remote directory listing exceeds its entry bound." }
        }
        return $entries
    }
    finally {
        $reader.Dispose()
        $stream.Dispose()
        $response.Dispose()
    }
}

function Get-RemoteParentAndLeaf {
    param([Parameter(Mandatory)][string]$LogicalPath)

    $relative = Assert-RelativePath -Path $LogicalPath
    $lastSlash = $relative.LastIndexOf("/")
    return [pscustomobject]@{
        Parent = if ($lastSlash -ge 0) { $relative.Substring(0, $lastSlash) } else { "" }
        Leaf = if ($lastSlash -ge 0) { $relative.Substring($lastSlash + 1) } else { $relative }
    }
}

function Test-ExactListedLeaf {
    param([Parameter(Mandatory)][string[]]$Entries, [Parameter(Mandatory)][string]$Leaf)

    $exact = @($Entries | Where-Object { [string]$_ -ceq $Leaf })
    $aliases = @($Entries | Where-Object {
        [string]::Equals([string]$_, $Leaf, [StringComparison]::OrdinalIgnoreCase) -and [string]$_ -cne $Leaf
    })
    if ($exact.Count -gt 1 -or $aliases.Count -gt 0) {
        throw "Remote directory listing contains an ambiguous path alias."
    }
    return $exact.Count -eq 1
}

function Get-RemoteFileObservation {
    param([Parameter(Mandatory)][string]$LogicalPath)

    $parts = Get-RemoteParentAndLeaf -LogicalPath $LogicalPath
    $before = @(Get-RemoteDirectoryEntries -LogicalPath $parts.Parent)
    $listedBefore = Test-ExactListedLeaf -Entries $before -Leaf $parts.Leaf
    if (-not $listedBefore) {
        $afterAbsent = @(Get-RemoteDirectoryEntries -LogicalPath $parts.Parent)
        $listedAfterAbsent = Test-ExactListedLeaf -Entries $afterAbsent -Leaf $parts.Leaf
        if ($listedAfterAbsent) { throw "Remote LIST observations changed while proving absence." }
        return Resolve-CustodyRemoteFileObservation -Listed $false -SizeStatus "not-probed"
    }

    $size = Get-RemoteSize -LogicalPath $LogicalPath
    $after = @(Get-RemoteDirectoryEntries -LogicalPath $parts.Parent)
    if (-not (Test-ExactListedLeaf -Entries $after -Leaf $parts.Leaf)) {
        throw "Remote SIZE and LIST observations contradict each other."
    }
    return Resolve-CustodyRemoteFileObservation -Listed $true -SizeStatus "success" -Size $size
}

function Test-RemoteIdentity {
    param([string]$LogicalPath, [int64]$Bytes, [string]$Sha256)

    $observation = Get-RemoteFileObservation -LogicalPath $LogicalPath
    if ($observation.State -ceq "absent" -or [int64]$observation.Size -ne $Bytes) { return $false }
    $remoteBytes = Get-RemoteBytes -LogicalPath $LogicalPath -MaximumBytes ([Math]::Max([int64]1, $Bytes))
    return $remoteBytes.Length -eq $Bytes -and (Get-CustodyByteSha256 -Bytes $remoteBytes) -ceq $Sha256
}

function Get-ObservedRemoteTree {
    param([Parameter(Mandatory)][string[]]$ExpectedFilePaths)

    $expectedTree = Get-CustodyExpectedDirectoryTree -FilePaths $ExpectedFilePaths
    $observed = @{}
    foreach ($directory in @($expectedTree.Keys | Sort-Object -CaseSensitive)) {
        $observed[[string]$directory] = @(Get-RemoteDirectoryEntries -LogicalPath ([string]$directory))
    }
    return $observed
}

function Test-RemoteDirectory {
    param([string]$LogicalPath)

    if ([string]::IsNullOrEmpty($LogicalPath)) {
        Get-RemoteDirectoryEntries -LogicalPath $LogicalPath | Out-Null
        return $true
    }
    $parts = Get-RemoteParentAndLeaf -LogicalPath $LogicalPath
    $parentEntries = @(Get-RemoteDirectoryEntries -LogicalPath $parts.Parent)
    if (-not (Test-ExactListedLeaf -Entries $parentEntries -Leaf $parts.Leaf)) { return $false }
    Get-RemoteDirectoryEntries -LogicalPath $LogicalPath | Out-Null
    return $true
}

function Remove-RemoteFileIfPresent {
    param([string]$LogicalPath)

    $observation = Get-RemoteFileObservation -LogicalPath $LogicalPath
    if ($observation.State -ceq "absent") { return $false }
    $request = New-FtpRequest -LogicalPath $LogicalPath -Method ([Net.WebRequestMethods+Ftp]::DeleteFile) -Mutating
    $response = $request.GetResponse()
    $response.Dispose()
    if ((Get-RemoteFileObservation -LogicalPath $LogicalPath).State -cne "absent") {
        throw "Remote file deletion did not commit."
    }
    return $true
}

function Rename-RemoteFile {
    param([string]$LogicalPath, [string]$NewLeafName)

    if ($NewLeafName -notmatch "^[A-Za-z0-9._-]+$") { throw "Unsafe remote rename leaf." }
    $request = New-FtpRequest -LogicalPath $LogicalPath -Method ([Net.WebRequestMethods+Ftp]::Rename) -Mutating
    $request.RenameTo = $NewLeafName
    $response = $request.GetResponse()
    $response.Dispose()
}

function Write-RemoteChunk {
    param([string]$LogicalPath, [byte[]]$Bytes, [bool]$Append)

    $method = if ($Append) { [Net.WebRequestMethods+Ftp]::AppendFile } else { [Net.WebRequestMethods+Ftp]::UploadFile }
    $request = New-FtpRequest -LogicalPath $LogicalPath -Method $method -Mutating
    $request.ContentLength = $Bytes.Length
    $output = $null
    try {
        $output = $request.GetRequestStream()
        $output.Write($Bytes, 0, $Bytes.Length)
        $output.Flush()
        $output.Dispose()
        $output = $null
        $response = $request.GetResponse()
        $response.Dispose()
    }
    finally {
        if ($output) { try { $output.Dispose() } catch { } }
    }
}

function Get-PublicationPrefixSha256 {
    param([Parameter(Mandatory)][AllowEmptyCollection()][byte[]]$Payload, [Parameter(Mandatory)][int64]$Bytes)
    if ($Bytes -lt 0 -or $Bytes -gt $Payload.Length) { throw "Invalid publication prefix length." }
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($Payload,0,[int]$Bytes))).Replace("-", "").ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function Send-RemoteFile {
    param([Parameter(Mandatory)][AllowEmptyCollection()][byte[]]$Payload, [Parameter(Mandatory)][string]$LogicalPath)
    $expectedBytes = $Payload.Length
    if ($expectedBytes -gt 16777216) { throw "Publication member exceeds its byte budget." }
    if ((Get-RemoteFileObservation -LogicalPath $LogicalPath).State -cne "absent") {
        throw "Operation-owned remote staging path was not exactly absent."
    }
    $offset = 0
    if ($expectedBytes -eq 0) {
        Write-RemoteChunk -LogicalPath $LogicalPath -Bytes ([byte[]]::new(0)) -Append $false
        $observation = Get-RemoteFileObservation -LogicalPath $LogicalPath
        if ($observation.State -cne "present" -or [int64]$observation.Size -ne 0) {
            throw "Remote staging did not commit the empty file exactly."
        }
        return
    }
    while ($offset -lt $expectedBytes) {
        $count = [int][Math]::Min(8192, $expectedBytes - $offset)
        $chunk = [byte[]]::new($count)
        [Array]::Copy($Payload,$offset,$chunk,0,$count)
        $intendedSize = $offset + $count
        Write-RemoteChunk -LogicalPath $LogicalPath -Bytes $chunk -Append ($offset -gt 0)
        $observation = Get-RemoteFileObservation -LogicalPath $LogicalPath
        if ($observation.State -cne "present" -or [int64]$observation.Size -ne $intendedSize) {
            throw "Remote staging offset drifted from the intended chunk boundary."
        }
        $offset = $intendedSize
    }
}

function Get-SafeFailureCode {
    param([Exception]$Exception)

    $current = $Exception
    while ($current.InnerException) { $current = $current.InnerException }
    $code = $current.GetType().FullName
    if ($current -is [Net.WebException]) {
        $code += "/" + [string]$current.Status
        if ($current.Response -is [Net.FtpWebResponse]) {
            $code += "/ftp-" + [int]$current.Response.StatusCode
        }
    }
    return $code
}

function Get-RemoteOperationPath {
    param([Parameter(Mandatory)][string]$Relative, [Parameter(Mandatory)][string]$Nonce, [Parameter(Mandatory)][string]$Kind)

    if ($Nonce -notmatch "^[0-9a-f]{32}$" -or $Kind -notin @("deploy", "backup")) {
        throw "Remote operation artifact identity is invalid."
    }
    $lastSlash = $Relative.LastIndexOf("/")
    $parent = if ($lastSlash -ge 0) { $Relative.Substring(0, $lastSlash + 1) } else { "" }
    $leaf = if ($lastSlash -ge 0) { $Relative.Substring($lastSlash + 1) } else { $Relative }
    $extension = [IO.Path]::GetExtension($leaf)
    $stem = $leaf.Substring(0, $leaf.Length - $extension.Length)
    return "$parent$stem.$Kind-$Nonce$extension"
}

function Get-RemoteBoundedIdentity {
    param([Parameter(Mandatory)][string]$LogicalPath, [Parameter(Mandatory)][int64]$MaximumBytes)

    $observation = Get-RemoteFileObservation -LogicalPath $LogicalPath
    if ($observation.State -ceq "absent") {
        return [pscustomobject]@{ State = "absent"; Bytes = $null; Sha256 = $null }
    }
    if ([int64]$observation.Size -gt $MaximumBytes) {
        throw "Remote file exceeds the bounded transaction identity."
    }
    $bytes = Get-RemoteBytes -LogicalPath $LogicalPath -MaximumBytes ([Math]::Max([int64]1, $MaximumBytes))
    if ($bytes.Length -ne [int64]$observation.Size) {
        throw "Remote SIZE and download observations contradict each other."
    }
    return [pscustomobject]@{
        State = "present"
        Bytes = [int64]$bytes.Length
        Sha256 = Get-CustodyByteSha256 -Bytes $bytes
    }
}

function Assert-RemoteExactIdentity {
    param([Parameter(Mandatory)][string]$LogicalPath, [Parameter(Mandatory)][int64]$Bytes, [Parameter(Mandatory)][string]$Sha256)

    $identity = Get-RemoteBoundedIdentity -LogicalPath $LogicalPath -MaximumBytes ([Math]::Max([int64]1, $Bytes))
    if ($identity.State -cne "present" -or [int64]$identity.Bytes -ne $Bytes -or $identity.Sha256 -cne $Sha256) {
        throw "Remote transaction identity mismatch: $LogicalPath"
    }
}

function Remove-RemoteExactFile {
    param([Parameter(Mandatory)][string]$LogicalPath, [Parameter(Mandatory)][int64]$Bytes, [Parameter(Mandatory)][string]$Sha256)

    $identity = Get-RemoteBoundedIdentity -LogicalPath $LogicalPath -MaximumBytes ([Math]::Max([int64]1, $Bytes))
    if ($identity.State -ceq "absent") { return $false }
    if ([int64]$identity.Bytes -ne $Bytes -or $identity.Sha256 -cne $Sha256) {
        throw "Refusing to remove a remote file with substituted bytes: $LogicalPath"
    }
    Remove-RemoteFileIfPresent -LogicalPath $LogicalPath | Out-Null
    return $true
}

function Assert-RemoteLeaseOwned {
    if ($null -eq $script:lease) { throw "The remote deployment lease is not held." }
    $entries = @(Get-RemoteDirectoryEntries -LogicalPath ([string]$script:lease.DirectoryPath))
    if ($entries.Count -ne 1 -or [string]$entries[0] -cne [string]$script:lease.OwnerLeaf) {
        throw "Remote deployment lease closure or ownership changed."
    }
    Assert-RemoteExactIdentity `
        -LogicalPath ([string]$script:lease.OwnerPath) `
        -Bytes ([int64]$script:lease.OwnerBytes.Length) `
        -Sha256 ([string]$script:lease.OwnerSha256)
    $ownerBytes = Get-RemoteBytes `
        -LogicalPath ([string]$script:lease.OwnerPath) `
        -MaximumBytes ([int64]$script:lease.OwnerBytes.Length)
    Assert-CustodyLeaseOwnership `
        -ExpectedOwnerBytes $script:lease.OwnerBytes `
        -ObservedOwnerBytes $ownerBytes `
        -OperationId ([string]$script:lease.OperationId)
}

function New-RemoteDeploymentLease {
    param(
        [Parameter(Mandatory)]$Authority,
        [Parameter(Mandatory)][string]$OperationId,
        [Parameter(Mandatory)][string]$PublisherCommit,
        [Parameter(Mandatory)][string]$ManifestSha256,
        [Parameter(Mandatory)][string]$AuthoritySha256
    )

    $directoryPath = [string]$Authority.transport.lease_directory_path
    if (Test-RemoteDirectory -LogicalPath $directoryPath) {
        throw "A remote deployment lease or crash residue already exists."
    }
    $request = New-FtpRequest -LogicalPath $directoryPath -Method ([Net.WebRequestMethods+Ftp]::MakeDirectory) -Mutating
    $response = $request.GetResponse()
    $response.Dispose()
    if (-not (Test-RemoteDirectory -LogicalPath $directoryPath)) {
        throw "Remote deployment lease creation did not commit exactly."
    }

    $ownerLeaf = "owner-$OperationId.json"
    $ownerPath = "$directoryPath/$ownerLeaf"
    $ownerBytes = New-CustodyLeaseOwnerBytes `
        -Authority $Authority `
        -OperationId $OperationId `
        -PublisherCommit $PublisherCommit `
        -ManifestSha256 $ManifestSha256 `
        -AuthoritySha256 $AuthoritySha256
    if ((Get-RemoteFileObservation -LogicalPath $ownerPath).State -cne "absent") {
        throw "New remote deployment lease owner path was occupied."
    }
    Write-RemoteChunk -LogicalPath $ownerPath -Bytes $ownerBytes -Append $false
    $lease = [pscustomobject]@{
        DirectoryPath = $directoryPath
        OwnerLeaf = $ownerLeaf
        OwnerPath = $ownerPath
        OwnerBytes = $ownerBytes
        OwnerSha256 = Get-CustodyByteSha256 -Bytes $ownerBytes
        OperationId = $OperationId
    }
    $script:lease = $lease
    Assert-RemoteLeaseOwned
    return $lease
}

function Remove-RemoteDeploymentLease {
    Assert-RemoteLeaseOwned
    Remove-RemoteExactFile `
        -LogicalPath ([string]$script:lease.OwnerPath) `
        -Bytes ([int64]$script:lease.OwnerBytes.Length) `
        -Sha256 ([string]$script:lease.OwnerSha256) | Out-Null
    if (@(Get-RemoteDirectoryEntries -LogicalPath ([string]$script:lease.DirectoryPath)).Count -ne 0) {
        throw "Refusing to remove a nonempty remote deployment lease."
    }
    $request = New-FtpRequest `
        -LogicalPath ([string]$script:lease.DirectoryPath) `
        -Method ([Net.WebRequestMethods+Ftp]::RemoveDirectory) `
        -Mutating
    $response = $request.GetResponse()
    $response.Dispose()
    if (Test-RemoteDirectory -LogicalPath ([string]$script:lease.DirectoryPath)) {
        throw "Remote deployment lease removal did not commit."
    }
    $script:lease = $null
}

function Assert-RemoteParentDirectoriesExist {
    param([Parameter(Mandatory)][string]$LogicalPath)

    $parts = $LogicalPath -split "/"
    $current = @()
    for ($index = 0; $index -lt $parts.Count - 1; $index++) {
        $current += $parts[$index]
        if (-not (Test-RemoteDirectory -LogicalPath ($current -join "/"))) {
            throw "Candidate publication requires a directory outside the exact predecessor closure."
        }
    }
}

function Publish-File {
    param(
        [Parameter(Mandatory)]$File,
        $PredecessorFile,
        [Parameter(Mandatory)][string]$Nonce
    )

    Assert-RemoteLeaseOwned
    $relative = [string]$File.path
    $payload = Get-CustodyPublicationFileBytes `
        -InputRecord $script:publicationInput -InputSha256 $script:publicationInputSha256 `
        -ManifestSha256 $script:manifestSha256 -OperationId $script:publicationOperationId `
        -RelativePath $relative
    if ($payload.Length -ne [int64]$File.bytes -or
        (Get-CustodyByteSha256 -Bytes $payload) -cne [string]$File.sha256) {
        throw "Publication input changed before upload."
    }
    $target = Get-RemoteBoundedIdentity `
        -LogicalPath $relative `
        -MaximumBytes ([Math]::Max([int64]1, [Math]::Max([int64]$File.bytes, $(if ($null -ne $PredecessorFile) { [int64]$PredecessorFile.bytes } else { [int64]0 }))))
    if ($null -eq $PredecessorFile) {
        if ($target.State -cne "absent") { throw "Candidate target was not absent in the exact predecessor closure: $relative" }
    }
    elseif ($target.State -cne "present" -or
        [int64]$target.Bytes -ne [int64]$PredecessorFile.bytes -or
        $target.Sha256 -cne [string]$PredecessorFile.sha256) {
        throw "Candidate target changed after exact predecessor proof: $relative"
    }
    if ($target.State -ceq "present" -and
        [int64]$target.Bytes -eq [int64]$File.bytes -and
        $target.Sha256 -ceq [string]$File.sha256) {
        return [ordered]@{ path = $relative; status = "already-current"; bytes = [int64]$File.bytes; sha256 = [string]$File.sha256 }
    }

    Assert-RemoteParentDirectoriesExist -LogicalPath $relative
    $stagePath = Get-RemoteOperationPath -Relative $relative -Nonce $Nonce -Kind "deploy"
    $backupPath = Get-RemoteOperationPath -Relative $relative -Nonce $Nonce -Kind "backup"
    if ((Get-RemoteFileObservation -LogicalPath $stagePath).State -cne "absent" -or
        (Get-RemoteFileObservation -LogicalPath $backupPath).State -cne "absent") {
        throw "Operation-owned remote staging or backup path was not absent."
    }

    $entry = [pscustomobject]@{
        Kind = "publish"
        Path = $relative
        StagePath = $stagePath
        BackupPath = $backupPath
        HadPredecessor = $null -ne $PredecessorFile
        PredecessorBytes = if ($null -ne $PredecessorFile) { [int64]$PredecessorFile.bytes } else { [int64]0 }
        PredecessorSha256 = if ($null -ne $PredecessorFile) { [string]$PredecessorFile.sha256 } else { $null }
        CandidateBytes = [int64]$File.bytes
        CandidateSha256 = [string]$File.sha256
        State = "planned"
    }
    $script:transactionJournal.Add($entry)
    Send-RemoteFile -Payload $payload -LogicalPath $stagePath
    Assert-RemoteExactIdentity -LogicalPath $stagePath -Bytes ([int64]$File.bytes) -Sha256 ([string]$File.sha256)
    $entry.State = "staged"
    if ($entry.HadPredecessor) {
        Rename-RemoteFile -LogicalPath $relative -NewLeafName (($backupPath -split "/")[-1])
        $entry.State = "backup-retained"
        Assert-RemoteExactIdentity -LogicalPath $backupPath -Bytes $entry.PredecessorBytes -Sha256 $entry.PredecessorSha256
        if ((Get-RemoteFileObservation -LogicalPath $relative).State -cne "absent") {
            throw "Original target remained after backup rename: $relative"
        }
    }
    Rename-RemoteFile -LogicalPath $stagePath -NewLeafName (($relative -split "/")[-1])
    $entry.State = "published"
    Assert-RemoteExactIdentity -LogicalPath $relative -Bytes ([int64]$File.bytes) -Sha256 ([string]$File.sha256)
    if ((Get-RemoteFileObservation -LogicalPath $stagePath).State -cne "absent") {
        throw "Remote staging path remained after publication rename: $relative"
    }
    return [ordered]@{ path = $relative; status = "published"; bytes = [int64]$File.bytes; sha256 = [string]$File.sha256 }
}

function Retire-RemoteFile {
    param([Parameter(Mandatory)]$PredecessorFile, [Parameter(Mandatory)][string]$Nonce)

    Assert-RemoteLeaseOwned
    $relative = [string]$PredecessorFile.path
    Assert-RemoteExactIdentity `
        -LogicalPath $relative `
        -Bytes ([int64]$PredecessorFile.bytes) `
        -Sha256 ([string]$PredecessorFile.sha256)
    $backupPath = Get-RemoteOperationPath -Relative $relative -Nonce $Nonce -Kind "backup"
    if ((Get-RemoteFileObservation -LogicalPath $backupPath).State -cne "absent") {
        throw "Operation-owned retirement backup path was not absent."
    }
    $entry = [pscustomobject]@{
        Kind = "retire"
        Path = $relative
        StagePath = $null
        BackupPath = $backupPath
        HadPredecessor = $true
        PredecessorBytes = [int64]$PredecessorFile.bytes
        PredecessorSha256 = [string]$PredecessorFile.sha256
        CandidateBytes = [int64]0
        CandidateSha256 = $null
        State = "planned"
    }
    $script:transactionJournal.Add($entry)
    Rename-RemoteFile -LogicalPath $relative -NewLeafName (($backupPath -split "/")[-1])
    $entry.State = "retired-backup-retained"
    Assert-RemoteExactIdentity -LogicalPath $backupPath -Bytes $entry.PredecessorBytes -Sha256 $entry.PredecessorSha256
    if ((Get-RemoteFileObservation -LogicalPath $relative).State -cne "absent") {
        throw "Retired path remained after its exact backup rename: $relative"
    }
    return [ordered]@{ path = $relative; status = "retired" }
}

function Test-RemoteIdentityValue {
    param($Identity, [int64]$Bytes, [string]$Sha256)

    return $Identity.State -ceq "present" -and [int64]$Identity.Bytes -eq $Bytes -and $Identity.Sha256 -ceq $Sha256
}

function Remove-RemoteCandidateStageForRollback {
    param([Parameter(Mandatory)]$Entry, [Parameter(Mandatory)]$Identity)

    if ($Identity.State -ceq "absent") { return }
    if ([int64]$Identity.Bytes -lt 0 -or [int64]$Identity.Bytes -gt [int64]$Entry.CandidateBytes) {
        throw "Exact rollback found an oversized staging artifact: $($Entry.StagePath)"
    }
    if (Test-RemoteIdentityValue `
        -Identity $Identity `
        -Bytes ([int64]$Entry.CandidateBytes) `
        -Sha256 ([string]$Entry.CandidateSha256)) {
        Remove-RemoteExactFile `
            -LogicalPath ([string]$Entry.StagePath) `
            -Bytes ([int64]$Entry.CandidateBytes) `
            -Sha256 ([string]$Entry.CandidateSha256) | Out-Null
        return
    }
    $payload = Get-CustodyPublicationFileBytes `
        -InputRecord $script:publicationInput -InputSha256 $script:publicationInputSha256 `
        -ManifestSha256 $script:manifestSha256 -OperationId $script:publicationOperationId `
        -RelativePath ([string]$Entry.Path)
    if ($payload.Length -ne [int64]$Entry.CandidateBytes -or
        (Get-CustodyByteSha256 -Bytes $payload) -cne [string]$Entry.CandidateSha256) {
        throw "Exact rollback cannot authenticate a partial stage from the publication input."
    }
    $prefixSha256 = Get-PublicationPrefixSha256 -Payload $payload -Bytes ([int64]$Identity.Bytes)
    if ($Identity.Sha256 -cne $prefixSha256) {
        throw "Exact rollback found substituted staging bytes: $($Entry.StagePath)"
    }
    Remove-RemoteExactFile `
        -LogicalPath ([string]$Entry.StagePath) `
        -Bytes ([int64]$Identity.Bytes) `
        -Sha256 ([string]$Identity.Sha256) | Out-Null
}

function Invoke-RemoteExactRollback {
    $script:phase = "rolling-back"
    $rollbackEntries = @(Get-CustodyBoundedRollbackOrder -Journal @($script:transactionJournal))
    foreach ($entry in $rollbackEntries) {
        Assert-RemoteLeaseOwned
        $maximumBytes = [Math]::Max([int64]1, [Math]::Max([int64]$entry.PredecessorBytes, [int64]$entry.CandidateBytes))
        $target = Get-RemoteBoundedIdentity -LogicalPath ([string]$entry.Path) -MaximumBytes $maximumBytes
        $backup = Get-RemoteBoundedIdentity -LogicalPath ([string]$entry.BackupPath) -MaximumBytes $maximumBytes
        $targetIsPredecessor = $entry.HadPredecessor -and
            (Test-RemoteIdentityValue -Identity $target -Bytes $entry.PredecessorBytes -Sha256 $entry.PredecessorSha256)
        $backupIsPredecessor = $entry.HadPredecessor -and
            (Test-RemoteIdentityValue -Identity $backup -Bytes $entry.PredecessorBytes -Sha256 $entry.PredecessorSha256)
        $targetIsCandidate = $entry.Kind -ceq "publish" -and
            (Test-RemoteIdentityValue -Identity $target -Bytes $entry.CandidateBytes -Sha256 $entry.CandidateSha256)

        if ($entry.HadPredecessor -and -not $targetIsPredecessor -and -not $backupIsPredecessor) {
            throw "Exact rollback has no authenticated predecessor copy: $($entry.Path)"
        }
        if ($target.State -ceq "present" -and -not $targetIsPredecessor -and -not $targetIsCandidate) {
            throw "Exact rollback found substituted target bytes: $($entry.Path)"
        }
        if ($backup.State -ceq "present" -and -not $backupIsPredecessor) {
            throw "Exact rollback found substituted backup bytes: $($entry.BackupPath)"
        }

        if ($targetIsCandidate) {
            Remove-RemoteExactFile `
                -LogicalPath ([string]$entry.Path) `
                -Bytes ([int64]$entry.CandidateBytes) `
                -Sha256 ([string]$entry.CandidateSha256) | Out-Null
            $targetIsPredecessor = $false
        }
        if ($entry.HadPredecessor) {
            if (-not $targetIsPredecessor) {
                if ((Get-RemoteFileObservation -LogicalPath ([string]$entry.Path)).State -cne "absent") {
                    throw "Exact rollback target was not absent before restore: $($entry.Path)"
                }
                Rename-RemoteFile `
                    -LogicalPath ([string]$entry.BackupPath) `
                    -NewLeafName (([string]$entry.Path -split "/")[-1])
                Assert-RemoteExactIdentity `
                    -LogicalPath ([string]$entry.Path) `
                    -Bytes ([int64]$entry.PredecessorBytes) `
                    -Sha256 ([string]$entry.PredecessorSha256)
            }
            elseif ($backupIsPredecessor) {
                Remove-RemoteExactFile `
                    -LogicalPath ([string]$entry.BackupPath) `
                    -Bytes ([int64]$entry.PredecessorBytes) `
                    -Sha256 ([string]$entry.PredecessorSha256) | Out-Null
            }
        }
        elseif ($backup.State -cne "absent") {
            throw "Exact rollback found an impossible backup for a new candidate path."
        }

        if ($entry.Kind -ceq "publish") {
            $stage = Get-RemoteBoundedIdentity -LogicalPath ([string]$entry.StagePath) -MaximumBytes $maximumBytes
            Remove-RemoteCandidateStageForRollback -Entry $entry -Identity $stage
        }

        if ($entry.HadPredecessor) {
            Assert-RemoteExactIdentity `
                -LogicalPath ([string]$entry.Path) `
                -Bytes ([int64]$entry.PredecessorBytes) `
                -Sha256 ([string]$entry.PredecessorSha256)
        }
        elseif ((Get-RemoteFileObservation -LogicalPath ([string]$entry.Path)).State -cne "absent") {
            throw "Exact rollback did not restore candidate-path absence: $($entry.Path)"
        }
        $entry.State = "rolled-back"
    }
}

function Complete-RemoteTransaction {
    Assert-RemoteLeaseOwned
    foreach ($entry in $script:transactionJournal) {
        if ($entry.Kind -ceq "publish") {
            Assert-RemoteExactIdentity `
                -LogicalPath ([string]$entry.Path) `
                -Bytes ([int64]$entry.CandidateBytes) `
                -Sha256 ([string]$entry.CandidateSha256)
            if ((Get-RemoteFileObservation -LogicalPath ([string]$entry.StagePath)).State -cne "absent") {
                throw "Transaction staging residue remained before commit."
            }
        }
        elseif ((Get-RemoteFileObservation -LogicalPath ([string]$entry.Path)).State -cne "absent") {
            throw "Retired path reappeared before transaction commit."
        }
        if ($entry.HadPredecessor) {
            Assert-RemoteExactIdentity `
                -LogicalPath ([string]$entry.BackupPath) `
                -Bytes ([int64]$entry.PredecessorBytes) `
                -Sha256 ([string]$entry.PredecessorSha256)
        }
    }

    $script:commitCleanupStarted = $true
    foreach ($entry in $script:transactionJournal) {
        if ($entry.HadPredecessor) {
            Remove-RemoteExactFile `
                -LogicalPath ([string]$entry.BackupPath) `
                -Bytes ([int64]$entry.PredecessorBytes) `
                -Sha256 ([string]$entry.PredecessorSha256) | Out-Null
            $entry.State = "committed"
        }
    }
}

function Get-RemoteTransactionArtifactPaths {
    $paths = @([string]$script:lease.OwnerPath)
    foreach ($entry in $script:transactionJournal) {
        if ($entry.HadPredecessor -and $entry.State -notin @("planned", "staged", "rolled-back", "committed")) {
            $paths += [string]$entry.BackupPath
        }
        if ($entry.Kind -ceq "publish" -and $entry.State -ceq "staged") {
            $paths += [string]$entry.StagePath
        }
    }
    return $paths
}

function Assert-BoundedReceiptPath {
    param([Parameter(Mandatory)][string]$Path)

    $receiptRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "deploy\receipts"))
    $resolved = [IO.Path]::GetFullPath($Path)
    if (-not $resolved.StartsWith($receiptRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path -Leaf $resolved) -notmatch "^site-deployment-[0-9]{8}T[0-9]{6}Z\.json$") {
        throw "Deployment receipt path is outside the bounded append-only receipt root."
    }
    return $resolved
}

function Write-Receipt {
    param([Parameter(Mandatory)][Collections.IDictionary]$Receipt, [Parameter(Mandatory)][string]$Path)

    Assert-CustodyMutationAllowed -Phase $script:phase -MutationKind local
    $resolved = Assert-BoundedReceiptPath -Path $Path
    if (Test-Path -LiteralPath $resolved) { throw "Deployment receipts are append-only." }
    $directory = Split-Path -Parent $resolved
    if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
        throw "Bounded deployment receipt root is unavailable."
    }
    $temporary = Join-Path $directory ((Split-Path -Leaf $resolved) + ".tmp")
    if (Test-Path -LiteralPath $temporary) { throw "Deployment receipt staging path is occupied." }
    [IO.File]::WriteAllText($temporary, (($Receipt | ConvertTo-Json -Depth 10) + "`n"), [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporary -Destination $resolved
}

function Assert-RemoteFileClosure {
    param([Parameter(Mandatory)]$Files)

    foreach ($file in @($Files)) {
        if (-not (Test-RemoteIdentity -LogicalPath ([string]$file.path) -Bytes ([int64]$file.bytes) -Sha256 ([string]$file.sha256))) {
            throw "Remote publication closure identity mismatch: $($file.path)"
        }
    }
}

function Assert-NoOverlayTransition {
    param([Parameter(Mandatory)]$PredecessorReceipt, [Parameter(Mandatory)]$Manifest)

    $candidate = @{}
    foreach ($file in @($Manifest.files)) { $candidate[[string]$file.path] = $true }
    $retired = @{}
    foreach ($path in @($Manifest.retired_paths)) { $retired[[string]$path] = $true }
    foreach ($file in @($PredecessorReceipt.files)) {
        $path = [string]$file.path
        if (-not $candidate.ContainsKey($path) -and -not $retired.ContainsKey($path)) {
            throw "Candidate publication would overlay an undeclared predecessor path."
        }
    }
}

$authorityRecord = Read-CustodyJsonFile -Path $AuthorityPath -MaximumBytes 131072 -Label "publication authority"
$authority = $authorityRecord.Value
Assert-CustodyPublicationAuthority -Authority $authority -RepositoryRoot $repositoryRoot -AuthorityPath $authorityRecord.Path
$validatedReportPath = Assert-BoundedReceiptPath -Path $ReportPath
$reportTemporaryPath = "$validatedReportPath.tmp"
if ((Test-Path -LiteralPath $validatedReportPath) -or (Test-Path -LiteralPath $reportTemporaryPath)) {
    throw "Deployment receipt or its staging path is already occupied."
}

$canonicalManifestPath = [IO.Path]::GetFullPath((Join-Path $repositoryRoot (([string]$authority.candidate.manifest_path) -replace "/", "\")))
if (-not [string]::Equals([IO.Path]::GetFullPath($ManifestPath), $canonicalManifestPath, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Only the canonical deployment manifest may be published."
}
$manifestRecord = Read-CustodyJsonFile -Path $canonicalManifestPath -MaximumBytes 2097152 -Label "deployment manifest"
$script:manifest = $manifestRecord.Value
$script:manifestSha256 = $manifestRecord.Sha256
$manifest = $script:manifest
if ($manifest.schema -cne "uaix.static_site_deployment_manifest.v1" -or
    $manifest.site -cne [string]$authority.site -or
    $manifest.origin -cne [string]$authority.origin -or
    $manifest.version -cne [string]$authority.candidate.version -or
    $manifest.publication_date -cne [string]$authority.candidate.publication_date -or
    $manifest.source_commit -cne [string]$authority.candidate.source_commit -or
    $manifest.source_tree -cne [string]$authority.candidate.source_tree -or
    $manifest.transport_profile -cne [string]$authority.transport.profile_name -or
    @($manifest.materializations).Count -ne 2 -or
    [string]$manifest.materializations[0].path -cne [string]$authority.predecessor.history_path -or
    [string]$manifest.materializations[1].path -cne [string]$authority.predecessor.release_path -or
    @($manifest.files).Count -lt 1 -or @($manifest.files).Count -gt 500 -or
    [string]$manifest.files[-1].path -cne [string]$authority.predecessor.release_path) {
    throw "Deployment manifest does not match the publication authority."
}

$seen = @{}
$totalBytes = [int64]0
foreach ($file in @($manifest.files)) {
    $relative = Assert-RelativePath -Path ([string]$file.path)
    if ($seen.ContainsKey($relative)) { throw "Deployment manifest contains a duplicate path." }
    $seen[$relative] = $true
    if ([int64]$file.bytes -lt 0 -or [int64]$file.bytes -gt 16777216 -or [string]$file.sha256 -notmatch "^[0-9a-f]{64}$") {
        throw "Deployment manifest contains an invalid file identity."
    }
    $totalBytes += [int64]$file.bytes
}
if ($totalBytes -gt 67108864) { throw "Deployment closure exceeds the total byte budget." }
$retiredSeen = @{}
foreach ($retiredPathValue in @($manifest.retired_paths)) {
    $retiredPath = Assert-RelativePath -Path ([string]$retiredPathValue)
    if ($seen.ContainsKey($retiredPath) -or $retiredSeen.ContainsKey($retiredPath)) {
        throw "Deployment manifest contains an active or duplicate retired path."
    }
    $retiredSeen[$retiredPath] = $true
}

$builderResult = Invoke-VerifiedNode -Tool manifest -Arguments @([string]$manifest.source_commit,'--verify')
if ($builderResult.ExitCode -ne 0 -or $builderResult.Stderr.Length -ne 0 -or $builderResult.Stdout -cne "VERIFIED $canonicalManifestPath`n$(@($manifest.files).Count) files`n") {
    $reason = $builderResult.Stderr.Trim()
    if ($reason.Length -gt 512) { $reason = $reason.Substring(0,512) }
    throw "Deployment manifest or current catalog admission failed: $reason"
}

$predecessorReceiptPath = [IO.Path]::GetFullPath((Join-Path $repositoryRoot (([string]$authority.predecessor.receipt_path) -replace "/", "\")))
$predecessorReceiptBytes = [IO.File]::ReadAllBytes($predecessorReceiptPath)
$predecessorReceipt = Assert-CustodyPredecessorReceipt -Authority $authority -ReceiptBytes $predecessorReceiptBytes
Assert-NoOverlayTransition -PredecessorReceipt $predecessorReceipt -Manifest $manifest

$repositoryObservation = Get-RepositoryObservation -Authority $authority
Assert-CustodyRepositoryIdentity -Authority $authority -Observed $repositoryObservation -ExpectedRepositoryRoot $repositoryRoot

$auditNonce = [Guid]::NewGuid().ToString("N").Substring(0, 12)
$handler = [Net.Http.HttpClientHandler]::new()
$handler.AllowAutoRedirect = $false
$client = [Net.Http.HttpClient]::new($handler)
$client.Timeout = [TimeSpan]::FromSeconds(60)
try {
    $publicReleaseBytes = Get-PublicBytes -Client $client -Origin ([string]$authority.origin) -RelativePath ([string]$authority.predecessor.release_path) -AuditNonce $auditNonce
    $publicHistoryBytes = Get-PublicBytes -Client $client -Origin ([string]$authority.origin) -RelativePath ([string]$authority.predecessor.history_path) -AuditNonce $auditNonce
    $publicCatalogBytes = Get-PublicBytes -Client $client -Origin ([string]$authority.origin) -RelativePath ([string]$authority.predecessor_discovery.composition_catalog.path) -AuditNonce $auditNonce
    $publicPeerBytes = Get-PublicBytes -Client $client -Origin ([string]$authority.origin) -RelativePath ([string]$authority.predecessor_discovery.peer_feed.path) -AuditNonce $auditNonce
}
finally {
    $client.Dispose()
    $handler.Dispose()
}

$candidateReleaseBytes = Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit ([string]$manifest.source_commit) -RelativePath ([string]$authority.predecessor.release_path)
$candidateHistoryBytes = Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit ([string]$manifest.source_commit) -RelativePath ([string]$authority.predecessor.history_path)
$candidateCatalogBytes = Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit ([string]$manifest.source_commit) -RelativePath ([string]$authority.candidate_discovery.composition_catalog.path)
$candidatePeerBytes = Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit ([string]$manifest.source_commit) -RelativePath ([string]$authority.candidate_discovery.peer_feed.path)
Assert-CustodyPublicPredecessor -Authority $authority `
    -PublicReleaseBytes $publicReleaseBytes `
    -PublicHistoryBytes $publicHistoryBytes `
    -PublicCatalogBytes $publicCatalogBytes `
    -PublicPeerBytes $publicPeerBytes `
    -CandidateReleaseBytes $candidateReleaseBytes `
    -CandidateHistoryBytes $candidateHistoryBytes `
    -CandidateCatalogBytes $candidateCatalogBytes `
    -CandidatePeerBytes $candidatePeerBytes

$postPublicRepositoryObservation = Get-RepositoryObservation -Authority $authority
Assert-CustodyRepositoryIdentity -Authority $authority -Observed $postPublicRepositoryObservation -ExpectedRepositoryRoot $repositoryRoot
if ([string]$postPublicRepositoryObservation.head -cne [string]$repositoryObservation.head -or
    [string]$postPublicRepositoryObservation.remote_head -cne [string]$repositoryObservation.remote_head) {
    throw "Repository or direct remote identity changed during read-only public admission."
}

$script:phase = "public-admitted"
Write-Host ("READ-ONLY-ADMISSION-PASS {0} files, {1} bytes, manifest {2}" -f @($manifest.files).Count, $totalBytes, $manifestRecord.Sha256)
if (-not $Publish) {
    if ($authority.transport.status -cne "bound") {
        Write-Host "PUBLISH-BLOCKED transport authority is intentionally unbound; no credentials were accessed"
    }
    exit 0
}
if ($authority.transport.status -cne "bound") {
    throw "Publication transport authority is not bound; credential access is forbidden."
}

$transactionDate = [DateTime]::UtcNow.ToString('yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture)
if ($authority.candidate.publication_date -cne $transactionDate) {
    throw 'Publication date differs from the current UTC transaction date; regenerate and review the canonical package before publishing.'
}
$operationNonce = [Guid]::NewGuid().ToString("N")
$script:publicationInput = New-CustodyPublicationInput `
    -Phase $script:phase -RepositoryRoot $repositoryRoot -Manifest $manifest `
    -ManifestSha256 $manifestRecord.Sha256 -OperationId $operationNonce -Authority $authority
$script:publicationInputSha256 = $script:publicationInput.BindingSha256
$script:publicationOperationId = $operationNonce
$script:phase = "input-verified"
$publicationReleaseBytes = Get-CustodyPublicationFileBytes `
    -InputRecord $script:publicationInput -InputSha256 $script:publicationInputSha256 `
    -ManifestSha256 $script:manifestSha256 -OperationId $script:publicationOperationId `
    -RelativePath ([string]$authority.predecessor.release_path)
$publicationHistoryBytes = Get-CustodyPublicationFileBytes `
    -InputRecord $script:publicationInput -InputSha256 $script:publicationInputSha256 `
    -ManifestSha256 $script:manifestSha256 -OperationId $script:publicationOperationId `
    -RelativePath ([string]$authority.predecessor.history_path)
Assert-CustodyPublishedReleasePair `
    -Authority $authority `
    -PreparedReleaseBytes $candidateReleaseBytes `
    -PreparedHistoryBytes $candidateHistoryBytes `
    -PublishedReleaseBytes $publicationReleaseBytes `
    -PublishedHistoryBytes $publicationHistoryBytes

$startedUtc = (Get-Date).ToUniversalTime().ToString("o")
$results = @()
$retiredResults = @()
$predecessorByPath = @{}
foreach ($file in @($predecessorReceipt.files)) { $predecessorByPath[[string]$file.path] = $file }
$currentPath = $null
$profile = $null
$publicationError = $null
$leaseAcquired = $false
$finalGatesPassed = $false
$rollbackCompleted = $false
try {
    Import-MiniModelCredentialModule
    $profile = Get-CredentialProfile -Path $FtpExportPath -ProfileName ([string]$authority.transport.profile_name)
    $observedTransport = [pscustomobject]@{
        profile_name = [string]$profile.profile
        protocol = "ftps-explicit"
        host = [string]$profile.Host
        port = [int]$profile.Port
        username = [string]$profile.User
        remote_root = [string]$authority.transport.remote_root
    }
    Assert-CustodyTransportAuthority -Authority $authority -Observed $observedTransport

    $script:ftpHost = ([string]$profile.Host).ToLowerInvariant()
    $script:ftpPort = [int]$profile.Port
    $script:remoteRoot = [string]$authority.transport.remote_root
    $script:credential = [Net.NetworkCredential]::new([string]$profile.User, [string]$profile.Password)
    $profile.Password = $null

    Assert-RemoteExactIdentity `
        -LogicalPath ([string]$authority.transport.root_sentinel_path) `
        -Bytes ([int64]$authority.transport.root_sentinel_bytes) `
        -Sha256 ([string]$authority.transport.root_sentinel_sha256)
    $script:phase = "transport-admitted"
    $script:lease = New-RemoteDeploymentLease `
        -Authority $authority `
        -OperationId $operationNonce `
        -PublisherCommit ([string]$repositoryObservation.head) `
        -ManifestSha256 $manifestRecord.Sha256 `
        -AuthoritySha256 $authorityRecord.Sha256
    $leaseAcquired = $true
    $script:phase = "leased"

    Assert-RemoteExactIdentity `
        -LogicalPath ([string]$authority.transport.root_sentinel_path) `
        -Bytes ([int64]$authority.transport.root_sentinel_bytes) `
        -Sha256 ([string]$authority.transport.root_sentinel_sha256)
    $predecessorPaths = @($predecessorReceipt.files | ForEach-Object { [string]$_.path }) + @(
        [string]$authority.transport.root_sentinel_path,
        [string]$script:lease.OwnerPath
    )
    $predecessorTree = Get-ObservedRemoteTree -ExpectedFilePaths $predecessorPaths
    Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $predecessorPaths -ObservedTree $predecessorTree
    Assert-RemoteFileClosure -Files $predecessorReceipt.files

    $leasedReleaseBytes = Get-RemoteBytes `
        -LogicalPath ([string]$authority.predecessor.release_path) `
        -MaximumBytes ([int64]$authority.predecessor.release_bytes)
    $leasedHistoryBytes = Get-RemoteBytes `
        -LogicalPath ([string]$authority.predecessor.history_path) `
        -MaximumBytes ([int64]$authority.predecessor.history_bytes)
    $leasedCatalogBytes = Get-RemoteBytes `
        -LogicalPath ([string]$authority.predecessor_discovery.composition_catalog.path) `
        -MaximumBytes ([int64]$authority.predecessor_discovery.composition_catalog.bytes)
    $leasedPeerBytes = Get-RemoteBytes `
        -LogicalPath ([string]$authority.predecessor_discovery.peer_feed.path) `
        -MaximumBytes ([int64]$authority.predecessor_discovery.peer_feed.bytes)
    Assert-CustodyPublicPredecessor -Authority $authority `
        -PublicReleaseBytes $leasedReleaseBytes `
        -PublicHistoryBytes $leasedHistoryBytes `
        -PublicCatalogBytes $leasedCatalogBytes `
        -PublicPeerBytes $leasedPeerBytes `
        -CandidateReleaseBytes $candidateReleaseBytes `
        -CandidateHistoryBytes $candidateHistoryBytes `
        -CandidateCatalogBytes $candidateCatalogBytes `
        -CandidatePeerBytes $candidatePeerBytes
    Assert-CustodyPublicationInput `
        -InputRecord $script:publicationInput -InputSha256 $script:publicationInputSha256 `
        -ManifestSha256 $script:manifestSha256 -OperationId $script:publicationOperationId

    $releaseMarkerPath = [string]$authority.predecessor.release_path
    foreach ($file in @($manifest.files | Where-Object { [string]$_.path -cne $releaseMarkerPath })) {
        $script:phase = "mutating"
        $currentPath = [string]$file.path
        $predecessorFile = if ($predecessorByPath.ContainsKey($currentPath)) { $predecessorByPath[$currentPath] } else { $null }
        $result = Publish-File -File $file -PredecessorFile $predecessorFile -Nonce $operationNonce
        $results += $result
        Write-Host ("{0} {1}" -f $result.status.ToUpperInvariant(), $result.path)
    }

    foreach ($retiredPath in @($manifest.retired_paths)) {
        $script:phase = "mutating"
        $currentPath = [string]$retiredPath
        if ($predecessorByPath.ContainsKey($currentPath)) {
            $retiredResults += Retire-RemoteFile -PredecessorFile $predecessorByPath[$currentPath] -Nonce $operationNonce
        }
        else {
            if ((Get-RemoteFileObservation -LogicalPath $currentPath).State -cne "absent") {
                throw "Retired path was outside the exact predecessor receipt closure."
            }
            $retiredResults += [ordered]@{ path = $currentPath; status = "already-absent" }
        }
    }

    $nonMarkerFiles = @($manifest.files | Where-Object { [string]$_.path -cne $releaseMarkerPath })
    $historyFile = @($manifest.files | Where-Object { [string]$_.path -ceq [string]$authority.predecessor.history_path })
    if ($historyFile.Count -ne 1) { throw "Deployment manifest must contain one release ledger." }
    Assert-RemoteFileClosure -Files $nonMarkerFiles
    foreach ($retiredPath in @($manifest.retired_paths)) {
        if ((Get-RemoteFileObservation -LogicalPath ([string]$retiredPath)).State -cne "absent") {
            throw "Retired remote path remained present before marker publication."
        }
    }
    $remotePublishedHistory = Get-RemoteBytes `
        -LogicalPath ([string]$authority.predecessor.history_path) `
        -MaximumBytes ([int64]$historyFile[0].bytes)
    Assert-CustodyPublishedReleasePair `
        -Authority $authority `
        -PreparedReleaseBytes $candidateReleaseBytes `
        -PreparedHistoryBytes $candidateHistoryBytes `
        -PublishedReleaseBytes $publicationReleaseBytes `
        -PublishedHistoryBytes $remotePublishedHistory
    $preMarkerPaths = @($nonMarkerFiles | ForEach-Object { [string]$_.path }) + @(
        $releaseMarkerPath,
        [string]$authority.transport.root_sentinel_path
    ) + @(Get-RemoteTransactionArtifactPaths)
    $preMarkerTree = Get-ObservedRemoteTree -ExpectedFilePaths $preMarkerPaths
    Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $preMarkerPaths -ObservedTree $preMarkerTree

    $releaseMarker = @($manifest.files | Where-Object { [string]$_.path -ceq $releaseMarkerPath })
    if ($releaseMarker.Count -ne 1) { throw "Deployment manifest must contain one release marker." }
    $currentPath = $releaseMarkerPath
    $markerPredecessor = if ($predecessorByPath.ContainsKey($releaseMarkerPath)) { $predecessorByPath[$releaseMarkerPath] } else { $null }
    $markerResult = Publish-File -File $releaseMarker[0] -PredecessorFile $markerPredecessor -Nonce $operationNonce
    $results += $markerResult
    Write-Host ("{0} {1}" -f $markerResult.status.ToUpperInvariant(), $markerResult.path)

    foreach ($retiredPath in @($manifest.retired_paths)) {
        if ((Get-RemoteFileObservation -LogicalPath ([string]$retiredPath)).State -cne "absent") {
            throw "Retired remote path remained present."
        }
    }
    Assert-RemoteFileClosure -Files $manifest.files
    Assert-RemoteExactIdentity `
        -LogicalPath ([string]$authority.transport.root_sentinel_path) `
        -Bytes ([int64]$authority.transport.root_sentinel_bytes) `
        -Sha256 ([string]$authority.transport.root_sentinel_sha256)
    $remotePublishedRelease = Get-RemoteBytes `
        -LogicalPath $releaseMarkerPath `
        -MaximumBytes ([int64]$releaseMarker[0].bytes)
    $remotePublishedHistory = Get-RemoteBytes `
        -LogicalPath ([string]$authority.predecessor.history_path) `
        -MaximumBytes ([int64]$historyFile[0].bytes)
    Assert-CustodyPublishedReleasePair `
        -Authority $authority `
        -PreparedReleaseBytes $candidateReleaseBytes `
        -PreparedHistoryBytes $candidateHistoryBytes `
        -PublishedReleaseBytes $remotePublishedRelease `
        -PublishedHistoryBytes $remotePublishedHistory
    $candidatePaths = @($manifest.files | ForEach-Object { [string]$_.path }) + @(
        [string]$authority.transport.root_sentinel_path
    ) + @(Get-RemoteTransactionArtifactPaths)
    $candidateTree = Get-ObservedRemoteTree -ExpectedFilePaths $candidatePaths
    Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $candidatePaths -ObservedTree $candidateTree

    $handler = [Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $client = [Net.Http.HttpClient]::new($handler)
    $client.Timeout = [TimeSpan]::FromSeconds(60)
    $httpsPublishedRelease = $null
    $httpsPublishedHistory = $null
    try {
        foreach ($file in @($manifest.files | Where-Object { $null -ne $_.https_readback_path })) {
            $body = Get-PublicBytes `
                -Client $client `
                -Origin ([string]$manifest.origin) `
                -RelativePath ([string]$file.path) `
                -AuditNonce $operationNonce `
                -MaximumBytes ([Math]::Max([int64]1, [int64]$file.bytes))
            if ($body.Length -ne [int64]$file.bytes -or
                (Get-CustodyByteSha256 -Bytes $body) -cne [string]$file.sha256) {
                throw "HTTPS readback identity mismatch for $($file.path)."
            }
            if ([string]$file.path -ceq $releaseMarkerPath) { $httpsPublishedRelease = $body }
            if ([string]$file.path -ceq [string]$authority.predecessor.history_path) { $httpsPublishedHistory = $body }
        }
        Assert-CustodyPublishedReleasePair `
            -Authority $authority `
            -PreparedReleaseBytes $candidateReleaseBytes `
            -PreparedHistoryBytes $candidateHistoryBytes `
            -PublishedReleaseBytes $httpsPublishedRelease `
            -PublishedHistoryBytes $httpsPublishedHistory

        foreach ($route in @($manifest.route_smokes)) {
            $url = "{0}{1}?release_smoke={2}" -f $manifest.origin, $route.path, $operationNonce
            $response = $client.GetAsync($url).GetAwaiter().GetResult()
            try {
                $contentLength = $response.Content.Headers.ContentLength
                if ($null -ne $contentLength -and [int64]$contentLength -gt 2097152) {
                    throw "HTTPS route smoke exceeded its byte bound."
                }
                $body = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
                if ($body.Length -gt 2097152 -or [int]$response.StatusCode -ne 200 -or -not $body.Contains([string]$route.contains)) {
                    throw "HTTPS route smoke failed for $($route.path)."
                }
            }
            finally { $response.Dispose() }
        }

        foreach ($retiredPath in @($manifest.retired_paths)) {
            $url = "{0}/{1}?release_retired={2}" -f $manifest.origin, [string]$retiredPath, $operationNonce
            $response = $client.GetAsync($url).GetAwaiter().GetResult()
            try {
                if ([int]$response.StatusCode -notin @(404, 410)) {
                    throw "Retired HTTPS route remained available: $retiredPath"
                }
            }
            finally { $response.Dispose() }
        }
    }
    finally {
        $client.Dispose()
        $handler.Dispose()
    }

    $finalGatesPassed = $true
    $script:phase = "finalizing"
    Complete-RemoteTransaction
    Remove-RemoteDeploymentLease
    $leaseAcquired = $false

    $script:publicationInput = $null
    $script:phase = "complete"

    $receipt = [ordered]@{
        schema = "uaix.site_deployment_receipt.v2"
        site = [string]$manifest.site
        origin = [string]$manifest.origin
        version = [string]$manifest.version
        source_commit = [string]$manifest.source_commit
        source_tree = [string]$manifest.source_tree
        publisher_commit = [string]$repositoryObservation.head
        manifest_sha256 = $manifestRecord.Sha256
        authority_sha256 = $authorityRecord.Sha256
        predecessor_receipt_sha256 = [string]$authority.predecessor.receipt_sha256
        publication_input_sha256 = $script:publicationInputSha256
        operation_nonce = $operationNonce
        started_utc = $startedUtc
        completed_utc = (Get-Date).ToUniversalTime().ToString("o")
        status = "published-and-readback-verified"
        file_count = @($manifest.files).Count
        total_bytes = $totalBytes
        published_count = @($results | Where-Object { $_.status -eq "published" }).Count
        already_current_count = @($results | Where-Object { $_.status -eq "already-current" }).Count
        files = $results
        retired_count = $retiredResults.Count
        retired_paths = $retiredResults
    }
    Write-Receipt -Receipt $receipt -Path $validatedReportPath
    Write-Host "RECEIPT $validatedReportPath"
}
catch {
    $publicationError = $_.Exception
    $rollbackStatus = "not-required"
    if ($leaseAcquired -and -not $script:commitCleanupStarted) {
        try {
            Invoke-RemoteExactRollback
            Assert-RemoteExactIdentity `
                -LogicalPath ([string]$authority.transport.root_sentinel_path) `
                -Bytes ([int64]$authority.transport.root_sentinel_bytes) `
                -Sha256 ([string]$authority.transport.root_sentinel_sha256)
            Assert-RemoteFileClosure -Files $predecessorReceipt.files
            $rollbackPaths = @($predecessorReceipt.files | ForEach-Object { [string]$_.path }) + @(
                [string]$authority.transport.root_sentinel_path,
                [string]$script:lease.OwnerPath
            )
            $rollbackTree = Get-ObservedRemoteTree -ExpectedFilePaths $rollbackPaths
            Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $rollbackPaths -ObservedTree $rollbackTree
            Remove-RemoteDeploymentLease
            $leaseAcquired = $false
            $rollbackCompleted = $true
            $rollbackStatus = "exact-predecessor-restored"
        }
        catch {
            $rollbackStatus = "failed-closed-lease-retained"
            $publicationError = [InvalidOperationException]::new(
                "Publication failed and bounded exact rollback could not prove predecessor restoration; the operation lease was retained.",
                $publicationError
            )
        }
    }
    elseif ($leaseAcquired -and $script:commitCleanupStarted) {
        $rollbackStatus = "final-gates-passed-cleanup-incomplete-lease-retained"
        $publicationError = [InvalidOperationException]::new(
            "Publication final gates passed, but post-gate backup or lease cleanup did not complete; the lease was retained fail closed.",
            $publicationError
        )
    }

    $script:publicationInput = $null
    if ($script:phase -ceq "read-only") { throw $publicationError }
    $failure = [ordered]@{
        schema = "uaix.site_deployment_receipt.v2"
        site = [string]$manifest.site
        origin = [string]$manifest.origin
        version = [string]$manifest.version
        source_commit = [string]$manifest.source_commit
        source_tree = [string]$manifest.source_tree
        publisher_commit = [string]$repositoryObservation.head
        manifest_sha256 = $manifestRecord.Sha256
        authority_sha256 = $authorityRecord.Sha256
        predecessor_receipt_sha256 = [string]$authority.predecessor.receipt_sha256
        publication_input_sha256 = $script:publicationInputSha256
        operation_nonce = $operationNonce
        started_utc = $startedUtc
        completed_utc = (Get-Date).ToUniversalTime().ToString("o")
        status = "failed"
        final_gates_passed = $finalGatesPassed
        rollback_status = $rollbackStatus
        failure_code = Get-SafeFailureCode -Exception $publicationError
        current_path = $currentPath
        completed_file_count = $results.Count
        files = $results
        completed_retired_count = $retiredResults.Count
        retired_paths = $retiredResults
    }
    Write-Receipt -Receipt $failure -Path $validatedReportPath
    throw $publicationError
}
finally {
    if ($profile) { $profile.Password = $null }
    $script:credential = $null
}
