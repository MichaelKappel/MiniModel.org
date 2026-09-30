param(
    [string]$AuthorityPath = (Join-Path (Split-Path -Parent $PSScriptRoot) "deploy\publication-authority.v1.json"),
    [string]$ManifestPath = (Join-Path (Split-Path -Parent $PSScriptRoot) "deploy\site-deployment-manifest.v1.json"),
    [string]$FtpExportPath = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "ftp.txt"),
    [string]$ReportPath = (Join-Path (Split-Path -Parent $PSScriptRoot) ("deploy\receipts\site-deployment-{0}.json" -f (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ"))),
    [string]$NodeExecutablePath = (Join-Path $env:ProgramFiles 'nodejs\node.exe'),
    [switch]$Publish,
    [switch]$BootstrapSentinel,
    [switch]$PublishSeed,
    [switch]$ProbeHostedRenewal,
    [string]$SeedSourceDirectory
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Net.Http
if (@(@($Publish,$BootstrapSentinel,$PublishSeed,$ProbeHostedRenewal) | Where-Object {$_}).Count -gt 1) { throw 'Choose exactly one publication operation.' }
if ($PublishSeed -ne (-not [string]::IsNullOrWhiteSpace($SeedSourceDirectory))) { throw 'Seed publication requires its explicit canonical source directory only.' }

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

# Execute reviewed tools with their pinned dependency closure, retained read leases and bounded pipes.
function Invoke-VerifiedNode {
    param(
        [Parameter(Mandatory)][ValidateSet('source-authority','manifest','seed-admission')][string]$Tool,
        [AllowEmptyCollection()][string[]]$Arguments = @(),
        [AllowNull()][AllowEmptyCollection()][byte[]]$InputBytes,
        [ValidateRange(1,120000)][int]$TimeoutMilliseconds = 120000
    )

    $dependencyIdentities = @()
    if ($Tool -cne 'seed-admission' -and $null -ne $InputBytes) { throw 'This verified tool does not accept stdin bytes.' }
    if ($Tool -ceq 'source-authority') {
        if (@($Arguments | Where-Object { $_ -cnotin @('--require-clean','--require-remote-exact') }).Count -gt 0 -or
            @($Arguments | Select-Object -Unique).Count -ne $Arguments.Count) { throw 'Invalid source-authority arguments.' }
        $toolPath = Join-Path (Split-Path -Parent $repositoryRoot) 'TinyRustLM.com\tinyrustlm\tools\verify-single-source-authority.js'
        $toolBytes = 144521
        $toolSha256 = 'e8867b01d00399f319852449ff80c249edab30a8bcee4c09fd3789300f65aaf7'
        $maximumOutput = 4096
    }
    elseif ($Tool -ceq 'manifest') {
        if ($Arguments.Count -lt 1 -or $Arguments.Count -gt 2 -or $Arguments[0].Length -ne 40 -or $Arguments[0] -cnotmatch '^[0-9a-f]{40}$' -or
            ($Arguments.Count -eq 2 -and $Arguments[1] -cne '--verify')) { throw 'Invalid manifest arguments.' }
        $toolPath = Join-Path $repositoryRoot 'tools\build-site-deployment-manifest.js'
        $toolBytes = 17615
        $toolSha256 = 'bd9276bc4a1296db550b7fb6511a6d2ba257b481b08d212c1f85724a3015e305'
        $maximumOutput = 2097152
    }
    else {
        if ($Arguments.Count -ne 0 -or $null -eq $InputBytes -or $InputBytes.Length -lt 1 -or $InputBytes.Length -gt 3145728) {
            throw 'Invalid seed-admission input.'
        }
        $toolPath = Join-Path $repositoryRoot 'tools\admit-static-seed-publication.js'
        $toolBytes = 7611
        $toolSha256 = '88abfd7e0b0f3633cc121e7cfb3c35a94b7da1e9d889f1f159c610362918035c'
        $maximumOutput = 262144
        $dependencyIdentities = @(
            @{Path=(Join-Path $repositoryRoot 'tools\build-static-seed-plan.js');Bytes=7934;Sha256='47821c9a826be9f0cf0ee0f44ec6aaf4fe24863dc912dc1a5f6a0ac08f47048d'},
            @{Path=(Join-Path $repositoryRoot 'catalog.js');Bytes=52668;Sha256='353a9a986931dfbee8306185f3254f9b8e12202a38cf20ea7f61ccdc8f6fe549'},
            @{Path=(Join-Path (Split-Path -Parent $repositoryRoot) 'TinyRustLM.com\tinyrustlm\app\minimodel-catalog-v1.js');Bytes=76134;Sha256='1ac264b1e347c61bd2e8bee1b878978cdf3e7077e60748305ef7bb65664b01fa'},
            @{Path=(Join-Path (Split-Path -Parent $repositoryRoot) 'TinyRustLM.com\tinyrustlm\app\browser-model-admission.js');Bytes=14858;Sha256='87cdc059e71435f1572b3ad03fcce17701d5b73e83c3a6172e078a3349e5b683'},
            @{Path=(Join-Path (Split-Path -Parent $repositoryRoot) 'TinyRustLM.com\tinyrustlm\app\catalog-admission-loader.js');Bytes=38187;Sha256='623bb655aacdb075ea575193a64ef91469c6eda545e54b1dd4718fcf22493951'},
            @{Path=(Join-Path (Split-Path -Parent $repositoryRoot) 'TinyRustLM.com\tinyrustlm\app\operation-lifetime.js');Bytes=2292;Sha256='4a9d907a3c62b679f896d859e843124bf4252e6d2be0f8aacc7e754113a9f570'}
        )
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
        foreach ($identity in (@(
            @{Path=$nodePath;Bytes=91380224;Sha256='63c259c81e5d472b5f11c8d506070130cb04a1ecf84b80377a34ed6ec9048088'},
            @{Path=$toolPath;Bytes=$toolBytes;Sha256=$toolSha256}
        ) + $dependencyIdentities)) {
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
        $start.RedirectStandardInput=($null -ne $InputBytes)
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
        if ($null -ne $InputBytes) {
            $stdinTask=$process.StandardInput.BaseStream.WriteAsync($InputBytes,0,$InputBytes.Length)
            if (-not $stdinTask.Wait([Math]::Max(0,$TimeoutMilliseconds-[int]$watch.ElapsedMilliseconds))) {
                throw 'Verified Node exceeded its stdin deadline.'
            }
            $process.StandardInput.Close()
        }
        if (-not [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdoutTask,$stderrTask),[Math]::Max(0,$TimeoutMilliseconds-[int]$watch.ElapsedMilliseconds)) -or
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
    if ($Publish -or $BootstrapSentinel -or $PublishSeed -or $ProbeHostedRenewal) {
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
Import-Module -Name (Join-Path $PSScriptRoot 'MiniModel.StaticSeed.psm1') -Force

$script:phase = "read-only"
$script:credential = $null
$script:ftpHost = $null
$script:ftpPort = 0
$script:remoteRoot = $null
$script:publicationInput = $null
$script:publicationInputSha256 = $null
$script:publicationOperationId = $null
$script:lease = $null
$script:seedFilePaths = @()
$script:seedOperationId = ''
$script:seedTransferFiles = @()
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

# Compare working bytes with the exact commit captured by repository observation.
function Test-TrackedPathMatchesHead {
    param(
        [Parameter(Mandatory)][string]$RelativePath,
        [Parameter(Mandatory)][string]$ObservedCommit
    )

    $tracked = @(& git --no-optional-locks -C $repositoryRoot ls-files -- $RelativePath 2>$null)
    if ($LASTEXITCODE -ne 0 -or $tracked.Count -ne 1 -or ([string]$tracked[0]).Trim() -cne $RelativePath) { return $false }
    $headBytes = Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $ObservedCommit -RelativePath $RelativePath
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
        manifest_matches_head = Test-TrackedPathMatchesHead -RelativePath ([string]$Authority.candidate.manifest_path) -ObservedCommit $head
        authority_matches_head = Test-TrackedPathMatchesHead -RelativePath "deploy/publication-authority.v1.json" -ObservedCommit $head
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
    if (-not [string]::IsNullOrEmpty($LogicalPath)) {
        $null = Assert-CustodyRemotePath -Path $LogicalPath -SeedFilePaths $script:seedFilePaths -SeedOperationId $script:seedOperationId -AllowAncestor
    }
    $encoded = Get-EncodedRemotePath -LogicalPath $LogicalPath
    # NLST must address a directory; a file-shaped URI makes .NET send NLST <path>, yielding prefixed entries.
    if ($Method -ceq [Net.WebRequestMethods+Ftp]::ListDirectory -and $encoded.Length -gt 0 -and -not $encoded.EndsWith('/')) {
        $encoded += '/'
    }
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
        return ,$memory.ToArray()
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
    $memory = [IO.MemoryStream]::new()
    $buffer = [byte[]]::new(4096)
    try {
        while (($count=$stream.Read($buffer,0,$buffer.Length)) -gt 0) {
            if ($memory.Length+$count -gt 65536) { throw 'Remote directory listing exceeds its byte bound.' }
            $memory.Write($buffer,0,$count)
        }
        return ConvertFrom-CustodyDirectoryListing -Bytes $memory.ToArray() -DirectoryPath $LogicalPath -SeedFilePaths $script:seedFilePaths -SeedOperationId $script:seedOperationId
    }
    finally {
        $memory.Dispose()
        $stream.Dispose()
        $response.Dispose()
    }
}

function Get-RemoteParentAndLeaf {
    param([Parameter(Mandatory)][string]$LogicalPath)

    $relative = Assert-CustodyRemotePath -Path $LogicalPath -SeedFilePaths $script:seedFilePaths -SeedOperationId $script:seedOperationId -AllowAncestor
    $lastSlash = $relative.LastIndexOf("/")
    return [pscustomobject]@{
        Parent = if ($lastSlash -ge 0) { $relative.Substring(0, $lastSlash) } else { "" }
        Leaf = if ($lastSlash -ge 0) { $relative.Substring($lastSlash + 1) } else { $relative }
    }
}

function Test-ExactListedLeaf {
    param([Parameter(Mandatory)][AllowEmptyCollection()][string[]]$Entries, [Parameter(Mandatory)][string]$Leaf)

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
    $parts = Get-RemoteParentAndLeaf -LogicalPath $LogicalPath
    $target = if ($parts.Parent.Length -eq 0) { $NewLeafName } else { "$($parts.Parent)/$NewLeafName" }
    $null = Assert-CustodyRemotePath -Path $target -SeedFilePaths $script:seedFilePaths -SeedOperationId $script:seedOperationId
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

# Read exact committed public metadata and invoke the canonical browser qualification/admission readers.
function Get-StaticSeedPublicationInput {
    param([Parameter(Mandatory)][string]$Commit)
    $registryBytes=Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $Commit -RelativePath 'deploy/static-seed-namespace.v1.json' -MaximumBytes 16384
    $utf8=[Text.UTF8Encoding]::new($false,$true)
    $registry=$utf8.GetString($registryBytes)|ConvertFrom-Json
    if (@($registry.PSObject.Properties.Name).Count -ne 2 -or $registry.schema -cne 'minimodel.static-seed-namespace/1' -or
        $registry.descriptorBase64 -isnot [string] -or $registry.descriptorBase64.Length -lt 1 -or $registry.descriptorBase64.Length -gt 10924) {
        throw 'No exact static seed descriptor is registered for publication.'
    }
    $registryCanonical=$utf8.GetBytes((([ordered]@{schema=$registry.schema;descriptorBase64=$registry.descriptorBase64}|ConvertTo-Json -Compress)+"`n"))
    $descriptor=[Convert]::FromBase64String($registry.descriptorBase64)
    if ($descriptor.Length -gt 8192 -or [Convert]::ToBase64String($descriptor) -cne $registry.descriptorBase64 -or
        $registryBytes.Length -ne $registryCanonical.Length -or
        (Get-CustodyByteSha256 -Bytes $registryBytes) -cne (Get-CustodyByteSha256 -Bytes $registryCanonical)) {
        throw 'Static seed registry does not contain canonical descriptor bytes.'
    }
    $catalogBytes=Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $Commit -RelativePath 'catalog/compositions.v1.json' -MaximumBytes 1048576
    $peerBytes=Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $Commit -RelativePath 'catalog/peer-announcements.v1.json' -MaximumBytes 1048576
    $catalog=$utf8.GetString($catalogBytes)|ConvertFrom-Json
    if (@($catalog.entries).Count -ne 1) { throw 'Static seed publication requires one current catalog entry.' }
    $entry=$catalog.entries[0]
    $routes=[ordered]@{admission=[string]$entry.browser_admission.route;qualificationReceipt=[string]$entry.qualification.receipt_route}
    $inputs=[ordered]@{descriptor=$descriptor;catalog=$catalogBytes;peerFeed=$peerBytes}
    foreach($name in $routes.Keys) {
        $route=$routes[$name]
        if (-not $route.StartsWith('/') -or $route.StartsWith('//')) { throw 'Static seed metadata route is not local.' }
        $relative=Assert-RelativePath -Path $route.Substring(1)
        $maximum=if($name -ceq 'admission'){65536}else{131072}
        $inputs[$name]=Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $Commit -RelativePath $relative -MaximumBytes $maximum
    }
    $envelope=[ordered]@{}
    foreach($name in $inputs.Keys){$envelope[$name]=[Convert]::ToBase64String($inputs[$name])}
    $inputBytes=$utf8.GetBytes((($envelope|ConvertTo-Json -Compress)+"`n"))
    $result=Invoke-VerifiedNode -Tool seed-admission -InputBytes $inputBytes
    if ($result.ExitCode -ne 0 -or $result.Stderr.Length -ne 0) { throw 'Canonical static seed qualification and metadata admission failed.' }
    $admitted=$result.Stdout|ConvertFrom-Json
    if ($admitted.schema -cne 'minimodel.qualified-static-seed-plan/1' -or
        $admitted.status -cne 'metadata-admitted-payload-and-host-unverified' -or $admitted.publicationAllowed -ne $false) {
        throw 'Static seed metadata admission returned an unsupported result.'
    }
    foreach($name in $inputs.Keys) {
        if ($admitted.metadata.$name.bytes -ne $inputs[$name].Length -or
            $admitted.metadata.$name.sha256 -cne (Get-CustodyByteSha256 -Bytes $inputs[$name])) {
            throw 'Static seed metadata admission consumed different bytes.'
        }
    }
    return [pscustomobject]@{Commit=$Commit;RegistrySha256=(Get-CustodyByteSha256 -Bytes $registryBytes);Inputs=$inputs;Admission=$admitted}
}

# Resolve only one of the seven admitted immutable members or its current operation's exact stage.
function Get-RegisteredSeedTransferFile {
    param([Parameter(Mandatory)][string]$LogicalPath,[switch]$RequireStage)
    $null = Assert-CustodyRemotePath -Path $LogicalPath -SeedFilePaths $script:seedFilePaths -SeedOperationId $script:seedOperationId
    if ($script:seedTransferFiles.Count -ne 7 -or $script:seedOperationId -cnotmatch '^[0-9a-f]{32}$') {
        throw 'Static seed transfer has no admitted member closure.'
    }
    $found = @($script:seedTransferFiles | Where-Object {
        $LogicalPath -ceq "$($_.path).seed-$($script:seedOperationId).tmp" -or
        (-not $RequireStage -and $LogicalPath -ceq [string]$_.path)
    })
    if ($found.Count -ne 1 -or [int64]$found[0].bytes -lt 1 -or [int64]$found[0].bytes -gt 68719476736 -or
        ([string]$found[0].sha256).Length -ne 64 -or [string]$found[0].sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        [string]$found[0].path -cnotin $script:seedFilePaths) {
        throw 'Static seed transfer path or identity is not admitted.'
    }
    return $found[0]
}

# Stream from a retained read-only source handle; no model-sized byte array or FTP append loop is used.
function Send-RemoteSeedStream {
    param([Parameter(Mandatory)][IO.Stream]$Source,[Parameter(Mandatory)][string]$LogicalPath)
    $file = Get-RegisteredSeedTransferFile -LogicalPath $LogicalPath -RequireStage
    if (-not $Source.CanRead -or -not $Source.CanSeek -or $Source.Length -ne [int64]$file.bytes) {
        throw 'Static seed upload source is not the exact retained stream.'
    }
    Assert-RemoteLeaseOwned
    if ((Get-RemoteFileObservation -LogicalPath $LogicalPath).State -cne 'absent') {
        throw 'Static seed staging path was not exactly absent.'
    }
    $Source.Position = 0
    $request = New-FtpRequest -LogicalPath $LogicalPath -Method ([Net.WebRequestMethods+Ftp]::UploadFile) -Mutating
    $request.ContentLength = [int64]$file.bytes
    $output = $null
    $completed = $false
    try {
        $output = $request.GetRequestStream()
        $identity = Copy-StaticSeedVerifiedStream -Source $Source -Destination $output -ExpectedBytes ([int64]$file.bytes) -ExpectedSha256 ([string]$file.sha256)
        $output.Dispose(); $output = $null
        $response = $request.GetResponse()
        try { $completed = $true } finally { $response.Dispose() }
        return $identity
    }
    finally {
        if ($output) { try { $output.Dispose() } catch { } }
        if (-not $completed) { $request.Abort() }
    }
}

# Verify the complete remote seed body in a 64 KiB buffer after exact LIST/SIZE admission.
function Assert-RemoteSeedStreamIdentity {
    param([Parameter(Mandatory)][string]$LogicalPath)
    $file = Get-RegisteredSeedTransferFile -LogicalPath $LogicalPath
    $observation = Get-RemoteFileObservation -LogicalPath $LogicalPath
    if ($observation.State -cne 'present' -or [int64]$observation.Size -ne [int64]$file.bytes) {
        throw 'Remote seed SIZE differs from its admitted identity.'
    }
    $request = New-FtpRequest -LogicalPath $LogicalPath -Method ([Net.WebRequestMethods+Ftp]::DownloadFile)
    $response = $request.GetResponse()
    $stream = $null
    try {
        $stream = $response.GetResponseStream()
        return Copy-StaticSeedVerifiedStream -Source $stream -ExpectedBytes ([int64]$file.bytes) -ExpectedSha256 ([string]$file.sha256)
    }
    finally {
        if ($stream) { $stream.Dispose() }
        $response.Dispose()
    }
}

# Bind the admitted metadata and retained source handles to one exact transfer closure.
function New-StaticSeedTransferSet {
    param([Parameter(Mandatory)]$InputRecord,[Parameter(Mandatory)]$SourceSet,[Parameter(Mandatory)][string]$OperationId)
    $plan=$InputRecord.Admission.plan
    if($plan.schema -cne 'minimodel.static-seed-preparation.v1' -or $plan.origin -cne 'https://minimodel.org' -or
        @($plan.files).Count -ne 6 -or @($SourceSet.Members).Count -ne 6) { throw 'Seed transfer has no exact admitted preparation.' }
    $paths=@($plan.files|ForEach-Object {[string]$_.remotePath})+@([string]$plan.configuration.remotePath)
    foreach($path in $paths){$null=Assert-CustodyRemotePath -Path $path -SeedFilePaths $paths -SeedOperationId $OperationId}
    $files=[Collections.Generic.List[object]]::new()
    for($index=0;$index -lt 6;$index++) {
        $file=$plan.files[$index];$source=$SourceSet.Members[$index]
        if($file.name -cne $source.Name -or [int64]$file.bytes -ne $source.Bytes -or $file.sha256 -cne $source.Sha256 -or
            $file.url -cne ('https://minimodel.org/'+$file.remotePath) -or
            $file.remotePath -cne ($plan.remoteDirectory+'/'+$file.name)) {throw 'Seed transfer source and descriptor differ.'}
        $files.Add([pscustomobject]@{path=[string]$file.remotePath;bytes=[int64]$file.bytes;sha256=[string]$file.sha256;stream=$source.Stream})
    }
    $configuration=[Text.UTF8Encoding]::new($false,$true).GetBytes([string]$plan.configuration.text)
    if($configuration.Length -lt 1 -or $configuration.Length -gt 65536 -or
        $configuration.Length -ne $plan.configuration.bytes -or
        (Get-CustodyByteSha256 $configuration) -cne $plan.configuration.sha256) {throw 'Seed configuration identity drifted.'}
    $binding=[ordered]@{schema='minimodel.static-seed-transfer/1';operation_id=$OperationId;publisher_commit=$InputRecord.Commit;
        registry_sha256=$InputRecord.RegistrySha256;metadata=$InputRecord.Admission.metadata;files=@($files|ForEach-Object {[ordered]@{path=$_.path;bytes=$_.bytes;sha256=$_.sha256}});
        configuration=[ordered]@{path=$plan.configuration.remotePath;bytes=$configuration.Length;sha256=$plan.configuration.sha256}}
    $bindingBytes=[Text.Encoding]::UTF8.GetBytes((($binding|ConvertTo-Json -Depth 12 -Compress)+"`n"))
    $configurationStream=[IO.MemoryStream]::new($configuration,$false)
    $files.Add([pscustomobject]@{path=[string]$plan.configuration.remotePath;bytes=$configuration.Length;sha256=[string]$plan.configuration.sha256;stream=$configurationStream})
    return [pscustomobject]@{FilePaths=$paths;Files=$files.ToArray();ConfigurationStream=$configurationStream;BindingSha256=(Get-CustodyByteSha256 $bindingBytes)}
}

# A namespace can be entirely absent or have the exact seven-file closure. Never adopt partial output.
function Get-StaticSeedNamespaceState {
    param([Parameter(Mandatory)]$Plan)
    $null=Assert-CustodyRemotePath -Path $Plan.remoteDirectory -SeedFilePaths $script:seedFilePaths -SeedOperationId $script:seedOperationId -AllowAncestor
    $parent=''
    foreach($leaf in ($Plan.remoteDirectory -split '/')) {
        $before=@(Get-RemoteDirectoryEntries -LogicalPath $parent)
        $present=Test-ExactListedLeaf -Entries $before -Leaf $leaf
        $after=@(Get-RemoteDirectoryEntries -LogicalPath $parent)
        if($present -ne (Test-ExactListedLeaf -Entries $after -Leaf $leaf)){throw 'Seed ancestor observations changed.'}
        if(-not $present){return 'absent'}
        $parent=if($parent.Length -eq 0){$leaf}else{"$parent/$leaf"}
    }
    $entries=@(Get-RemoteDirectoryEntries -LogicalPath $parent)
    $expected=@($script:seedFilePaths|ForEach-Object {$_.Substring($_.LastIndexOf('/')+1)})
    if($entries.Count -ne 7 -or @($entries|Where-Object {$_ -cnotin $expected}).Count -gt 0) {throw 'Seed namespace is partial or contains unowned entries.'}
    foreach($name in $expected){if(-not (Test-ExactListedLeaf -Entries $entries -Leaf $name)){throw 'Seed namespace is partial or contains unowned entries.'}}
    return 'complete'
}

# Create only exact registered ancestors under the shared lease; journal attempts before mutation.
function New-StaticSeedDirectories {
    param([Parameter(Mandatory)]$Plan,[Parameter(Mandatory)][AllowEmptyCollection()][Collections.Generic.List[object]]$Journal)
    $parent=''
    foreach($leaf in ($Plan.remoteDirectory -split '/')) {
        $path=if($parent.Length -eq 0){$leaf}else{"$parent/$leaf"}
        $before=@(Get-RemoteDirectoryEntries -LogicalPath $parent)
        if(-not (Test-ExactListedLeaf -Entries $before -Leaf $leaf)) {
            $after=@(Get-RemoteDirectoryEntries -LogicalPath $parent)
            if(Test-ExactListedLeaf -Entries $after -Leaf $leaf){throw 'Seed ancestor changed before creation.'}
            Assert-RemoteLeaseOwned
            $Journal.Add([ordered]@{step='directory-create-attempted';path=$path})
            $request=New-FtpRequest -LogicalPath $path -Method ([Net.WebRequestMethods+Ftp]::MakeDirectory) -Mutating
            $response=$request.GetResponse();$response.Dispose()
            if(@(Get-RemoteDirectoryEntries -LogicalPath $path).Count -ne 0){throw 'New seed ancestor is not exactly empty.'}
        } else {Get-RemoteDirectoryEntries -LogicalPath $path|Out-Null}
        $parent=$path
    }
    if(@(Get-RemoteDirectoryEntries -LogicalPath $Plan.remoteDirectory).Count -ne 0){throw 'New seed namespace is occupied.'}
}

# One fixed no-cookie, no-redirect client; disposing it also disposes its owned handler.
function New-StaticSeedHttpClient {
    $handler=[Net.Http.HttpClientHandler]::new();$handler.AllowAutoRedirect=$false;$handler.UseCookies=$false
    $handler.AutomaticDecompression=[Net.DecompressionMethods]::None
    $client=[Net.Http.HttpClient]::new($handler);$client.Timeout=[Threading.Timeout]::InfiniteTimeSpan
    return $client
}

# The same deadline covers HTTP headers and the entire streaming body. Redirects/decompression are disabled.
function Assert-PublicStaticSeedMembers {
    param([Parameter(Mandatory)]$Plan,[Parameter(Mandatory)][AllowEmptyCollection()][Collections.Generic.List[object]]$Journal)
    $client=New-StaticSeedHttpClient
    try {
        foreach($member in $Plan.files) {
            foreach($method in @('GET','HEAD')) {
                $request=[Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::new($method),[string]$member.url)
                $request.Headers.AcceptEncoding.ParseAdd('identity')
                $null=$request.Headers.TryAddWithoutValidation('Origin','https://tinyrustlm.com')
                $cts=[Threading.CancellationTokenSource]::new();$cts.CancelAfter(1800000)
                $response=$null;$cancellationClose=$null
                try {
                    $response=$client.SendAsync($request,[Net.Http.HttpCompletionOption]::ResponseHeadersRead,$cts.Token).GetAwaiter().GetResult()
                    $cancellationClose=Register-StaticSeedResponseCancellation -Response $response -CancellationToken $cts.Token
                    $null=Assert-StaticSeedHttpMember -Response $response -Member $member -CancellationToken $cts.Token -HeadersOnly:($method -ceq 'HEAD')
                    $Journal.Add([ordered]@{step='https-member-verified';path=$member.remotePath;method=$method})
                } finally {if($null -ne $cancellationClose){$cancellationClose.Dispose()};if($response){$response.Dispose()};$request.Dispose();$cts.Dispose()}
            }
        }
    } finally {$client.Dispose()}
}

# Promote only after all staged bodies verify. Discovery documents are outside this transaction.
function Invoke-StaticSeedMemberTransaction {
    param([Parameter(Mandatory)]$Plan,[Parameter(Mandatory)][AllowEmptyCollection()][Collections.Generic.List[object]]$Journal)
    Assert-RemoteLeaseOwned
    $state=Get-StaticSeedNamespaceState -Plan $Plan
    if($state -ceq 'absent') {
        New-StaticSeedDirectories -Plan $Plan -Journal $Journal
        # Put the restrictive serving configuration in place before any model member is uploaded.
        $configuration=$script:seedTransferFiles[6]
        $configStage="$($configuration.path).seed-$($script:seedOperationId).tmp"
        $Journal.Add([ordered]@{step='configuration-stage-attempted';path=$configStage})
        $null=Send-RemoteSeedStream -Source $configuration.stream -LogicalPath $configStage
        $null=Assert-RemoteSeedStreamIdentity -LogicalPath $configStage
        Assert-RemoteLeaseOwned
        if((Get-RemoteFileObservation -LogicalPath $configuration.path).State -cne 'absent'){throw 'Seed configuration target changed.'}
        Rename-RemoteFile -LogicalPath $configStage -NewLeafName '.htaccess'
        $null=Assert-RemoteSeedStreamIdentity -LogicalPath $configuration.path
        if((Get-RemoteFileObservation -LogicalPath $configStage).State -cne 'absent'){throw 'Seed configuration promotion left a stage.'}
        for($index=0;$index -lt 6;$index++) {
            $file=$script:seedTransferFiles[$index];$stage="$($file.path).seed-$($script:seedOperationId).tmp"
            $Journal.Add([ordered]@{step='member-stage-attempted';path=$stage})
            $null=Send-RemoteSeedStream -Source $file.stream -LogicalPath $stage
            $null=Assert-RemoteSeedStreamIdentity -LogicalPath $stage
            $Journal.Add([ordered]@{step='member-stage-verified';path=$stage})
        }
        for($index=0;$index -lt 6;$index++) {
            $file=$script:seedTransferFiles[$index];$stage="$($file.path).seed-$($script:seedOperationId).tmp"
            Assert-RemoteLeaseOwned
            if((Get-RemoteFileObservation -LogicalPath $file.path).State -cne 'absent'){throw 'Seed final target changed before promotion.'}
            $Journal.Add([ordered]@{step='member-promotion-attempted';path=$file.path})
            Rename-RemoteFile -LogicalPath $stage -NewLeafName $Plan.files[$index].name
            $null=Assert-RemoteSeedStreamIdentity -LogicalPath $file.path
            if((Get-RemoteFileObservation -LogicalPath $stage).State -cne 'absent'){throw 'Seed promotion left staging residue.'}
        }
    } else {
        foreach($file in $script:seedTransferFiles){$null=Assert-RemoteSeedStreamIdentity -LogicalPath $file.path}
    }
    if((Get-StaticSeedNamespaceState -Plan $Plan) -cne 'complete'){throw 'Seed final closure is incomplete.'}
    Assert-PublicStaticSeedMembers -Plan $Plan -Journal $Journal
    Assert-RemoteLeaseOwned
    return $(if($state -ceq 'absent'){'installed-and-http-verified'}else{'already-present-http-verified'})
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

# Bootstrap only the reviewed identity marker; this grants no website/seed success.
function Invoke-RootSentinelBootstrap {
    param([Parameter(Mandatory)]$AuthorityRecord)

    if (-not $script:sourceAuthorityAdmitted) { throw 'Root bootstrap is forbidden before source authority admission.' }
    $bootstrapAuthority=$AuthorityRecord.Value
    if ($bootstrapAuthority.transport.status -cne 'bound') { throw 'Root bootstrap requires an exact bound transport tuple.' }
    $bootstrapRepository=Get-RepositoryObservation -Authority $bootstrapAuthority
    Assert-CustodyRepositoryIdentity -Authority $bootstrapAuthority -Observed $bootstrapRepository -ExpectedRepositoryRoot $repositoryRoot
    $bootstrapManifest=Read-CustodyJsonFile -Path (Join-Path $repositoryRoot 'deploy/root-bootstrap-manifest.v1.json') -MaximumBytes 16384 -Label 'root bootstrap manifest'
    Assert-CustodyBootstrapManifest -Manifest $bootstrapManifest.Value -Authority $bootstrapAuthority
    $committed=Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $bootstrapRepository.head -RelativePath 'deploy/root-bootstrap-manifest.v1.json' -MaximumBytes 16384
    if ((Get-CustodyByteSha256 $committed) -cne $bootstrapManifest.Sha256) { throw 'Bootstrap manifest differs from the admitted commit.' }
    $sentinelBytes=Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $bootstrapRepository.head -RelativePath $bootstrapAuthority.transport.root_sentinel_source_path -MaximumBytes 8192
    if ($sentinelBytes.Length -ne $bootstrapAuthority.transport.root_sentinel_bytes -or
        (Get-CustodyByteSha256 $sentinelBytes) -cne $bootstrapAuthority.transport.root_sentinel_sha256) { throw 'Bootstrap sentinel bytes differ from the admitted identity.' }

    $httpHandler=[Net.Http.HttpClientHandler]::new();$httpHandler.AllowAutoRedirect=$false
    $http=[Net.Http.HttpClient]::new($httpHandler);$http.Timeout=[TimeSpan]::FromSeconds(60)
    try {
        foreach ($match in $bootstrapManifest.Value.public_matches) {
            $observedBytes=Get-PublicBytes -Client $http -Origin $bootstrapAuthority.origin -RelativePath $match.path -AuditNonce ([Guid]::NewGuid().ToString('N')) -MaximumBytes $match.bytes
            if ($observedBytes.Length -ne $match.bytes -or (Get-CustodyByteSha256 $observedBytes) -cne $match.sha256) { throw 'Bootstrap public correspondence changed.' }
        }
    } finally {$http.Dispose();$httpHandler.Dispose()}
    $afterPublic=Get-RepositoryObservation -Authority $bootstrapAuthority
    Assert-CustodyRepositoryIdentity -Authority $bootstrapAuthority -Observed $afterPublic -ExpectedRepositoryRoot $repositoryRoot
    if ($afterPublic.head -cne $bootstrapRepository.head) { throw 'Bootstrap source changed during public admission.' }
    $script:phase='public-admitted'
    $operation=[Guid]::NewGuid().ToString('N')
    $sentinelPath=[string]$bootstrapAuthority.transport.root_sentinel_path
    $stagePath=Get-RemoteOperationPath -Relative $sentinelPath -Nonce $operation -Kind deploy
    $bootstrapProfile=$null;$held=$false;$leaseAttempted=$false;$leaseReleased=$false;$sentinelVerified=$false;$stageAttempted=$false
    $receipt=[ordered]@{schema='minimodel.root_bootstrap_receipt.v1';operation_id=$operation;publisher_commit=$bootstrapRepository.head;authority_sha256=$AuthorityRecord.Sha256;manifest_sha256=$bootstrapManifest.Sha256;started_utc=[DateTime]::UtcNow.ToString('o');status='failed';sentinel_path=$sentinelPath;sentinel_bytes=$sentinelBytes.Length;sentinel_sha256=$bootstrapAuthority.transport.root_sentinel_sha256;stage_path=$stagePath;sentinel_verified=$false;recovery='not-needed';website_published=$false;seed_published=$false}
    try {
        Import-MiniModelCredentialModule
        $bootstrapProfile=Get-CredentialProfile -Path $FtpExportPath -ProfileName $bootstrapAuthority.transport.profile_name
        $transport=[pscustomobject]@{profile_name=$bootstrapProfile.profile;protocol='ftps-explicit';host=$bootstrapProfile.host;port=$bootstrapProfile.port;username=$bootstrapProfile.user;remote_root=$bootstrapAuthority.transport.remote_root}
        Assert-CustodyTransportAuthority -Authority $bootstrapAuthority -Observed $transport
        $script:ftpHost=$bootstrapProfile.host;$script:ftpPort=$bootstrapProfile.port;$script:remoteRoot=$bootstrapAuthority.transport.remote_root
        $script:credential=[Net.NetworkCredential]::new($bootstrapProfile.user,$bootstrapProfile.password);$bootstrapProfile.password=$null
        $initial=Get-RemoteFileObservation -LogicalPath $sentinelPath
        $initialPaths=@($bootstrapManifest.Value.root_entries)
        if ($initial.State -ceq 'present') {
            Assert-RemoteExactIdentity -LogicalPath $sentinelPath -Bytes $sentinelBytes.Length -Sha256 $bootstrapAuthority.transport.root_sentinel_sha256
            $initialPaths += $sentinelPath
        }
        Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $initialPaths -ObservedTree @{''=@(Get-RemoteDirectoryEntries -LogicalPath '')}
        foreach($match in $bootstrapManifest.Value.public_matches){Assert-RemoteExactIdentity -LogicalPath $match.path -Bytes $match.bytes -Sha256 $match.sha256}
        if ($initial.State -ceq 'present') {
            $sentinelVerified=$true;$receipt.status='already-present-verified'
        } else {
            $script:phase='transport-admitted'
            $leaseAttempted=$true
            $ownedLease=New-RemoteDeploymentLease -Authority $bootstrapAuthority -OperationId $operation -PublisherCommit $bootstrapRepository.head -ManifestSha256 $bootstrapManifest.Sha256 -AuthoritySha256 $AuthorityRecord.Sha256
            $held=$true;$script:phase='leased'
            $leasedPaths=@($bootstrapManifest.Value.root_entries)+@($ownedLease.DirectoryPath)
            Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $leasedPaths -ObservedTree @{''=@(Get-RemoteDirectoryEntries -LogicalPath '')}
            foreach($match in $bootstrapManifest.Value.public_matches){Assert-RemoteExactIdentity -LogicalPath $match.path -Bytes $match.bytes -Sha256 $match.sha256}
            if ((Get-RemoteFileObservation -LogicalPath $sentinelPath).State -cne 'absent' -or
                (Get-RemoteFileObservation -LogicalPath $stagePath).State -cne 'absent') { throw 'Bootstrap target or operation stage is occupied.' }
            Assert-RemoteLeaseOwned
            $stageAttempted=$true
            Send-RemoteFile -Payload $sentinelBytes -LogicalPath $stagePath
            Assert-RemoteExactIdentity -LogicalPath $stagePath -Bytes $sentinelBytes.Length -Sha256 $bootstrapAuthority.transport.root_sentinel_sha256
            Assert-RemoteLeaseOwned
            if ((Get-RemoteFileObservation -LogicalPath $sentinelPath).State -cne 'absent') { throw 'Bootstrap final target changed before promotion.' }
            Rename-RemoteFile -LogicalPath $stagePath -NewLeafName $sentinelPath
            Assert-RemoteExactIdentity -LogicalPath $sentinelPath -Bytes $sentinelBytes.Length -Sha256 $bootstrapAuthority.transport.root_sentinel_sha256
            if ((Get-RemoteFileObservation -LogicalPath $stagePath).State -cne 'absent') { throw 'Bootstrap promotion left staging residue.' }
            $sentinelVerified=$true
            Remove-RemoteDeploymentLease;$held=$false;$leaseReleased=$true
            Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths (@($bootstrapManifest.Value.root_entries)+@($sentinelPath)) -ObservedTree @{''=@(Get-RemoteDirectoryEntries -LogicalPath '')}
            $receipt.status='installed-and-readback-verified'
        }
    } catch {
        $receipt.failure_code=Get-SafeFailureCode -Exception $_.Exception
        $receipt.recovery='no-remote-cleanup-attempted'
        if ($held) {
            try {
                Assert-RemoteLeaseOwned
                if ($stageAttempted) {
                    $stage=Get-RemoteBoundedIdentity -LogicalPath $stagePath -MaximumBytes $sentinelBytes.Length
                    if ($stage.State -ceq 'present') {
                        if ($stage.Bytes -lt 0 -or $stage.Bytes -gt $sentinelBytes.Length -or
                            $stage.Sha256 -cne (Get-PublicationPrefixSha256 -Payload $sentinelBytes -Bytes $stage.Bytes)) { throw 'Bootstrap stage is not an exact owned prefix.' }
                        $null=Remove-RemoteExactFile -LogicalPath $stagePath -Bytes $stage.Bytes -Sha256 $stage.Sha256
                    }
                }
                if ((Get-RemoteFileObservation -LogicalPath $sentinelPath).State -ceq 'present') {
                    Assert-RemoteExactIdentity -LogicalPath $sentinelPath -Bytes $sentinelBytes.Length -Sha256 $bootstrapAuthority.transport.root_sentinel_sha256
                    $sentinelVerified=$true
                }
                Remove-RemoteDeploymentLease;$held=$false;$leaseReleased=$true
                $receipt.recovery='owned-stage-removed-lease-released'
            } catch {$receipt.recovery='retained-for-exact-ownership-review';$receipt.recovery_failure_code=Get-SafeFailureCode -Exception $_.Exception}
        }
    } finally {
        if ($bootstrapProfile) {$bootstrapProfile.password=$null};$script:credential=$null
    }
    $receipt.sentinel_verified=$sentinelVerified
    $receipt.lease_retained_or_creation_ambiguous=$leaseAttempted -and -not $leaseReleased
    $receipt.completed_utc=[DateTime]::UtcNow.ToString('o')
    return $receipt
}

# Capture only the four reviewed probe source files from the admitted commit.
function Get-HostedCapabilityInput {
    param([Parameter(Mandatory)][string]$Commit)
    $bytes=Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $Commit -RelativePath 'deploy/host-capability/source-manifest.v1.json' -MaximumBytes 16384
    $utf8=[Text.UTF8Encoding]::new($false,$true);$manifest=$utf8.GetString($bytes)|ConvertFrom-Json
    $names=@('.htaccess','private/.htaccess','probe-core.php','probe.php')
    if($manifest.schema -cne 'minimodel.host_capability_source_manifest.v1' -or
        $manifest.remoteNamespace -cne '.minimodel-host-capability.v1' -or $manifest.published -ne $false -or @($manifest.members).Count -ne 4) {
        throw 'Hosted capability manifest is not the reviewed source closure.'
    }
    $files=[ordered]@{}
    for($index=0;$index -lt 4;$index++) {
        $member=$manifest.members[$index]
        if($member.path -cne $names[$index] -or $member.byteLength -lt 1 -or $member.byteLength -gt 32768 -or
            ([string]$member.sha256).Length -ne 64 -or $member.sha256 -cnotmatch '^[0-9a-f]{64}$'){throw 'Hosted capability member identity is invalid.'}
        $body=Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $Commit -RelativePath ('deploy/host-capability/'+$member.path) -MaximumBytes $member.byteLength
        if($body.Length -ne $member.byteLength -or (Get-CustodyByteSha256 $body) -cne $member.sha256){throw 'Hosted capability committed source identity differs.'}
        $files[$member.path]=$body
    }
    if($utf8.GetString($files['private/.htaccess']) -cne "Require all denied`n"){throw 'Hosted capability private denial configuration differs.'}
    return [pscustomobject]@{ManifestSha256=(Get-CustodyByteSha256 $bytes);Namespace=$manifest.remoteNamespace;Files=$files}
}

# Validate only the fixed public result shape; never persist unknown response fields or PHP diagnostics.
function Assert-HostedCapabilityResult {
    param([Parameter(Mandatory)][byte[]]$Bytes,[Parameter(Mandatory)][string]$OperationId,[switch]$AllowFailedChecks)
    if($Bytes.Length -lt 1 -or $Bytes.Length -gt 4096){throw 'Hosted capability result exceeds its byte bound.'}
    $value=[Text.UTF8Encoding]::new($false,$true).GetString($Bytes)|ConvertFrom-Json
    $keys=@('schema','nonce','ok','checks')
    if(@($value.PSObject.Properties.Name).Count -ne 4 -or @($value.PSObject.Properties.Name|Where-Object {$_ -cnotin $keys}).Count -gt 0 -or
        $value.schema -cne 'minimodel.host_capability_result.v1' -or $value.nonce -cne $OperationId -or $value.ok -isnot [bool]){throw 'Hosted capability result identity differs.'}
    $checks=@('php','sodium','leaseBinding','privateRead','privateWrite','flush','atomicRename','readback','flock','cleanup')
    if(@($value.checks.PSObject.Properties.Name).Count -ne $checks.Count -or
        @($value.checks.PSObject.Properties.Name|Where-Object {$_ -cnotin $checks}).Count -gt 0){throw 'Hosted capability check closure differs.'}
    foreach($name in $checks){if($value.checks.$name -isnot [bool]){throw 'Hosted capability check is not a boolean.'}}
    $allPassed=@($checks|Where-Object {-not $value.checks.$_}).Count -eq 0
    if($value.ok -ne $allPassed){throw 'Hosted capability result contradicts its check observations.'}
    if(-not $allPassed -and -not $AllowFailedChecks){throw 'Hosted capability prerequisite was not established.'}
    return $value
}

# The probe admits fixed no-query routes only, with bounded bytes and a complete response deadline.
function Get-HostedCapabilityHttpObservation {
    param([Parameter(Mandatory)][string]$RelativePath,[Parameter(Mandatory)][string]$OperationId,[switch]$Deny)
    $namespace='.minimodel-host-capability.v1'
    $allowed=if($Deny){@("$namespace/private/canary.txt","$namespace/private/operation.json","$namespace/probe-core.php","$namespace/probe.php")}else{@("$namespace/probe.json")}
    if($RelativePath -cnotin $allowed -or $OperationId.Length -ne 32 -or $OperationId -cnotmatch '^[0-9a-f]{32}$'){throw 'Hosted capability HTTP route is not admitted.'}
    $client=New-StaticSeedHttpClient;$response=$null;$close=$null
    $request=[Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Get,('https://minimodel.org/'+$RelativePath))
    $request.Headers.CacheControl=[Net.Http.Headers.CacheControlHeaderValue]::new();$request.Headers.CacheControl.NoCache=$true;$request.Headers.CacheControl.NoStore=$true
    $request.Headers.AcceptEncoding.ParseAdd('identity')
    $cts=[Threading.CancellationTokenSource]::new();$cts.CancelAfter(60000)
    try {
        $response=$client.SendAsync($request,[Net.Http.HttpCompletionOption]::ResponseHeadersRead,$cts.Token).GetAwaiter().GetResult()
        $close=Register-StaticSeedResponseCancellation -Response $response -CancellationToken $cts.Token
        if($Deny) {
            if([int]$response.StatusCode -notin @(403,404)){throw 'Hosted private file or implementation route was not denied.'}
            return [pscustomobject]@{status=[int]$response.StatusCode;denied=$true}
        }
        if([int]$response.StatusCode -ne 200 -or $null -eq $response.Content -or $null -eq $response.Content.Headers.ContentType -or
            $response.Content.Headers.ContentType.MediaType -cne 'application/json' -or $null -eq $response.Headers.CacheControl -or
            -not $response.Headers.CacheControl.NoStore -or $response.Content.Headers.ContentEncoding.Count -gt 0 -or
            ($null -ne $response.Content.Headers.ContentLength -and $response.Content.Headers.ContentLength -gt 4096)){throw 'Hosted capability HTTP response is not bounded uncached JSON.'}
        $buffer=[byte[]]::new(4097);$count=0;$stream=$response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        do {
            $read=$stream.ReadAsync($buffer,$count,$buffer.Length-$count,$cts.Token).GetAwaiter().GetResult();$count+=$read
            if($count -gt 4096){throw 'Hosted capability HTTP body exceeds its byte bound.'}
        } while($read -gt 0)
        $body=[byte[]]::new($count);[Array]::Copy($buffer,$body,$count)
        $result=Assert-HostedCapabilityResult -Bytes $body -OperationId $OperationId -AllowFailedChecks
        return [pscustomobject]@{status=200;bytes=$count;sha256=(Get-CustodyByteSha256 $body);result=$result}
    } finally {
        if($null -ne $close){$close.Dispose()};if($response){$response.Dispose()};$cts.Dispose();$request.Dispose();$client.Dispose()
    }
}

# Refuse any pre-existing probe namespace, including partial or case-aliased output.
function Assert-HostedCapabilityNamespaceAbsent {
    $name='.minimodel-host-capability.v1'
    foreach($observation in 1..2) {
        if(Test-ExactListedLeaf -Entries @(Get-RemoteDirectoryEntries -LogicalPath '') -Leaf $name){throw 'Hosted capability namespace already exists; exact recovery is required.'}
    }
}

function New-HostedCapabilityDirectory {
    param([Parameter(Mandatory)][string]$Path)
    if($Path -cnotin @('.minimodel-host-capability.v1','.minimodel-host-capability.v1/private')){throw 'Hosted capability directory is not admitted.'}
    $parts=Get-RemoteParentAndLeaf -LogicalPath $Path
    foreach($observation in 1..2) {
        if(Test-ExactListedLeaf -Entries @(Get-RemoteDirectoryEntries -LogicalPath $parts.Parent) -Leaf $parts.Leaf){throw 'Hosted capability directory changed before creation.'}
    }
    Assert-RemoteLeaseOwned
    $request=New-FtpRequest -LogicalPath $Path -Method ([Net.WebRequestMethods+Ftp]::MakeDirectory) -Mutating
    $response=$request.GetResponse();$response.Dispose()
    if(@(Get-RemoteDirectoryEntries -LogicalPath $Path).Count -ne 0){throw 'New hosted capability directory is not empty.'}
}

# Closure is relative to the owned probe root; unrelated account-root entries are preserved.
function Assert-HostedCapabilityRemoteClosure {
    param([Parameter(Mandatory)][Collections.IDictionary]$Payloads)
    $namespace='.minimodel-host-capability.v1'
    $tree=@{''=@(Get-RemoteDirectoryEntries -LogicalPath $namespace);'private'=@(Get-RemoteDirectoryEntries -LogicalPath ($namespace+'/private'))}
    Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths @($Payloads.Keys) -ObservedTree $tree
    foreach($path in $Payloads.Keys){Assert-RemoteExactIdentity -LogicalPath ($namespace+'/'+$path) -Bytes $Payloads[$path].Length -Sha256 (Get-CustodyByteSha256 $Payloads[$path])}
}

# Cleanup only after the PHP scratch has disappeared and all source/nonsecret inputs still match.
function Remove-HostedCapabilityNamespace {
    param([Parameter(Mandatory)][Collections.IDictionary]$Payloads)
    $namespace='.minimodel-host-capability.v1'
    Assert-RemoteLeaseOwned
    Assert-HostedCapabilityRemoteClosure -Payloads $Payloads
    $paths=@($Payloads.Keys);[Array]::Reverse($paths)
    foreach($path in $paths) {
        Assert-RemoteLeaseOwned
        $null=Remove-RemoteExactFile -LogicalPath ($namespace+'/'+$path) -Bytes $Payloads[$path].Length -Sha256 (Get-CustodyByteSha256 $Payloads[$path])
    }
    foreach($directory in @(($namespace+'/private'),$namespace)) {
        Assert-RemoteLeaseOwned
        if(@(Get-RemoteDirectoryEntries -LogicalPath $directory).Count -ne 0){throw 'Hosted capability cleanup found an unowned entry.'}
        $request=New-FtpRequest -LogicalPath $directory -Method ([Net.WebRequestMethods+Ftp]::RemoveDirectory) -Mutating
        $response=$request.GetResponse();$response.Dispose()
        if(Test-RemoteDirectory -LogicalPath $directory){throw 'Hosted capability directory removal was not verified.'}
    }
    Assert-HostedCapabilityNamespaceAbsent
}

# Install a temporary nonsecret capability probe only under the verified root and exclusive lease.
function Invoke-HostedCapabilityProbe {
    param([Parameter(Mandatory)]$AuthorityRecord)
    if(-not $script:sourceAuthorityAdmitted){throw 'Hosted capability probe requires source admission.'}
    $probeAuthority=$AuthorityRecord.Value
    if($probeAuthority.transport.status -cne 'bound'){throw 'Hosted capability probe requires bound transport.'}
    $repository=Get-RepositoryObservation -Authority $probeAuthority
    Assert-CustodyRepositoryIdentity -Authority $probeAuthority -Observed $repository -ExpectedRepositoryRoot $repositoryRoot
    $inputRecord=Get-HostedCapabilityInput -Commit $repository.head
    $namespace=$inputRecord.Namespace;$operation=[Guid]::NewGuid().ToString('N')
    $receipt=[ordered]@{schema='minimodel.host_capability_receipt.v1';operation_id=$operation;publisher_commit=$repository.head;
        authority_sha256=$AuthorityRecord.Sha256;source_manifest_sha256=$inputRecord.ManifestSha256;started_utc=[DateTime]::UtcNow.ToString('o');
        status='failed';private_denial_verified=$false;capabilities_verified=$false;namespace_cleanup_verified=$false;
        website_published=$false;seed_published=$false;discovery_published=$false;signing_key_published=$false;last_step='source-admitted'}
    $script:phase='input-verified';$profile=$null;$leaseAttempted=$false;$leaseReleased=$false
    try {
        Import-MiniModelCredentialModule
        $profile=Get-CredentialProfile -Path $FtpExportPath -ProfileName $probeAuthority.transport.profile_name
        $observed=[pscustomobject]@{profile_name=$profile.profile;protocol='ftps-explicit';host=$profile.host;port=$profile.port;username=$profile.user;remote_root=$probeAuthority.transport.remote_root}
        Assert-CustodyTransportAuthority -Authority $probeAuthority -Observed $observed
        $script:ftpHost=$profile.host;$script:ftpPort=$profile.port;$script:remoteRoot=$probeAuthority.transport.remote_root
        $script:credential=[Net.NetworkCredential]::new($profile.user,$profile.password);$profile.password=$null
        Assert-RemoteExactIdentity -LogicalPath $probeAuthority.transport.root_sentinel_path -Bytes $probeAuthority.transport.root_sentinel_bytes -Sha256 $probeAuthority.transport.root_sentinel_sha256
        Assert-HostedCapabilityNamespaceAbsent
        $script:phase='transport-admitted';$leaseAttempted=$true;$receipt.last_step='lease-create-attempted'
        $ownedLease=New-RemoteDeploymentLease -Authority $probeAuthority -OperationId $operation -PublisherCommit $repository.head -ManifestSha256 $inputRecord.ManifestSha256 -AuthoritySha256 $AuthorityRecord.Sha256
        $script:phase='leased'
        Assert-RemoteExactIdentity -LogicalPath $probeAuthority.transport.root_sentinel_path -Bytes $probeAuthority.transport.root_sentinel_bytes -Sha256 $probeAuthority.transport.root_sentinel_sha256
        Assert-HostedCapabilityNamespaceAbsent
        $receipt.last_step='namespace-create-attempted';New-HostedCapabilityDirectory -Path $namespace
        $receipt.last_step='private-directory-create-attempted';New-HostedCapabilityDirectory -Path ($namespace+'/private')
        $operationValue=[ordered]@{schema='minimodel.host_capability_operation.v1';operation_id=$operation;lease_owner_sha256=$ownedLease.OwnerSha256}
        $utf8=[Text.UTF8Encoding]::new($false,$true)
        $payloads=[ordered]@{
            'private/.htaccess'=$inputRecord.Files['private/.htaccess'];
            'private/canary.txt'=$utf8.GetBytes(('minimodel-host-capability-canary:'+$operation+"`n"));
            '.htaccess'=$inputRecord.Files['.htaccess'];'probe-core.php'=$inputRecord.Files['probe-core.php'];
            'private/operation.json'=$utf8.GetBytes((($operationValue|ConvertTo-Json -Compress)+"`n"));'probe.php'=$inputRecord.Files['probe.php']
        }
        foreach($path in $payloads.Keys) {
            Assert-RemoteLeaseOwned;$receipt.last_step='upload-attempted:'+ $path
            Send-RemoteFile -Payload $payloads[$path] -LogicalPath ($namespace+'/'+$path)
            Assert-RemoteExactIdentity -LogicalPath ($namespace+'/'+$path) -Bytes $payloads[$path].Length -Sha256 (Get-CustodyByteSha256 $payloads[$path])
            if($path -ceq 'private/canary.txt') {
                $receipt.last_step='initial-private-denial'
                $null=Get-HostedCapabilityHttpObservation -RelativePath ($namespace+'/'+$path) -OperationId $operation -Deny
                $receipt.private_denial_verified=$true
            }
        }
        Assert-HostedCapabilityRemoteClosure -Payloads $payloads
        $receipt.last_step='public-capability-observation'
        $observation=Get-HostedCapabilityHttpObservation -RelativePath ($namespace+'/probe.json') -OperationId $operation
        $receipt.capabilities=$observation.result.checks;$receipt.public_response_sha256=$observation.sha256;$receipt.public_response_bytes=$observation.bytes
        if(-not $observation.result.ok){throw 'Hosted capability prerequisite was not established.'}
        foreach($path in @('private/canary.txt','private/operation.json','probe-core.php','probe.php')) {
            $null=Get-HostedCapabilityHttpObservation -RelativePath ($namespace+'/'+$path) -OperationId $operation -Deny
        }
        Assert-HostedCapabilityRemoteClosure -Payloads $payloads
        $receipt.capabilities_verified=$true;$receipt.last_step='namespace-cleanup-attempted'
        Remove-HostedCapabilityNamespace -Payloads $payloads
        $receipt.namespace_cleanup_verified=$true;$receipt.last_step='lease-release-attempted'
        Remove-RemoteDeploymentLease;$leaseReleased=$true
        $receipt.status='verified-and-removed';$receipt.last_step='complete'
    } catch {
        $receipt.failure_code=Get-SafeFailureCode -Exception $_.Exception
        $receipt.recovery=if($leaseAttempted -and -not $leaseReleased){'retained-for-exact-ownership-review'}else{'no-remote-cleanup-needed'}
    } finally {if($profile){$profile.password=$null};$script:credential=$null}
    $receipt.lease_retained_or_creation_ambiguous=$leaseAttempted -and -not $leaseReleased
    $receipt.completed_utc=[DateTime]::UtcNow.ToString('o')
    return $receipt
}

# Qualified immutable bytes can be installed independently of website/discovery publication.
function Invoke-QualifiedStaticSeedPublication {
    param([Parameter(Mandatory)]$AuthorityRecord,[Parameter(Mandatory)][string]$SourceDirectory)
    if(-not $script:sourceAuthorityAdmitted){throw 'Seed publication is forbidden before source authority admission.'}
    $seedAuthority=$AuthorityRecord.Value
    if($seedAuthority.transport.status -cne 'bound'){throw 'Seed publication requires bound transport authority.'}
    $repository=Get-RepositoryObservation -Authority $seedAuthority
    Assert-CustodyRepositoryIdentity -Authority $seedAuthority -Observed $repository -ExpectedRepositoryRoot $repositoryRoot
    # Real current qualification is required before model bytes or account credentials are opened.
    $inputRecord=Get-StaticSeedPublicationInput -Commit $repository.head
    $sourceSet=$null;$transfer=$null;$profile=$null
    $operation=[Guid]::NewGuid().ToString('N')
    $journal=[Collections.Generic.List[object]]::new()
    $receipt=[ordered]@{schema='minimodel.static_seed_publication_receipt.v1';operation_id=$operation;publisher_commit=$repository.head;
        authority_sha256=$AuthorityRecord.Sha256;composition_id=$inputRecord.Admission.compositionId;piece_set_id=$inputRecord.Admission.pieceSetId;
        qualification_receipt_id=$inputRecord.Admission.qualificationReceiptId;metadata=$inputRecord.Admission.metadata;
        started_utc=[DateTime]::UtcNow.ToString('o');status='failed';seed_published=$false;discovery_published=$false;website_published=$false;
        browser_origin_readback_verified=$false;lease_retained_or_creation_ambiguous=$false;recovery='not-needed'}
    $script:phase='input-verified'
    $leaseAttempted=$false;$leaseReleased=$false
    try {
        $sourceSet=Open-StaticSeedSourceSet -Directory $SourceDirectory -Members $inputRecord.Admission.plan.files
        $transfer=New-StaticSeedTransferSet -InputRecord $inputRecord -SourceSet $sourceSet -OperationId $operation
        $script:seedFilePaths=$transfer.FilePaths;$script:seedTransferFiles=$transfer.Files;$script:seedOperationId=$operation
        $receipt.transfer_binding_sha256=$transfer.BindingSha256
        Import-MiniModelCredentialModule
        $profile=Get-CredentialProfile -Path $FtpExportPath -ProfileName $seedAuthority.transport.profile_name
        $observed=[pscustomobject]@{profile_name=$profile.profile;protocol='ftps-explicit';host=$profile.host;port=$profile.port;username=$profile.user;remote_root=$seedAuthority.transport.remote_root}
        Assert-CustodyTransportAuthority -Authority $seedAuthority -Observed $observed
        $script:ftpHost=$profile.host;$script:ftpPort=$profile.port;$script:remoteRoot=$seedAuthority.transport.remote_root
        $script:credential=[Net.NetworkCredential]::new($profile.user,$profile.password);$profile.password=$null
        Assert-RemoteExactIdentity -LogicalPath $seedAuthority.transport.root_sentinel_path -Bytes $seedAuthority.transport.root_sentinel_bytes -Sha256 $seedAuthority.transport.root_sentinel_sha256
        $null=Get-StaticSeedNamespaceState -Plan $inputRecord.Admission.plan
        $script:phase='transport-admitted';$leaseAttempted=$true
        $null=New-RemoteDeploymentLease -Authority $seedAuthority -OperationId $operation -PublisherCommit $repository.head -ManifestSha256 $transfer.BindingSha256 -AuthoritySha256 $AuthorityRecord.Sha256
        $script:phase='leased'
        Assert-RemoteExactIdentity -LogicalPath $seedAuthority.transport.root_sentinel_path -Bytes $seedAuthority.transport.root_sentinel_bytes -Sha256 $seedAuthority.transport.root_sentinel_sha256
        $result=Invoke-StaticSeedMemberTransaction -Plan $inputRecord.Admission.plan -Journal $journal
        # Long transfers do not silently extend the admitted metadata lifetime.
        $fresh=Get-StaticSeedPublicationInput -Commit $repository.head
        if($fresh.RegistrySha256 -cne $inputRecord.RegistrySha256){throw 'Seed metadata changed after transfer.'}
        Remove-RemoteDeploymentLease;$leaseReleased=$true
        $receipt.status=$result;$receipt.seed_published=$true
    } catch {
        $receipt.failure_code=Get-SafeFailureCode -Exception $_.Exception
        $receipt.recovery=if($leaseAttempted -and -not $leaseReleased){'retained-for-exact-ownership-review'}else{'no-remote-cleanup-needed'}
        # Preserve uncertain upload/rename/lease outcomes. Never adopt or overwrite a partial namespace.
    } finally {
        if($profile){$profile.password=$null};$script:credential=$null
        if($transfer){$transfer.ConfigurationStream.Dispose()}
        if($sourceSet){Close-StaticSeedSourceSet -SourceSet $sourceSet}
        $script:seedFilePaths=@();$script:seedTransferFiles=@();$script:seedOperationId=''
    }
    $receipt.lease_retained_or_creation_ambiguous=$leaseAttempted -and -not $leaseReleased
    $receipt.journal=$journal.ToArray();$receipt.completed_utc=[DateTime]::UtcNow.ToString('o')
    return $receipt
}

$authorityRecord = Read-CustodyJsonFile -Path $AuthorityPath -MaximumBytes 131072 -Label "publication authority"
$authority = $authorityRecord.Value
Assert-CustodyPublicationAuthority -Authority $authority -RepositoryRoot $repositoryRoot -AuthorityPath $authorityRecord.Path
$validatedReportPath = Assert-BoundedReceiptPath -Path $ReportPath
$reportTemporaryPath = "$validatedReportPath.tmp"
if ((Test-Path -LiteralPath $validatedReportPath) -or (Test-Path -LiteralPath $reportTemporaryPath)) {
    throw "Deployment receipt or its staging path is already occupied."
}
if ($BootstrapSentinel) {
    $bootstrapReceipt=Invoke-RootSentinelBootstrap -AuthorityRecord $authorityRecord
    Write-Receipt -Receipt $bootstrapReceipt -Path $validatedReportPath
    if ($bootstrapReceipt.status -ceq 'failed') { throw 'Root sentinel bootstrap failed; inspect its exact receipt before recovery.' }
    Write-Host "ROOT-SENTINEL-VERIFIED; website and seed publication were not performed"
    exit 0
}
if ($PublishSeed) {
    $seedReceipt=Invoke-QualifiedStaticSeedPublication -AuthorityRecord $authorityRecord -SourceDirectory $SeedSourceDirectory
    Write-Receipt -Receipt $seedReceipt -Path $validatedReportPath
    if($seedReceipt.status -ceq 'failed'){throw 'Seed publication failed; inspect its exact receipt and retained lease before recovery.'}
    Write-Host 'SEED-BYTES-HTTP-VERIFIED; website and discovery were not published; browser-origin readback remains separate'
    exit 0
}
if ($ProbeHostedRenewal) {
    $probeReceipt=Invoke-HostedCapabilityProbe -AuthorityRecord $authorityRecord
    Write-Receipt -Receipt $probeReceipt -Path $validatedReportPath
    if($probeReceipt.status -ceq 'failed'){throw 'Hosted capability probe failed; inspect its exact receipt and lease before recovery.'}
    Write-Host 'HOST-CAPABILITY-VERIFIED-AND-REMOVED; no model, discovery metadata, website or signing key was published'
    exit 0
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
