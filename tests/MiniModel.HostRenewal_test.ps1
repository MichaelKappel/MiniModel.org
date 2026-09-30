$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$repositoryRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$script:NodeExecutablePath='C:\Program Files\nodejs\node.exe'
Import-Module (Join-Path $repositoryRoot 'tools/MiniModel.DeploymentCustody.psm1') -Force
Import-Module (Join-Path $repositoryRoot 'tools/MiniModel.StaticSeed.psm1') -Force
$errors=$null;$tokens=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $repositoryRoot 'tools/deploy-site.ps1'),[ref]$tokens,[ref]$errors)
if($errors){throw 'Publisher parse failed.'}
foreach($definition in $ast.EndBlock.Statements|Where-Object {$_ -is [Management.Automation.Language.FunctionDefinitionAst]}) { . ([scriptblock]::Create($definition.Extent.Text)) }
$script:realRenewalNode=${function:Invoke-VerifiedNode}

function New-CurrentRenewalSourceFiles {
    $files=@{}
    foreach($name in @('source-manifest.v1.json','.htaccess','private/.htaccess','renewal-core.php','metadata.php')) {
        $relative='deploy/host-renewal/'+$name
        $files[$relative]=[IO.File]::ReadAllBytes((Join-Path $repositoryRoot $relative))
    }
    return $files
}

Describe 'Actual current renewal Git and pinned child boundary' {
    It 'captures the current committed four-file runtime and exact private denial bytes' {
        $commit=(& git -c ('safe.directory='+$repositoryRoot) -C $repositoryRoot rev-parse HEAD).Trim()
        if($LASTEXITCODE -ne 0){throw 'Cannot identify canonical Git commit.'}
        $inputRecord=Get-HostedRenewalSourceInput -Commit $commit
        $inputRecord.Namespace | Should Be '.minimodel-metadata-renewal.v1'
        $inputRecord.Files.Count | Should Be 4
        [Text.Encoding]::UTF8.GetString($inputRecord.Files['private/.htaccess']) | Should Be "Require all denied`n"
    }
    It 'reaches the real pinned producer and preserves its actual malformed-envelope rejection' {
        $result=Invoke-VerifiedNode -Tool renewal-input -InputBytes ([Text.Encoding]::UTF8.GetBytes("{}`n"))
        $result.ExitCode | Should Be 1
        $result.Stdout | Should Be ''
        $result.Stderr.Trim() | Should Be 'host-renewal-envelope-fields'
    }
    It 'rejects arguments and missing, empty or oversized input before child execution' {
        {Invoke-VerifiedNode -Tool renewal-input} | Should Throw 'Invalid renewal-input input'
        {Invoke-VerifiedNode -Tool renewal-input -InputBytes ([byte[]]::new(0))} | Should Throw 'Invalid renewal-input input'
        {Invoke-VerifiedNode -Tool renewal-input -InputBytes ([byte[]]::new(6291457))} | Should Throw 'Invalid renewal-input input'
        {Invoke-VerifiedNode -Tool renewal-input -Arguments @('--key') -InputBytes ([byte[]](1))} | Should Throw 'Invalid renewal-input input'
    }
}

Describe 'Renewal source-manifest byte custody' {
    BeforeEach {
        $script:renewalBlobs=New-CurrentRenewalSourceFiles
        Mock Get-CustodyGitBlobBytes {
            param($Commit,$RelativePath,$MaximumBytes)
            if($Commit -cne ('a'*40) -or -not $script:renewalBlobs.ContainsKey($RelativePath)){throw 'Unregistered fixture Git read.'}
            $bytes=$script:renewalBlobs[$RelativePath]
            if($bytes.Length -gt $MaximumBytes){throw 'Fixture source exceeds bound.'}
            return ,([byte[]]$bytes.Clone())
        }
    }
    It 'rejects changed runtime bytes before returning a deployment closure' {
        $script:renewalBlobs['deploy/host-renewal/metadata.php'][0]=0
        {Get-HostedRenewalSourceInput -Commit ('a'*40)} | Should Throw 'committed source identity'
    }
    It 'refuses alternate namespaces, source routes and oversized members' {
        foreach($mode in @('namespace','route','size')) {
            $script:renewalBlobs=New-CurrentRenewalSourceFiles
            $manifest=[Text.Encoding]::UTF8.GetString($script:renewalBlobs['deploy/host-renewal/source-manifest.v1.json'])|ConvertFrom-Json
            if($mode -ceq 'namespace'){$manifest.remoteNamespace='other'}
            if($mode -ceq 'route'){$manifest.members[0].path='../outside'}
            if($mode -ceq 'size'){$manifest.members[0].byteLength=32769}
            $script:renewalBlobs['deploy/host-renewal/source-manifest.v1.json']=[Text.Encoding]::UTF8.GetBytes(($manifest|ConvertTo-Json -Depth 6))
            {Get-HostedRenewalSourceInput -Commit ('a'*40)} | Should Throw 'Hosted renewal'
        }
    }
}

Describe 'Publisher renewal input consumes actual canonical qualification and producer' {
    BeforeEach {
        $script:renewalBlobs=New-CurrentRenewalSourceFiles
        $code='const {inputs}=require("./tests/helpers/static-seed-publication-fixture.js").createAdmittedSeedFixture({renewal:true});process.stdout.write(JSON.stringify(Object.fromEntries(Object.entries(inputs).map(([k,v])=>[k,v.toString("base64")]))));'
        $encoded=($code | & $script:NodeExecutablePath)|ConvertFrom-Json
        if($LASTEXITCODE -ne 0){throw 'Synthetic admission fixture failed.'}
        $bodies=@{}
        foreach($property in $encoded.PSObject.Properties){$bodies[$property.Name]=[Convert]::FromBase64String($property.Value)}
        $entry=([Text.Encoding]::UTF8.GetString($bodies.catalog)|ConvertFrom-Json).entries[0]
        $script:renewalBlobs['deploy/static-seed-namespace.v1.json']=[Text.Encoding]::UTF8.GetBytes((([ordered]@{schema='minimodel.static-seed-namespace/1';descriptorBase64=$encoded.descriptor}|ConvertTo-Json -Compress)+"`n"))
        $script:renewalBlobs['catalog/compositions.v1.json']=$bodies.catalog
        $script:renewalBlobs['catalog/peer-announcements.v1.json']=$bodies.peerFeed
        $script:renewalBlobs[$entry.browser_admission.route.Substring(1)]=$bodies.admission
        $script:renewalBlobs[$entry.qualification.receipt_route.Substring(1)]=$bodies.qualificationReceipt
        $script:licensePath=$entry.license.route.Substring(1)
        $script:renewalBlobs[$script:licensePath]=[Text.Encoding]::UTF8.GetBytes("Synthetic fixture license only`n")
        $script:renewalAuthority=[pscustomobject]@{Sha256=('d'*64);Value=[pscustomobject]@{candidate=[pscustomobject]@{release_id='synthetic-test-never-live';source_commit=('b'*40)}}}
        $script:renewalResultMutation=''
        Mock Get-CustodyGitBlobBytes {
            param($Commit,$RelativePath,$MaximumBytes)
            if($Commit -cne ('a'*40) -or -not $script:renewalBlobs.ContainsKey($RelativePath)){throw 'Unregistered fixture Git read.'}
            $bytes=$script:renewalBlobs[$RelativePath]
            if($bytes.Length -gt $MaximumBytes){throw 'Fixture source exceeds bound.'}
            return ,([byte[]]$bytes.Clone())
        }
        Mock Invoke-VerifiedNode {
            param($Tool,$InputBytes)
            $result=& $script:realRenewalNode -Tool $Tool -InputBytes $InputBytes
            if($Tool -ceq 'renewal-input' -and $script:renewalResultMutation.Length -gt 0) {
                $body=$result.Stdout|ConvertFrom-Json
                if($script:renewalResultMutation -ceq 'consumed'){$body.metadata.license.sha256='0'*64}
                if($script:renewalResultMutation -ceq 'private'){$body.configuration.text+=' '}
                if($script:renewalResultMutation -ceq 'permission'){$body.publicationAllowed=$true}
                $result.Stdout=$body|ConvertTo-Json -Depth 15 -Compress
            }
            return $result
        }
    }
    It 'binds both actual child validations, Git license bytes, runtime manifest and distinct commits' {
        $inputRecord=Get-HostedRenewalPublicationInput -Commit ('a'*40) -AuthorityRecord $script:renewalAuthority
        $inputRecord.Prepared.publicationAllowed | Should Be $false
        $inputRecord.Prepared.sourceReferences.Count | Should Be 9
        $inputRecord.ReleaseOwner.source_commit | Should Be ('b'*40)
        $inputRecord.ReleaseOwner.publisher_commit | Should Be ('a'*40)
        $inputRecord.ReleaseOwner.manifest_sha256 | Should Be $inputRecord.Runtime.ManifestSha256
        $inputRecord.LicensePath | Should Be $script:licensePath
        $inputRecord.PrivateFiles.Count | Should Be 2
        $inputRecord.Prepared.metadata.license.sha256 | Should Be (Get-CustodyByteSha256 $script:renewalBlobs[$script:licensePath])
        Assert-MockCalled Invoke-VerifiedNode -Times 1 -Exactly -Scope It -ParameterFilter {$Tool -ceq 'seed-admission'}
        Assert-MockCalled Invoke-VerifiedNode -Times 1 -Exactly -Scope It -ParameterFilter {$Tool -ceq 'renewal-input'}
        $script:renewalBlobs[$script:licensePath][0]=0
        [Text.Encoding]::UTF8.GetString($inputRecord.Inputs.license) | Should Be "Synthetic fixture license only`n"
    }
    It 'does not bypass missing source qualification to reach renewal or credentials' {
        $script:renewalBlobs['deploy/static-seed-namespace.v1.json']=[Text.Encoding]::UTF8.GetBytes("{`"schema`":`"minimodel.static-seed-namespace/1`",`"descriptorBase64`":null}`n")
        {Get-HostedRenewalPublicationInput -Commit ('a'*40) -AuthorityRecord $script:renewalAuthority} | Should Throw 'No exact static seed descriptor'
        Assert-MockCalled Invoke-VerifiedNode -Times 0 -Exactly -Scope It
    }
    It 'rejects substituted consumed/private byte identities and a fabricated publication permission' {
        foreach($mode in @('consumed','private','permission')) {
            $script:renewalResultMutation=$mode
            {Get-HostedRenewalPublicationInput -Commit ('a'*40) -AuthorityRecord $script:renewalAuthority} | Should Throw 'Hosted renewal'
        }
    }
    It 'requires the reviewed license to be present in the same committed source closure' {
        $script:renewalBlobs.Remove($script:licensePath)
        {Get-HostedRenewalPublicationInput -Commit ('a'*40) -AuthorityRecord $script:renewalAuthority} | Should Throw 'Unregistered fixture Git read'
        Assert-MockCalled Invoke-VerifiedNode -Times 0 -Exactly -Scope It -ParameterFilter {$Tool -ceq 'renewal-input'}
    }
}
