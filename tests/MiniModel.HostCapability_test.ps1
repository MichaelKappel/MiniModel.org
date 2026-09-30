$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Net.Http
$repositoryRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Import-Module (Join-Path $repositoryRoot 'tools/MiniModel.DeploymentCustody.psm1') -Force
Import-Module (Join-Path $repositoryRoot 'tools/MiniModel.StaticSeed.psm1') -Force
$errors=$null;$tokens=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $repositoryRoot 'tools/deploy-site.ps1'),[ref]$tokens,[ref]$errors)
if($errors){throw 'Publisher parse failed.'}
foreach($definition in $ast.EndBlock.Statements|Where-Object {$_ -is [Management.Automation.Language.FunctionDefinitionAst]}){. ([scriptblock]::Create($definition.Extent.Text))}
$script:seedFilePaths=@();$script:seedOperationId=''

function New-TestCapabilityInput {
    $files=[ordered]@{}
    foreach($name in @('.htaccess','private/.htaccess','probe-core.php','probe.php')){$files[$name]=[IO.File]::ReadAllBytes((Join-Path $repositoryRoot ('deploy/host-capability/'+$name)))}
    return [pscustomobject]@{ManifestSha256=('e'*64);Namespace='.minimodel-host-capability.v1';Files=$files}
}
function New-TestCapabilityResultBytes {
    param([string]$OperationId=('1'*32),[string]$BadCheck='')
    $checks=[ordered]@{}
    foreach($name in @('php','sodium','leaseBinding','privateRead','privateWrite','flush','atomicRename','readback','flock','cleanup')){$checks[$name]=$name -cne $BadCheck}
    return ,[Text.Encoding]::UTF8.GetBytes((([ordered]@{schema='minimodel.host_capability_result.v1';nonce=$OperationId;ok=($BadCheck.Length -eq 0);checks=$checks}|ConvertTo-Json -Depth 4 -Compress)+"`n"))
}

Describe 'Actual committed capability manifest ingress' {
    It 'loads the current immutable Git source closure without credentials or transport' {
        $commit=(& git -c ('safe.directory='+$repositoryRoot) -C $repositoryRoot rev-parse HEAD).Trim()
        if($LASTEXITCODE -ne 0){throw 'Fixture could not identify the canonical commit.'}
        $input=Get-HostedCapabilityInput -Commit $commit
        $input.Files.Count | Should Be 4
        $input.ManifestSha256 | Should Match '^[0-9a-f]{64}$'
        $input.Files['probe.php'].Length | Should Be 554
    }
}

Describe 'Committed capability source closure' {
    BeforeEach {
        $script:capabilityBlobs=@{}
        foreach($name in @('source-manifest.v1.json','.htaccess','private/.htaccess','probe-core.php','probe.php')) {
            $path='deploy/host-capability/'+$name;$script:capabilityBlobs[$path]=[IO.File]::ReadAllBytes((Join-Path $repositoryRoot $path))
        }
        Mock Get-CustodyGitBlobBytes {
            param($Commit,$RelativePath,$MaximumBytes)
            if($Commit -cne ('a'*40) -or -not $script:capabilityBlobs.ContainsKey($RelativePath)){throw 'Unregistered fixture Git read.'}
            $bytes=$script:capabilityBlobs[$RelativePath]
            if($bytes.Length -gt $MaximumBytes){throw 'Fixture source exceeds bound.'}
            return ,([byte[]]$bytes.Clone())
        }
    }
    It 'binds the exact four reviewed source files and denial bytes' {
        $input=Get-HostedCapabilityInput -Commit ('a'*40)
        $input.Files.Count | Should Be 4
        $input.Namespace | Should Be '.minimodel-host-capability.v1'
        [Text.Encoding]::UTF8.GetString($input.Files['private/.htaccess']) | Should Be "Require all denied`n"
        Assert-MockCalled Get-CustodyGitBlobBytes -Times 5 -Exactly -Scope It
    }
    It 'rejects source substitution and unapproved manifest routes' {
        $path='deploy/host-capability/probe.php';$script:capabilityBlobs[$path][0]=0
        {Get-HostedCapabilityInput -Commit ('a'*40)} | Should Throw 'committed source identity'
        $manifest=[Text.Encoding]::UTF8.GetString($script:capabilityBlobs['deploy/host-capability/source-manifest.v1.json'])|ConvertFrom-Json
        $manifest.members[0].path='../outside.php'
        $script:capabilityBlobs['deploy/host-capability/source-manifest.v1.json']=[Text.Encoding]::UTF8.GetBytes(($manifest|ConvertTo-Json -Depth 5))
        {Get-HostedCapabilityInput -Commit ('a'*40)} | Should Throw 'member identity'
    }
    It 'rejects alternate namespaces before reading source members' {
        $manifest=[Text.Encoding]::UTF8.GetString($script:capabilityBlobs['deploy/host-capability/source-manifest.v1.json'])|ConvertFrom-Json
        $manifest.remoteNamespace='unreviewed'
        $script:capabilityBlobs['deploy/host-capability/source-manifest.v1.json']=[Text.Encoding]::UTF8.GetBytes(($manifest|ConvertTo-Json -Depth 5))
        {Get-HostedCapabilityInput -Commit ('a'*40)} | Should Throw 'reviewed source closure'
        Assert-MockCalled Get-CustodyGitBlobBytes -Times 1 -Exactly -Scope It
    }
}

Describe 'Closed hosted capability result' {
    It 'admits the exact operation and all ten true prerequisite observations' {
        $result=Assert-HostedCapabilityResult -Bytes (New-TestCapabilityResultBytes) -OperationId ('1'*32)
        $result.ok | Should Be $true
        @($result.checks.PSObject.Properties).Count | Should Be 10
    }
    It 'rejects every failed prerequisite, nonce mismatch and unknown output' {
        foreach($name in @('php','sodium','leaseBinding','privateRead','privateWrite','flush','atomicRename','readback','flock','cleanup')) {
            {Assert-HostedCapabilityResult -Bytes (New-TestCapabilityResultBytes -BadCheck $name) -OperationId ('1'*32)} | Should Throw 'prerequisite'
        }
        {Assert-HostedCapabilityResult -Bytes (New-TestCapabilityResultBytes) -OperationId ('2'*32)} | Should Throw 'identity'
        $value=[Text.Encoding]::UTF8.GetString((New-TestCapabilityResultBytes))|ConvertFrom-Json
        $value|Add-Member NoteProperty unknown 'not-public-evidence'
        {Assert-HostedCapabilityResult -Bytes ([Text.Encoding]::UTF8.GetBytes(($value|ConvertTo-Json -Depth 4))) -OperationId ('1'*32)} | Should Throw 'identity'
    }
    It 'rejects oversize or misleading boolean fields' {
        {Assert-HostedCapabilityResult -Bytes ([byte[]]::new(4097)) -OperationId ('1'*32)} | Should Throw 'byte bound'
        $value=[Text.Encoding]::UTF8.GetString((New-TestCapabilityResultBytes))|ConvertFrom-Json
        $value.checks.sodium='true'
        {Assert-HostedCapabilityResult -Bytes ([Text.Encoding]::UTF8.GetBytes(($value|ConvertTo-Json -Depth 4))) -OperationId ('1'*32)} | Should Throw 'not a boolean'
    }
    It 'can retain only consistent fixed-shape failed checks without treating them as success' {
        $body=New-TestCapabilityResultBytes -BadCheck 'sodium'
        $result=Assert-HostedCapabilityResult -Bytes $body -OperationId ('1'*32) -AllowFailedChecks
        $result.ok | Should Be $false
        $result.checks.sodium | Should Be $false
        $result.checks.php | Should Be $true
        $value=[Text.Encoding]::UTF8.GetString($body)|ConvertFrom-Json
        $value.ok=$true
        {Assert-HostedCapabilityResult -Bytes ([Text.Encoding]::UTF8.GetBytes(($value|ConvertTo-Json -Depth 4))) -OperationId ('1'*32) -AllowFailedChecks} | Should Throw 'contradicts'
    }
}

Describe 'Capability HTTP ingress and lifetime without network' {
    BeforeEach {
        $body=New-TestCapabilityResultBytes
        $script:capStream=[IO.MemoryStream]::new($body,$false)
        $script:capResponse=[Net.Http.HttpResponseMessage]::new([Net.HttpStatusCode]::OK)
        $script:capResponse.Content=[Net.Http.StreamContent]::new($script:capStream)
        $script:capResponse.Content.Headers.ContentType=[Net.Http.Headers.MediaTypeHeaderValue]::new('application/json')
        $script:capResponse.Headers.CacheControl=[Net.Http.Headers.CacheControlHeaderValue]::new();$script:capResponse.Headers.CacheControl.NoStore=$true
        $script:capClient=[pscustomobject]@{Disposed=$false;Sent=0}
        $script:capClient|Add-Member ScriptMethod Dispose {$this.Disposed=$true}
        $script:capClient|Add-Member ScriptMethod SendAsync {
            param($Request,$Completion,$Token)
            $this.Sent++;$script:capHttpUri=$Request.RequestUri.AbsoluteUri
            if($Completion -ne [Net.Http.HttpCompletionOption]::ResponseHeadersRead -or -not $Token.CanBeCanceled){throw 'Fixture HTTP completion contract differs.'}
            $task=[pscustomobject]@{Response=$script:capResponse}
            $task|Add-Member ScriptMethod GetAwaiter {return $this}
            $task|Add-Member ScriptMethod GetResult {return $this.Response}
            return $task
        }
        Mock New-StaticSeedHttpClient {return $script:capClient}
    }
    AfterEach {$script:capResponse.Dispose()}
    It 'reads only the fixed no-query route and disposes its response' {
        $value=Get-HostedCapabilityHttpObservation -RelativePath '.minimodel-host-capability.v1/probe.json' -OperationId ('1'*32)
        $value.result.ok | Should Be $true
        $script:capHttpUri | Should Be 'https://minimodel.org/.minimodel-host-capability.v1/probe.json'
        $script:capStream.CanRead | Should Be $false
        $script:capClient.Disposed | Should Be $true
    }
    It 'requires real denial and does not accept redirects or an exposed canary' {
        {Get-HostedCapabilityHttpObservation -RelativePath '.minimodel-host-capability.v1/private/canary.txt' -OperationId ('1'*32) -Deny} | Should Throw 'not denied'
        $script:capResponse=[Net.Http.HttpResponseMessage]::new([Net.HttpStatusCode]::Forbidden)
        (Get-HostedCapabilityHttpObservation -RelativePath '.minimodel-host-capability.v1/private/canary.txt' -OperationId ('1'*32) -Deny).denied | Should Be $true
    }
    It 'rejects unapproved routes before acquiring a client' {
        foreach($route in @('.minimodel-host-capability.v1/probe.json?caller=1','catalog/compositions.v1.json','../probe.json')) {
            {Get-HostedCapabilityHttpObservation -RelativePath $route -OperationId ('1'*32)} | Should Throw 'not admitted'
        }
        Assert-MockCalled New-StaticSeedHttpClient -Times 0 -Exactly -Scope It
    }
    It 'retains validated negative HTTP observations and still closes the response' {
        $script:capResponse.Content.Dispose()
        $script:capStream=[IO.MemoryStream]::new((New-TestCapabilityResultBytes -BadCheck 'sodium'),$false)
        $script:capResponse.Content=[Net.Http.StreamContent]::new($script:capStream)
        $script:capResponse.Content.Headers.ContentType=[Net.Http.Headers.MediaTypeHeaderValue]::new('application/json')
        $value=Get-HostedCapabilityHttpObservation -RelativePath '.minimodel-host-capability.v1/probe.json' -OperationId ('1'*32)
        $value.result.ok | Should Be $false
        $value.result.checks.sodium | Should Be $false
        $value.sha256.Length | Should Be 64
        $script:capStream.CanRead | Should Be $false
        $script:capClient.Disposed | Should Be $true
    }
    It 'rejects an oversized actual HTTP body even without a declared length' {
        $script:capResponse.Content.Dispose()
        $packed=[IO.MemoryStream]::new()
        $compress=[IO.Compression.GZipStream]::new($packed,[IO.Compression.CompressionMode]::Compress,$true)
        try{$compress.Write([byte[]]::new(4097),0,4097)}finally{$compress.Dispose()}
        $packed.Position=0
        # A non-seekable memory-only stream prevents StreamContent from inferring Content-Length.
        $script:capStream=[IO.Compression.GZipStream]::new($packed,[IO.Compression.CompressionMode]::Decompress)
        $script:capResponse.Content=[Net.Http.StreamContent]::new($script:capStream)
        $script:capResponse.Content.Headers.ContentType=[Net.Http.Headers.MediaTypeHeaderValue]::new('application/json')
        $script:capResponse.Content.Headers.ContentLength | Should Be $null
        {Get-HostedCapabilityHttpObservation -RelativePath '.minimodel-host-capability.v1/probe.json' -OperationId ('1'*32)} | Should Throw 'body exceeds'
        $script:capStream.CanRead | Should Be $false
    }
}

Describe 'Capability publisher ordering and exact namespace cleanup' {
    BeforeEach {
        $script:sourceAuthorityAdmitted=$true;$script:lease=$null;$script:FtpExportPath='not-read'
        $script:capInput=New-TestCapabilityInput;$script:capFailure=''
        $script:capAuthority=[pscustomobject]@{Sha256=('c'*64);Value=(Get-Content (Join-Path $repositoryRoot 'deploy/publication-authority.v1.json') -Raw|ConvertFrom-Json)}
        $script:capFiles=@{};$script:capDirectories=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);$null=$script:capDirectories.Add('')
        $script:capTrace=[Collections.Generic.List[string]]::new()
        Mock Get-RepositoryObservation {return [pscustomobject]@{head=('a'*40)}}
        Mock Assert-CustodyRepositoryIdentity {}
        Mock Get-HostedCapabilityInput {if($script:capFailure -ceq 'source'){throw 'Synthetic source identity failure.'};return $script:capInput}
        Mock Import-MiniModelCredentialModule {}
        Mock Get-CredentialProfile {return [pscustomobject]@{profile='fixture';host='fixture.invalid';port=21;user='fixture';password='not-a-real-key'}}
        Mock Assert-CustodyTransportAuthority {}
        Mock Assert-RemoteLeaseOwned {$script:capTrace.Add('lease')}
        Mock New-RemoteDeploymentLease {
            param($OperationId)
            $script:capTrace.Add('take-lease');$script:lease=[pscustomobject]@{OwnerSha256=('d'*64);OperationId=$OperationId};return $script:lease
        }
        Mock Remove-RemoteDeploymentLease {$script:capTrace.Add('release-lease');$script:lease=$null}
        Mock Get-RemoteDirectoryEntries {
            param($LogicalPath)
            if(-not $script:capDirectories.Contains($LogicalPath)){throw 'Fixture directory absent.'}
            $entries=[Collections.Generic.List[string]]::new()
            if($LogicalPath -ceq ''){$entries.Add('.ftpquota')}
            $prefix=if($LogicalPath.Length){$LogicalPath+'/'}else{''}
            foreach($path in (@($script:capDirectories)+@($script:capFiles.Keys))) {
                if($path.Length -gt $prefix.Length -and $path.StartsWith($prefix,[StringComparison]::Ordinal)) {
                    $suffix=$path.Substring($prefix.Length);if(-not $suffix.Contains('/')){$entries.Add($suffix)}
                }
            }
            return $entries.ToArray()
        }
        Mock New-FtpRequest {
            param($LogicalPath,$Method,$Mutating)
            if(-not $Mutating){throw 'Fixture expects only directory mutations.'}
            if($Method -ceq 'MKD'){$null=$script:capDirectories.Add($LogicalPath)}
            elseif($Method -ceq 'RMD'){$null=$script:capDirectories.Remove($LogicalPath)}
            else{throw 'Unexpected fixture method.'}
            $script:capTrace.Add($Method+':'+$LogicalPath)
            $reply=[pscustomobject]@{};$reply|Add-Member ScriptMethod Dispose {}
            $request=[pscustomobject]@{Reply=$reply};$request|Add-Member ScriptMethod GetResponse {return $this.Reply};return $request
        }
        Mock Send-RemoteFile {
            param($Payload,$LogicalPath)
            if($script:capFiles.ContainsKey($LogicalPath)){throw 'Fixture overwrite forbidden.'}
            $script:capFiles[$LogicalPath]=[byte[]]$Payload.Clone();$script:capTrace.Add('upload:'+$LogicalPath)
        }
        Mock Assert-RemoteExactIdentity {
            param($LogicalPath,$Bytes,$Sha256)
            if($LogicalPath -ceq '.minimodel-publication-root.v1.json'){return}
            if(-not $script:capFiles.ContainsKey($LogicalPath) -or $script:capFiles[$LogicalPath].Length -ne $Bytes -or
                (Get-CustodyByteSha256 $script:capFiles[$LogicalPath]) -cne $Sha256){throw 'Fixture remote identity differs.'}
        }
        Mock Remove-RemoteExactFile {
            param($LogicalPath,$Bytes,$Sha256)
            Assert-RemoteExactIdentity -LogicalPath $LogicalPath -Bytes $Bytes -Sha256 $Sha256
            $script:capFiles.Remove($LogicalPath);$script:capTrace.Add('delete:'+$LogicalPath);return $true
        }
        Mock Get-HostedCapabilityHttpObservation {
            param($RelativePath,$OperationId,$Deny)
            $script:capTrace.Add('http:'+$RelativePath)
            if($Deny){if($script:capFailure -ceq 'denial'){throw 'Synthetic private denial failed.'};return [pscustomobject]@{denied=$true;status=403}}
            if($script:capFailure -ceq 'php'){throw 'Synthetic PHP capability failed.'}
            $badCheck=if($script:capFailure -ceq 'negative'){'sodium'}else{''}
            $body=New-TestCapabilityResultBytes -OperationId $OperationId -BadCheck $badCheck
            return [pscustomobject]@{result=(Assert-HostedCapabilityResult -Bytes $body -OperationId $OperationId -AllowFailedChecks);bytes=$body.Length;sha256=(Get-CustodyByteSha256 $body)}
        }
    }
    AfterEach {$script:lease=$null;$script:credential=$null}
    It 'proves denial before code, verifies all checks, then removes private denial last and releases its lease' {
        $receipt=Invoke-HostedCapabilityProbe -AuthorityRecord $script:capAuthority
        $receipt.status | Should Be 'verified-and-removed'
        $receipt.private_denial_verified | Should Be $true
        $receipt.capabilities_verified | Should Be $true
        $receipt.namespace_cleanup_verified | Should Be $true
        $receipt.signing_key_published | Should Be $false
        $receipt.lease_retained_or_creation_ambiguous | Should Be $false
        $trace=$script:capTrace.ToArray();$root='.minimodel-host-capability.v1/'
        [Array]::IndexOf($trace,('http:'+$root+'private/canary.txt')) | Should BeLessThan ([Array]::IndexOf($trace,('upload:'+$root+'probe-core.php')))
        [Array]::IndexOf($trace,('delete:'+$root+'private/operation.json')) | Should BeLessThan ([Array]::IndexOf($trace,('delete:'+$root+'private/.htaccess')))
        $script:capFiles.Count | Should Be 0
        $script:capDirectories.Count | Should Be 1
        $trace[-1] | Should Be 'release-lease'
        @(Get-RemoteDirectoryEntries -LogicalPath '')[0] | Should Be '.ftpquota'
    }
    It 'stops on failed denial before any PHP or operation input is uploaded and retains the lease' {
        $script:capFailure='denial';$receipt=Invoke-HostedCapabilityProbe -AuthorityRecord $script:capAuthority
        $receipt.status | Should Be 'failed'
        $receipt.lease_retained_or_creation_ambiguous | Should Be $true
        $script:capFiles.Count | Should Be 2
        @($script:capFiles.Keys|Where-Object {$_ -like '*.php' -or $_ -like '*operation.json'}).Count | Should Be 0
        Assert-MockCalled Remove-RemoteDeploymentLease -Times 0 -Exactly -Scope It
    }
    It 'preserves failed capability evidence and never advertises success or cleans unknown PHP output' {
        $script:capFailure='php';$receipt=Invoke-HostedCapabilityProbe -AuthorityRecord $script:capAuthority
        $receipt.status | Should Be 'failed'
        $receipt.capabilities_verified | Should Be $false
        $receipt.lease_retained_or_creation_ambiguous | Should Be $true
        $script:capFiles.Count | Should Be 6
        Assert-MockCalled Remove-RemoteExactFile -Times 0 -Exactly -Scope It
    }
    It 'records exact false prerequisite checks and retains the failed namespace and lease' {
        $script:capFailure='negative';$receipt=Invoke-HostedCapabilityProbe -AuthorityRecord $script:capAuthority
        $receipt.status | Should Be 'failed'
        $receipt.capabilities_verified | Should Be $false
        $receipt.capabilities.sodium | Should Be $false
        $receipt.capabilities.php | Should Be $true
        @($receipt.capabilities.PSObject.Properties).Count | Should Be 10
        $receipt.public_response_sha256.Length | Should Be 64
        $receipt.lease_retained_or_creation_ambiguous | Should Be $true
        $script:capFiles.Count | Should Be 6
        Assert-MockCalled Remove-RemoteExactFile -Times 0 -Exactly -Scope It
        Assert-MockCalled Remove-RemoteDeploymentLease -Times 0 -Exactly -Scope It
    }
    It 'refuses an existing namespace before the lease or any upload' {
        $null=$script:capDirectories.Add('.minimodel-host-capability.v1')
        $receipt=Invoke-HostedCapabilityProbe -AuthorityRecord $script:capAuthority
        $receipt.status | Should Be 'failed'
        $receipt.lease_retained_or_creation_ambiguous | Should Be $false
        Assert-MockCalled New-RemoteDeploymentLease -Times 0 -Exactly -Scope It
        Assert-MockCalled Send-RemoteFile -Times 0 -Exactly -Scope It
    }
    It 'rejects unbound or failed source admission before reading credentials' {
        $script:capFailure='source'
        {Invoke-HostedCapabilityProbe -AuthorityRecord $script:capAuthority} | Should Throw 'source identity'
        $script:capAuthority.Value.transport.status='unbound'
        {Invoke-HostedCapabilityProbe -AuthorityRecord $script:capAuthority} | Should Throw 'bound transport'
        Assert-MockCalled Get-CredentialProfile -Times 0 -Exactly -Scope It
    }
}
