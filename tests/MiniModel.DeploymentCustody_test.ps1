# Exercise canonical code with publication payloads and transport observations held in memory.
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Import-Module (Join-Path $repositoryRoot 'tools/MiniModel.DeploymentCustody.psm1') -Force

InModuleScope MiniModel.DeploymentCustody {
    Describe 'Bounded canonical publication readers' {
        It 'reads current Git bytes without changing their array type or writing a source copy' {
            $root = 'E:\Source\Rust\MiniModel.org'
            $head = & git -c safe.directory=E:/Source/Rust/MiniModel.org -C $root rev-parse HEAD
            $bytes = Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit $head -RelativePath 'catalog/compositions.v1.json' -MaximumBytes 65536
            ($bytes -is [byte[]]) | Should Be $true
            $working = Read-CustodyPublicationSourceBytes -RepositoryRoot $root -RelativePath 'catalog/compositions.v1.json' -MaximumBytes $bytes.Length
            Get-CustodyByteSha256 -Bytes $bytes | Should Be (Get-CustodyByteSha256 -Bytes $working)
            { Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit $head -RelativePath 'catalog/compositions.v1.json' -MaximumBytes 0 } | Should Throw 'bounded process contract'
            { Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit ('0'*40) -RelativePath 'absent.txt' } | Should Throw 'bounded process contract'
        }

        It 'rejects alternate roots, revision expressions, path escapes and oversized reads' {
            $root = 'E:\Source\Rust\MiniModel.org'
            { Get-CustodyGitBlobBytes -RepositoryRoot 'E:\Source\Rust\MiRust.com' -Commit ('a'*40) -RelativePath 'index.html' } | Should Throw 'canonical MiniModel'
            { Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit 'HEAD' -RelativePath 'index.html' } | Should Throw 'exact object identity'
            { Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit ('a'*40) -RelativePath '../escape' } | Should Throw 'Unsafe relative path'
            { Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit ('a'*40) -RelativePath 'index.html' -MaximumBytes 16777217 } | Should Throw
            { Read-CustodyPublicationSourceBytes -RepositoryRoot $root -RelativePath 'catalog/compositions.v1.json' -MaximumBytes 0 } | Should Throw 'bounded regular file'
            { Read-CustodyPublicationSourceBytes -RepositoryRoot $root -RelativePath 'catalog' -MaximumBytes 65536 } | Should Throw 'bounded regular file'
        }

        It 'authenticates a genuinely empty payload' {
            Get-CustodyByteSha256 -Bytes ([byte[]]::new(0)) | Should Be 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
        }

        It 'admits the current catalog from real Git and rejects the superseded publication catalog' {
            $root='E:\Source\Rust\MiniModel.org'
            $head=& git -c safe.directory=E:/Source/Rust/MiniModel.org -C $root rev-parse HEAD
            $tree=& git -c safe.directory=E:/Source/Rust/MiniModel.org -C $root rev-parse 'HEAD^{tree}'
            $bytes=Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit $head -RelativePath 'catalog/compositions.v1.json'
            $manifest=[pscustomobject]@{source_commit=$head;source_tree=$tree;materializations=@();files=@([pscustomobject]@{path='catalog/compositions.v1.json';bytes=$bytes.Length;sha256=(Get-CustodyByteSha256 -Bytes $bytes)})}
            $authority=Get-Content -LiteralPath (Join-Path $root 'deploy/publication-authority.v1.json') -Raw | ConvertFrom-Json
            $record=New-CustodyPublicationInput -Phase 'public-admitted' -RepositoryRoot $root -Manifest $manifest -ManifestSha256 ('e'*64) -OperationId ('1'*32) -Authority $authority
            Assert-CustodyPublicationInput -InputRecord $record -InputSha256 $record.BindingSha256 -ManifestSha256 ('e'*64) -OperationId ('1'*32)
            $oldBytes=Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit '672991ba646cde761ba6a361fc20b5a24aaabbe0' -RelativePath 'deploy/site-deployment-manifest.v1.json'
            $old=[Text.Encoding]::UTF8.GetString($oldBytes) | ConvertFrom-Json
            $old.files=@($old.files | Where-Object { $_.path -ceq 'catalog/compositions.v1.json' })
            $old.materializations=@()
            $old.files.Count | Should Be 1
            ($bytes.Length -gt $old.files[0].bytes) | Should Be $true
            { New-CustodyPublicationInput -Phase 'public-admitted' -RepositoryRoot $root -Manifest $old -ManifestSha256 ('e'*64) -OperationId ('1'*32) -Authority $authority } | Should Throw 'bounded regular file'
        }
    }

    function New-TestPublicationInput {
        New-CustodyPublicationInput -Phase 'public-admitted' -RepositoryRoot 'E:\Source\Rust\MiniModel.org' `
            -Manifest $script:testManifest -ManifestSha256 ('e'*64) -OperationId ('1'*32) -Authority $script:testAuthority
    }
    function Assert-TestPublicationInput {
        param($Record)
        Assert-CustodyPublicationInput -InputRecord $Record -InputSha256 $script:testInputHash -ManifestSha256 ('e'*64) -OperationId ('1'*32)
    }

    Describe 'Publication input custody without disk snapshots' {
        BeforeEach {
            $script:testSource = [Text.Encoding]::UTF8.GetBytes('publication bytes')
            $script:testWorking = [byte[]]$script:testSource.Clone()
            $script:testAuthority = Get-Content -LiteralPath 'E:\Source\Rust\MiniModel.org\deploy\publication-authority.v1.json' -Raw | ConvertFrom-Json
            $script:testManifest = [pscustomobject]@{
                source_commit=('a'*40);source_tree=('b'*40);materializations=@()
                files=@([pscustomobject]@{path='index.html';bytes=$script:testSource.Length;sha256=(Get-CustodyByteSha256 -Bytes $script:testSource)})
            }
            Mock Get-CustodyGitBlobBytes { return ,$script:testSource }
            Mock Read-CustodyPublicationSourceBytes { return ,$script:testWorking }
        }

        It 'binds independently owned bytes to one operation and manifest' {
            $record = New-TestPublicationInput
            $script:testInputHash = $record.BindingSha256
            Assert-TestPublicationInput $record
            $record.Payloads.Count | Should Be 1
            [Object]::ReferenceEquals($record.Payloads['index.html'],$script:testSource) | Should Be $false
            $script:testSource[0] = 0
            $script:testWorking[0] = 0
            Assert-TestPublicationInput $record
            $result = Get-CustodyPublicationFileBytes -InputRecord $record -InputSha256 $record.BindingSha256 -ManifestSha256 ('e'*64) -OperationId ('1'*32) -RelativePath 'index.html'
            ($result -is [byte[]]) | Should Be $true
            [Text.Encoding]::UTF8.GetString($result) | Should Be 'publication bytes'
            { Get-CustodyPublicationFileBytes -InputRecord $record -InputSha256 $record.BindingSha256 -ManifestSha256 ('e'*64) -OperationId ('1'*32) -RelativePath 'missing.html' } | Should Throw 'member is absent'
        }

        It 'rejects same-length committed source and working checkout drift' {
            $script:testSource[0] = 0
            { New-TestPublicationInput } | Should Throw 'Committed publication source identity drifted'
            $script:testSource = [byte[]]$script:testWorking.Clone()
            $script:testWorking[0] = 0
            { New-TestPublicationInput } | Should Throw 'Canonical working source differs'
        }

        It 'rejects payload substitution after input creation' {
            $record = New-TestPublicationInput
            $script:testInputHash = $record.BindingSha256
            $record.Payloads['index.html'][0] = 0
            { Assert-TestPublicationInput $record } | Should Throw 'bytes drifted'
        }

        It 'rejects operation, manifest, binding hash and member identity substitutions' {
            $record = New-TestPublicationInput
            $script:testInputHash = $record.BindingSha256
            { Assert-CustodyPublicationInput -InputRecord $record -InputSha256 $record.BindingSha256 -ManifestSha256 ('d'*64) -OperationId ('1'*32) } | Should Throw 'binding drifted'
            { Assert-CustodyPublicationInput -InputRecord $record -InputSha256 $record.BindingSha256 -ManifestSha256 ('e'*64) -OperationId ('2'*32) } | Should Throw 'binding drifted'
            $record.BindingSha256 = '0'*64
            { Assert-TestPublicationInput $record } | Should Throw 'binding drifted'
            $record.BindingSha256 = $script:testInputHash
            $record.Binding.files[0].path = 'substituted.html'
            { Assert-TestPublicationInput $record } | Should Throw 'binding drifted'
        }

        It 'rejects premature creation and malformed operation identities before reading blobs' {
            { New-CustodyPublicationInput -Phase 'read-only' -RepositoryRoot 'E:\Source\Rust\MiniModel.org' -Manifest $script:testManifest -ManifestSha256 ('e'*64) -OperationId ('1'*32) -Authority $script:testAuthority } | Should Throw 'forbidden'
            $script:testManifest.source_commit = 'HEAD'
            { New-TestPublicationInput } | Should Throw 'identity is invalid'
            Assert-MockCalled Get-CustodyGitBlobBytes -Times 0 -Exactly -Scope It
        }

        It 'rejects empty, oversized, duplicate and case-colliding closures before reading blobs' {
            $row = $script:testManifest.files[0]
            foreach ($files in @(@(),@($row,$row),@($row,[pscustomobject]@{path='INDEX.html';bytes=$row.bytes;sha256=$row.sha256}))) {
                $script:testManifest.files = $files
                { New-TestPublicationInput } | Should Throw
            }
            $script:testManifest.files = @(1..501 | ForEach-Object { $row })
            { New-TestPublicationInput } | Should Throw 'closure count'
            $script:testManifest.files = @($row)
            $row.bytes = 16777217
            { New-TestPublicationInput } | Should Throw 'member identity'
            $script:testManifest.files = @(1..5 | ForEach-Object { [pscustomobject]@{path="$_.txt";bytes=16777216;sha256=('a'*64)} })
            { New-TestPublicationInput } | Should Throw 'total byte budget'
            Assert-MockCalled Get-CustodyGitBlobBytes -Times 0 -Exactly -Scope It
        }

        It 'rejects unused, duplicate and unsupported materializations' {
            $row = [pscustomobject]@{path='missing.json';kind='prepared-release-to-published-v1';source_bytes=1;source_sha256=('a'*64)}
            $script:testManifest.materializations = @($row)
            { New-TestPublicationInput } | Should Throw 'unused materialization'
            $script:testManifest.materializations = @($row,$row)
            { New-TestPublicationInput } | Should Throw 'materialization identity'
            $row.kind = 'unknown'
            $script:testManifest.materializations = @($row)
            { New-TestPublicationInput } | Should Throw 'materialization identity'
        }

        It 'materializes the actual prepared release bytes solely in memory' {
            $script:testSource = [IO.File]::ReadAllBytes('E:\Source\Rust\MiniModel.org\.well-known\site-release.json')
            $script:testWorking = [byte[]]$script:testSource.Clone()
            $published = [byte[]](ConvertTo-CustodyPublishedArtifactBytes -Kind 'prepared-release-to-published-v1' -SourceBytes $script:testSource -Authority $script:testAuthority)
            $script:testManifest.files = @([pscustomobject]@{path='.well-known/site-release.json';bytes=$published.Length;sha256=(Get-CustodyByteSha256 -Bytes $published)})
            $script:testManifest.materializations = @([pscustomobject]@{path='.well-known/site-release.json';kind='prepared-release-to-published-v1';source_bytes=$script:testSource.Length;source_sha256=(Get-CustodyByteSha256 -Bytes $script:testSource)})
            $record = New-TestPublicationInput
            $script:testInputHash = $record.BindingSha256
            Assert-TestPublicationInput $record
            ([Text.Encoding]::UTF8.GetString($record.Payloads['.well-known/site-release.json']) | ConvertFrom-Json).status | Should Be 'published'
            ([Text.Encoding]::UTF8.GetString($script:testWorking) | ConvertFrom-Json).status | Should Be 'prepared'
            $script:testManifest.files[0].sha256 = '0'*64
            { New-TestPublicationInput } | Should Throw 'Materialized publication identity drifted'
        }

        It 'preserves an empty file as a typed zero-length member' {
            $script:testSource = [byte[]]::new(0)
            $script:testWorking = [byte[]]::new(0)
            $script:testManifest.files[0].bytes = 0
            $script:testManifest.files[0].sha256 = Get-CustodyByteSha256 -Bytes $script:testSource
            $record = New-TestPublicationInput
            $script:testInputHash = $record.BindingSha256
            Assert-TestPublicationInput $record
            $record.Payloads['index.html'].Length | Should Be 0
        }
    }
}

# Import only the actual canonical function definitions. Do not run publisher admission or its entrypoint.
$errors=$null; $tokens=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $repositoryRoot 'tools/deploy-site.ps1'),[ref]$tokens,[ref]$errors)
if ($errors) { throw 'Publisher parse failed.' }
foreach ($definition in $ast.EndBlock.Statements | Where-Object { $_ -is [Management.Automation.Language.FunctionDefinitionAst] }) {
    . ([scriptblock]::Create($definition.Extent.Text))
}

Describe 'Shared FTP-export credentials without remote operations' {
    BeforeEach {
        $script:credentialOriginalRoot = $repositoryRoot
        $script:sourceAuthorityAdmitted = $true
        Import-MiniModelCredentialModule
        $script:credentialFixturePath = Join-Path $env:TEMP ('credential-' + [Guid]::NewGuid().ToString('N') + '.txt')
        $script:credentialText = "FTP Username: fixture@MiniModel.org`nFTP server: publish.example`nFTP & explicit FTPS port: 21`nUsername: fixture`nPassword: synthetic:fixture=value`n"
        [IO.File]::WriteAllText($script:credentialFixturePath, $script:credentialText, [Text.UTF8Encoding]::new($false))
        # A synthetic source boundary inside the operation-owned directory; no source is created.
        $repositoryRoot = Join-Path $env:TEMP 'unwritten-source-boundary'
    }
    AfterEach {
        $repositoryRoot = $script:credentialOriginalRoot
        [IO.File]::Delete($script:credentialFixturePath)
    }

    It 'selects the exact MiniModel account and preserves supported password punctuation' {
        $twoSites = $script:credentialText.Replace('MiniModel.org', 'TinyRustLM.com') + $script:credentialText
        [IO.File]::WriteAllText($script:credentialFixturePath, $twoSites)
        $profile = Get-CredentialProfile -Path $script:credentialFixturePath -ProfileName 'MiniModel.org'
        $profile.profile | Should Be 'MiniModel.org'
        $profile.host | Should Be 'publish.example'
        $profile.user | Should Be 'fixture@MiniModel.org'
        $profile.port | Should Be 21
        $profile.password | Should Be 'synthetic:fixture=value'
        $profile.password = $null
        (Get-Command Get-DeploymentCredentialProfile).ModuleName | Should Be 'Deployment.Credentials'
    }

    It 'rejects missing source admission before reading an absent export or importing code' {
        $script:sourceAuthorityAdmitted = $false
        { Get-CredentialProfile -Path 'absent' -ProfileName 'MiniModel.org' } | Should Throw 'source-authority gate'
        { Import-MiniModelCredentialModule } | Should Throw 'source-authority gate'
    }

    It 'rejects duplicate accounts and obsolete XML without exposing credential fields' {
        foreach ($text in @(($script:credentialText + $script:credentialText), '<FileZilla3><Servers /></FileZilla3>')) {
            [IO.File]::WriteAllText($script:credentialFixturePath, $text)
            $caught = $null
            try { Get-CredentialProfile -Path $script:credentialFixturePath -ProfileName 'MiniModel.org' }
            catch { $caught = $_.Exception.Message }
            $caught | Should Not BeNullOrEmpty
            $caught | Should Not Match 'synthetic:fixture|publish.example|fixture@'
        }
    }

    It 'rejects the selected account against a different authorized transport tuple before FTP use' {
        $profile = Get-CredentialProfile -Path $script:credentialFixturePath -ProfileName 'MiniModel.org'
        $bound = Get-Content -LiteralPath (Join-Path $script:credentialOriginalRoot 'deploy/publication-authority.v1.json') -Raw | ConvertFrom-Json
        $bound.transport.status = 'bound'; $bound.transport.host = 'different.example'
        $bound.transport.port = 21; $bound.transport.remote_root = '.'
        $bound.transport.username_sha256 = Get-CustodyByteSha256 -Bytes ([Text.Encoding]::UTF8.GetBytes($profile.user))
        $observed = [pscustomobject]@{profile_name=$profile.profile;protocol='ftps-explicit';host=$profile.host;port=$profile.port;username=$profile.user;remote_root='.'}
        { Assert-CustodyTransportAuthority -Authority $bound -Observed $observed } | Should Throw 'immutable publication transport tuple'
        $profile.password = $null
    }

    It 'checks exact module bytes and rejects wrong length, same-size changed bytes and unreadable streams' {
        foreach ($length in @(0, 6524, 6525, 6526)) {
            $stream = [IO.MemoryStream]::new([byte[]]::new($length), $false)
            try { { Assert-MiniModelCredentialModuleIdentity -Stream $stream } | Should Throw 'identity drifted' }
            finally { $stream.Dispose() }
        }
        $closed = [IO.MemoryStream]::new(); $closed.Dispose()
        { Assert-MiniModelCredentialModuleIdentity -Stream $closed } | Should Throw 'identity drifted'
    }

    It 'rejects unsafe module ancestry before importing the shared module' {
        $repositoryRoot = $script:credentialOriginalRoot
        Mock Test-BootstrapPathHasReparsePoint { return $true }
        { Import-MiniModelCredentialModule } | Should Throw 'ancestry is unsafe'
    }
}

Describe 'Canonical publisher memory transfer and partial rollback' {
    BeforeEach {
        $script:chunks=[Collections.Generic.List[object]]::new()
        $script:transferred=0
        $script:present=$false
        $script:offsetDrift=$false
        $script:writeFailure=$false
        Mock Get-RemoteFileObservation {
            if (-not $script:present) { return [pscustomobject]@{State='absent';Size=$null} }
            return [pscustomobject]@{State='present';Size=($script:transferred + [int]$script:offsetDrift)}
        }
        Mock Write-RemoteChunk {
            param($LogicalPath,$Bytes,$Append)
            if ($script:writeFailure) { throw 'transport stopped' }
            $script:chunks.Add([pscustomobject]@{Bytes=[byte[]]$Bytes.Clone();Append=$Append})
            $script:transferred += $Bytes.Length
            $script:present=$true
        }
    }

    It 'sends exact ordered chunks and appends only after the first write' {
        $payload=[byte[]]::new(17003)
        for ($i=0;$i -lt $payload.Length;$i++) { $payload[$i]=[byte]($i%251) }
        Send-RemoteFile -Payload $payload -LogicalPath 'fixture.deploy'
        $script:chunks.Count | Should Be 3
        $script:chunks[0].Append | Should Be $false
        $script:chunks[1].Append | Should Be $true
        $script:chunks[2].Bytes.Length | Should Be 619
        $joined=[byte[]]@($script:chunks | ForEach-Object { $_.Bytes })
        Get-CustodyByteSha256 -Bytes $joined | Should Be (Get-CustodyByteSha256 -Bytes $payload)
    }

    It 'commits exactly one empty write and rejects contradictory empty readback' {
        Send-RemoteFile -Payload ([byte[]]::new(0)) -LogicalPath 'fixture.deploy'
        $script:chunks.Count | Should Be 1
        $script:chunks[0].Bytes.Length | Should Be 0
        $script:present=$false
        $script:offsetDrift=$true
        { Send-RemoteFile -Payload ([byte[]]::new(0)) -LogicalPath 'fixture.deploy' } | Should Throw 'empty file exactly'
    }

    It 'stops at an ambiguous target, failed write or incorrect remote offset' {
        $script:present=$true
        { Send-RemoteFile -Payload ([byte[]](1,2)) -LogicalPath 'fixture.deploy' } | Should Throw 'not exactly absent'
        $script:chunks.Count | Should Be 0
        $script:present=$false; $script:writeFailure=$true
        { Send-RemoteFile -Payload ([byte[]](1,2)) -LogicalPath 'fixture.deploy' } | Should Throw 'transport stopped'
        $script:writeFailure=$false; $script:offsetDrift=$true
        { Send-RemoteFile -Payload ([byte[]]::new(17003)) -LogicalPath 'fixture.deploy' } | Should Throw 'offset drifted'
        $script:chunks.Count | Should Be 1
    }

    It 'hashes exact prefixes including zero and rejects lengths outside the member' {
        $payload=[byte[]](1,2,3,4)
        Get-PublicationPrefixSha256 -Payload $payload -Bytes 2 | Should Be (Get-CustodyByteSha256 -Bytes ([byte[]](1,2)))
        Get-PublicationPrefixSha256 -Payload $payload -Bytes 0 | Should Be (Get-CustodyByteSha256 -Bytes ([byte[]]::new(0)))
        foreach ($length in @(-1,5)) { { Get-PublicationPrefixSha256 -Payload $payload -Bytes $length } | Should Throw 'prefix length' }
    }

    It 'deletes only authenticated partial staging bytes during rollback' {
        $script:publicationInput=[pscustomobject]@{}
        $script:publicationInputSha256='c'*64; $script:manifestSha256='e'*64; $script:publicationOperationId='1'*32
        $payload=[byte[]](1,2,3,4)
        $entry=[pscustomobject]@{Path='fixture.bin';StagePath='fixture.deploy';CandidateBytes=4;CandidateSha256=(Get-CustodyByteSha256 -Bytes $payload)}
        $identity=[pscustomobject]@{State='present';Bytes=2;Sha256=(Get-CustodyByteSha256 -Bytes ([byte[]](1,2)))}
        Mock Get-CustodyPublicationFileBytes { return ,([byte[]](1,2,3,4)) }
        Mock Remove-RemoteExactFile { }
        Remove-RemoteCandidateStageForRollback -Entry $entry -Identity $identity
        Assert-MockCalled Remove-RemoteExactFile -Times 1 -Exactly -Scope It -ParameterFilter { $Bytes -eq 2 -and $LogicalPath -eq 'fixture.deploy' }
        $identity.Sha256='0'*64
        { Remove-RemoteCandidateStageForRollback -Entry $entry -Identity $identity } | Should Throw 'substituted staging bytes'
        $identity.Bytes=5
        { Remove-RemoteCandidateStageForRollback -Entry $entry -Identity $identity } | Should Throw 'oversized staging artifact'
        $identity.Bytes=2; $entry.CandidateSha256='0'*64
        { Remove-RemoteCandidateStageForRollback -Entry $entry -Identity $identity } | Should Throw 'cannot authenticate'
        Assert-MockCalled Remove-RemoteExactFile -Times 1 -Exactly -Scope It
    }
}

Describe 'Publication input through the actual file publisher' {
    BeforeEach {
        $root='E:\Source\Rust\MiniModel.org'
        $head=& git -c safe.directory=E:/Source/Rust/MiniModel.org -C $root rev-parse HEAD
        $tree=& git -c safe.directory=E:/Source/Rust/MiniModel.org -C $root rev-parse 'HEAD^{tree}'
        $bytes=Get-CustodyGitBlobBytes -RepositoryRoot $root -Commit $head -RelativePath 'catalog/compositions.v1.json'
        $script:publicationFile=[pscustomobject]@{path='catalog/compositions.v1.json';bytes=$bytes.Length;sha256=(Get-CustodyByteSha256 -Bytes $bytes)}
        $manifest=[pscustomobject]@{source_commit=$head;source_tree=$tree;materializations=@();files=@($script:publicationFile)}
        $authority=Get-Content -LiteralPath (Join-Path $root 'deploy/publication-authority.v1.json') -Raw | ConvertFrom-Json
        $script:manifestSha256='e'*64; $script:publicationOperationId='1'*32
        $script:publicationInput=New-CustodyPublicationInput -Phase 'public-admitted' -RepositoryRoot $root -Manifest $manifest -ManifestSha256 $script:manifestSha256 -OperationId $script:publicationOperationId -Authority $authority
        $script:publicationInputSha256=$script:publicationInput.BindingSha256
        $script:transactionJournal=[Collections.Generic.List[object]]::new()
        $script:remoteFiles=@{}
        Mock Assert-RemoteLeaseOwned { }
        Mock Assert-RemoteParentDirectoriesExist { }
        Mock Get-RemoteFileObservation {
            param($LogicalPath)
            if ($script:remoteFiles.ContainsKey($LogicalPath)) { return [pscustomobject]@{State='present';Size=$script:remoteFiles[$LogicalPath].Length} }
            return [pscustomobject]@{State='absent';Size=$null}
        }
        Mock Get-RemoteBytes {
            param($LogicalPath,$MaximumBytes)
            if (-not $script:remoteFiles.ContainsKey($LogicalPath)) { throw 'Missing simulated remote file.' }
            if ($script:remoteFiles[$LogicalPath].Length -gt $MaximumBytes) { throw 'Simulated read exceeded bound.' }
            return ,([byte[]]$script:remoteFiles[$LogicalPath].Clone())
        }
        Mock Write-RemoteChunk {
            param($LogicalPath,$Bytes,$Append)
            if ($Append) { $script:remoteFiles[$LogicalPath]=[byte[]]($script:remoteFiles[$LogicalPath]+$Bytes) }
            else { $script:remoteFiles[$LogicalPath]=[byte[]]$Bytes.Clone() }
        }
        Mock Rename-RemoteFile {
            param($LogicalPath,$NewLeafName)
            $slash=$LogicalPath.LastIndexOf('/')
            $target=$LogicalPath.Substring(0,$slash+1)+$NewLeafName
            if ($script:remoteFiles.ContainsKey($target)) { throw 'Rename would replace an existing entry.' }
            $script:remoteFiles[$target]=$script:remoteFiles[$LogicalPath]
            $script:remoteFiles.Remove($LogicalPath)
        }
    }

    It 'publishes the verified bytes and leaves no operation stage' {
        $result=Publish-File -File $script:publicationFile -Nonce $script:publicationOperationId
        $result.status | Should Be 'published'
        $script:remoteFiles.Count | Should Be 1
        Get-CustodyByteSha256 -Bytes $script:remoteFiles[$script:publicationFile.path] | Should Be $script:publicationFile.sha256
        $script:transactionJournal.Count | Should Be 1
        $script:transactionJournal[0].State | Should Be 'published'
    }

    It 'retains the exact predecessor backup until the later transaction gates' {
        $script:remoteFiles[$script:publicationFile.path]=[byte[]](1,2,3)
        $previous=[pscustomobject]@{path=$script:publicationFile.path;bytes=3;sha256=(Get-CustodyByteSha256 -Bytes ([byte[]](1,2,3)))}
        $result=Publish-File -File $script:publicationFile -PredecessorFile $previous -Nonce $script:publicationOperationId
        $result.status | Should Be 'published'
        $script:remoteFiles.Count | Should Be 2
        $entry=$script:transactionJournal[0]
        $entry.HadPredecessor | Should Be $true
        Get-CustodyByteSha256 -Bytes $script:remoteFiles[$entry.BackupPath] | Should Be $previous.sha256
    }

    It 'leaves an already-current predecessor unchanged without writing a stage' {
        $script:remoteFiles[$script:publicationFile.path]=[byte[]]$script:publicationInput.Payloads[$script:publicationFile.path].Clone()
        $result=Publish-File -File $script:publicationFile -PredecessorFile $script:publicationFile -Nonce $script:publicationOperationId
        $result.status | Should Be 'already-current'
        $script:transactionJournal.Count | Should Be 0
        Assert-MockCalled Write-RemoteChunk -Times 0 -Exactly -Scope It
        Assert-MockCalled Rename-RemoteFile -Times 0 -Exactly -Scope It
    }

    It 'rejects corrupted input before any remote payload read or write' {
        $script:publicationInput.Payloads[$script:publicationFile.path][0]=0
        { Publish-File -File $script:publicationFile -Nonce $script:publicationOperationId } | Should Throw 'bytes drifted'
        $script:transactionJournal.Count | Should Be 0
        Assert-MockCalled Get-RemoteBytes -Times 0 -Exactly -Scope It
        Assert-MockCalled Write-RemoteChunk -Times 0 -Exactly -Scope It
        Assert-MockCalled Rename-RemoteFile -Times 0 -Exactly -Scope It
    }
}

Describe 'Verified Node child boundary' {
    BeforeEach {
        $script:NodeExecutablePath='C:\Program Files\nodejs\node.exe'
    }
    It 'rejects arbitrary tools, arguments and wrong runtime identities before execution' {
        { Invoke-VerifiedNode -Tool manifest -Arguments @('HEAD') } | Should Throw 'Invalid manifest arguments'
        { Invoke-VerifiedNode -Tool manifest -Arguments @((('a'*40)+"`n")) } | Should Throw 'Invalid manifest arguments'
        { Invoke-VerifiedNode -Tool manifest -Arguments @(('a'*40),'unexpected.json') } | Should Throw 'Invalid manifest arguments'
        { Invoke-VerifiedNode -Tool source-authority -Arguments @('--require-clean','--require-clean') } | Should Throw 'Invalid source-authority arguments'
        { Invoke-VerifiedNode -Tool source-authority -Arguments @('--eval') } | Should Throw 'Invalid source-authority arguments'
        $script:NodeExecutablePath='node.exe'
        { Invoke-VerifiedNode -Tool source-authority } | Should Throw 'must be absolute'
        $script:NodeExecutablePath=Join-Path $PSHOME 'powershell.exe'
        { Invoke-VerifiedNode -Tool source-authority } | Should Throw 'file identity drifted'
    }
    It 'terminates its actual Node child when the deadline expires' {
        1..3 | ForEach-Object {
            { Invoke-VerifiedNode -Tool source-authority -TimeoutMilliseconds 1 } | Should Throw 'exceeded its deadline'
        }
    }
    It 'runs the real manifest tool with hostile Node startup settings excluded from its child only' {
        $saved=@{}
        $coveragePath=Join-Path $TestDrive 'must-not-create-node-coverage'
        $settings=@{NODE_OPTIONS='--require definitely-not-loaded';NODE_PATH='definitely-not-searched';NODE_V8_COVERAGE=$coveragePath;NODE_TLS_REJECT_UNAUTHORIZED='0'}
        foreach ($name in $settings.Keys) {
            $saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')
            [Environment]::SetEnvironmentVariable($name,$settings[$name],'Process')
        }
        try {
            $head=& git -c safe.directory=E:/Source/Rust/MiniModel.org -C $repositoryRoot rev-parse HEAD
            $result=Invoke-VerifiedNode -Tool manifest -Arguments @($head)
            $result.ExitCode | Should Be 0
            $result.Stderr | Should Be ''
            ($result.Stdout | ConvertFrom-Json).files.Count | Should Be 22
            foreach ($name in $settings.Keys) { [Environment]::GetEnvironmentVariable($name,'Process') | Should Be $settings[$name] }
            Test-Path -LiteralPath $coveragePath | Should Be $false
        }
        finally {
            foreach ($name in $settings.Keys) { [Environment]::SetEnvironmentVariable($name,$saved[$name],'Process') }
        }
    }
}

# Gate responses are mocked at the verified process boundary; no executable or source copy is written.
Describe 'Publisher source-authority gate admission' {
    BeforeEach {
        $script:Publish=$true
        $script:gateVerdict='{"verdict":"pass","canonicalLayoutAvailable":true,"localScanPerformed":true,"coverageClass":"registered-governed-roots-only"}'
        $script:gateArguments=@()
        Mock Invoke-VerifiedNode {
            param($Tool,$Arguments)
            $script:gateTool=$Tool
            $script:gateArguments=@($Arguments)
            return [pscustomobject]@{ExitCode=0;Stdout=($script:gateVerdict+"`n");Stderr=''}
        }
    }
    It 'requires the fixed gate and both publication flags' {
        Assert-EcosystemSingleSourceAuthority
        $script:gateTool | Should Be 'source-authority'
        $script:gateArguments[0] | Should Be '--require-clean'
        $script:gateArguments[1] | Should Be '--require-remote-exact'
    }
    It 'rejects a failed, malformed or incomplete gate verdict' {
        foreach ($verdict in @('invalid-json','{}','{"verdict":"fail","canonicalLayoutAvailable":true,"localScanPerformed":true,"coverageClass":"registered-governed-roots-only"}','{"verdict":"pass","canonicalLayoutAvailable":true,"localScanPerformed":false,"coverageClass":"registered-governed-roots-only"}')) {
            $script:gateVerdict=$verdict
            { Assert-EcosystemSingleSourceAuthority } | Should Throw
        }
    }
}
