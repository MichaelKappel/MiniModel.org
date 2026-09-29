# Exercise canonical code with publication payloads and transport observations held in memory.
Add-Type -AssemblyName System.Net.Http
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Import-Module (Join-Path $repositoryRoot 'tools/MiniModel.DeploymentCustody.psm1') -Force
Import-Module (Join-Path $repositoryRoot 'tools/MiniModel.StaticSeed.psm1') -Force

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
$script:seedFilePaths=@()
$script:seedOperationId=''

Describe 'Publisher tracked-file observation with real canonical Git bytes' {
    It 'binds working bytes to the observed exact commit and preserves strict revision admission' {
        $observedCommit = (& git --no-optional-locks -C $repositoryRoot rev-parse HEAD).Trim()
        Test-TrackedPathMatchesHead -RelativePath 'catalog/compositions.v1.json' -ObservedCommit $observedCommit | Should Be $true
        Test-TrackedPathMatchesHead -RelativePath 'missing-tracked-file-fixture.txt' -ObservedCommit $observedCommit | Should Be $false
        Test-TrackedPathMatchesHead -RelativePath 'deploy/site-deployment-manifest.v1.json' -ObservedCommit '672991ba646cde761ba6a361fc20b5a24aaabbe0' | Should Be $false
        { Test-TrackedPathMatchesHead -RelativePath 'catalog/compositions.v1.json' -ObservedCommit 'HEAD' } | Should Throw 'exact object identity'
    }
}

Describe 'Bounded actual NLST ingress' {
    BeforeEach {$script:seedFilePaths=@()}
    It 'ignores only literal dot entries and keeps provider state and other names visible' {
        $bytes=[Text.Encoding]::UTF8.GetBytes(".`r`n..`r`n.ftpquota`r`nindex.html`r`n")
        (@(ConvertFrom-CustodyDirectoryListing -Bytes $bytes) -join ',') | Should Be '.ftpquota,index.html'
        @(ConvertFrom-CustodyDirectoryListing -Bytes ([byte[]]::new(0))).Count | Should Be 0
        foreach($text in @('../outside',"x`ny`nx","x`nX",' .',"x`ry",'dir/file','sha256:abc')){
            {ConvertFrom-CustodyDirectoryListing -Bytes ([Text.Encoding]::UTF8.GetBytes($text))} | Should Throw
        }
        {ConvertFrom-CustodyDirectoryListing -Bytes ([byte[]](255))} | Should Throw 'not UTF-8'
        {ConvertFrom-CustodyDirectoryListing -Bytes ([byte[]]::new(65537))} | Should Throw 'byte bound'
        $large=([string[]](1..501|ForEach-Object{"file$_"})) -join "`n"
        {ConvertFrom-CustodyDirectoryListing -Bytes ([Text.Encoding]::UTF8.GetBytes($large))} | Should Throw 'entry bound'
    }
    It 'admits a registered digest directory only at its exact parent' {
        $directory='tinyrustlm/direct-composition/v1/compositions/sha256:'+('a'*64)+'/members'
        $paths=@('model.slm2','tokenizer.tokenizer2','template.template2','sampling.sampling2','prompt.prompt2','composition.acg2','.htaccess')|ForEach-Object{"$directory/$_"}
        $bytes=[Text.Encoding]::UTF8.GetBytes('sha256:'+('a'*64))
        @(ConvertFrom-CustodyDirectoryListing -Bytes $bytes -DirectoryPath 'tinyrustlm/direct-composition/v1/compositions' -SeedFilePaths $paths).Count | Should Be 1
        {ConvertFrom-CustodyDirectoryListing -Bytes $bytes -DirectoryPath 'elsewhere' -SeedFilePaths $paths} | Should Throw 'outside the registered'
    }
    It 'bounds the actual response stream before decoding and closes its handles' {
        $script:nlstReply=[pscustomobject]@{Stream=[IO.MemoryStream]::new([Text.Encoding]::UTF8.GetBytes(".`n..`nindex.html`n"));Disposed=$false}
        $script:nlstReply|Add-Member ScriptMethod GetResponseStream {return $this.Stream}
        $script:nlstReply|Add-Member ScriptMethod Dispose {$this.Disposed=$true}
        $script:nlstRequest=[pscustomobject]@{Reply=$script:nlstReply}
        $script:nlstRequest|Add-Member ScriptMethod GetResponse {return $this.Reply}
        Mock New-FtpRequest {return $script:nlstRequest}
        (@(Get-RemoteDirectoryEntries -LogicalPath '') -join ',') | Should Be 'index.html'
        $script:nlstReply.Disposed | Should Be $true
        $script:nlstReply.Stream.CanRead | Should Be $false
        $script:nlstReply.Stream=[IO.MemoryStream]::new([byte[]]::new(0));$script:nlstReply.Disposed=$false
        $empty=Get-RemoteBytes -LogicalPath 'empty-test-only' -MaximumBytes 1
        ($empty -is [byte[]]) | Should Be $true
        $empty.Length | Should Be 0
        $script:nlstReply.Disposed | Should Be $true
        $script:nlstReply.Stream=[IO.MemoryStream]::new([byte[]]::new(65537));$script:nlstReply.Disposed=$false
        {Get-RemoteDirectoryEntries -LogicalPath ''} | Should Throw 'byte bound'
        $script:nlstReply.Disposed | Should Be $true
        $script:nlstReply.Stream.CanRead | Should Be $false
    }
}

Describe 'Empty remote directory observation' {
    It 'proves a new lease owner absent using two empty directory observations without SIZE' {
        Mock Get-RemoteDirectoryEntries { return @() }
        Mock Get-RemoteSize { throw 'SIZE is forbidden for an unlisted file.' }
        $observation=Get-RemoteFileObservation -LogicalPath '.minimodel-deployment-lease.v1/owner-fixture.json'
        $observation.State | Should Be 'absent'
        ($null -eq $observation.Size) | Should Be $true
        Assert-MockCalled Get-RemoteDirectoryEntries -Times 2 -Exactly -Scope It -ParameterFilter {$LogicalPath -ceq '.minimodel-deployment-lease.v1'}
        Assert-MockCalled Get-RemoteSize -Times 0 -Exactly -Scope It
    }
}

Describe 'Actual FTP request URI directory semantics without transport' {
    BeforeEach {
        $script:seedFilePaths=@()
        $script:seedOperationId=''
        $script:sourceAuthorityAdmitted=$true
        $script:phase='read-only'
        $script:ftpHost='fixture.invalid'
        $script:ftpPort=21
        $script:remoteRoot='.'
        $script:credential=[Net.NetworkCredential]::new('fixture-user','fixture-password')
    }
    AfterEach { $script:credential=$null;$script:seedFilePaths=@();$script:seedOperationId='' }
    It 'names a directory explicitly for root, nested and lease NLST requests' {
        foreach ($logical in @('','catalog','catalog/nested','.minimodel-deployment-lease.v1')) {
            $request=New-FtpRequest -LogicalPath $logical -Method ([Net.WebRequestMethods+Ftp]::ListDirectory)
            try {
                $expected=if ($logical.Length -eq 0) {'/'} else {"/$logical/"}
                $request.RequestUri.AbsolutePath | Should Be $expected
                $request.Method | Should Be ([Net.WebRequestMethods+Ftp]::ListDirectory)
                $request.EnableSsl | Should Be $true
                $request.KeepAlive | Should Be $false
            } finally { $request.Abort() }
        }
        $script:remoteRoot='public_html'
        $request=New-FtpRequest -LogicalPath 'catalog' -Method ([Net.WebRequestMethods+Ftp]::ListDirectory)
        try { $request.RequestUri.AbsolutePath | Should Be '/public_html/catalog/' }
        finally { $request.Abort() }
    }
    It 'preserves exact file and directory mutation targets for every other used FTP method' {
        $script:phase='leased'
        foreach ($method in @('SIZE','RETR','STOR','APPE','DELE','RENAME','MKD','RMD')) {
            $request=New-FtpRequest -LogicalPath 'catalog/fixture.json' -Method $method -Mutating:($method -notin @('SIZE','RETR'))
            try {
                $request.RequestUri.AbsolutePath | Should Be '/catalog/fixture.json'
                $request.Method | Should Be $method
            } finally { $request.Abort() }
        }
    }
    It 'preserves source admission and mutation phase gates before request construction' {
        $script:sourceAuthorityAdmitted=$false
        {New-FtpRequest -LogicalPath 'catalog' -Method 'NLST'} | Should Throw 'before the ecosystem'
        $script:sourceAuthorityAdmitted=$true
        {New-FtpRequest -LogicalPath 'catalog/fixture.json' -Method 'STOR' -Mutating} | Should Throw 'forbidden'
    }
    It 'restricts colon request and rename targets to the registered members and owned stages' {
        $directory='tinyrustlm/direct-composition/v1/compositions/sha256:'+('a'*64)+'/members'
        $script:seedFilePaths=@('model.slm2','tokenizer.tokenizer2','template.template2','sampling.sampling2','prompt.prompt2','composition.acg2','.htaccess')|ForEach-Object {"$directory/$_"}
        $script:seedOperationId='1'*32
        $stage="$($script:seedFilePaths[0]).seed-$($script:seedOperationId).tmp"
        $script:phase='leased'
        $request=New-FtpRequest -LogicalPath $stage -Method 'STOR' -Mutating
        try { $request.RequestUri.OriginalString.Contains('sha256%3A') | Should Be $true }
        finally {$request.Abort()}
        {New-FtpRequest -LogicalPath "$directory/extra.bin" -Method 'RETR'} | Should Throw 'outside the registered'
        {New-FtpRequest -LogicalPath ($stage -replace ('1'*32),('2'*32)) -Method 'STOR' -Mutating} | Should Throw 'outside the registered'
        {Rename-RemoteFile -LogicalPath $stage -NewLeafName 'extra.bin'} | Should Throw 'outside the registered'
    }
}

Describe 'Registered seed FTP streaming without model files or network' {
    BeforeEach {
        $directory='tinyrustlm/direct-composition/v1/compositions/sha256:'+('a'*64)+'/members'
        $script:seedFilePaths=@('model.slm2','tokenizer.tokenizer2','template.template2','sampling.sampling2','prompt.prompt2','composition.acg2','.htaccess')|ForEach-Object {"$directory/$_"}
        $script:seedOperationId='1'*32
        $script:seedBody=[byte[]]::new(131079)
        for($index=0;$index -lt $script:seedBody.Length;$index++){$script:seedBody[$index]=[byte]($index % 251)}
        $hash=Get-CustodyByteSha256 -Bytes $script:seedBody
        $script:seedTransferFiles=@($script:seedFilePaths|ForEach-Object {[pscustomobject]@{path=$_;bytes=$script:seedBody.Length;sha256=$hash}})
        $script:seedStage="$($script:seedFilePaths[0]).seed-$($script:seedOperationId).tmp"
        $script:seedObservation=[pscustomobject]@{State='absent';Size=$null}
        $script:seedReply=[pscustomobject]@{Stream=[IO.MemoryStream]::new($script:seedBody,$false);Disposed=$false}
        $script:seedReply|Add-Member ScriptMethod GetResponseStream {return $this.Stream}
        $script:seedReply|Add-Member ScriptMethod Dispose {$this.Disposed=$true}
        $script:seedRequest=[pscustomobject]@{ContentLength=[int64]0;Output=[IO.MemoryStream]::new();Reply=$script:seedReply;Aborted=$false;ResponseRequested=$false}
        $script:seedRequest|Add-Member ScriptMethod GetRequestStream {return $this.Output}
        $script:seedRequest|Add-Member ScriptMethod GetResponse {$this.ResponseRequested=$true;return $this.Reply}
        $script:seedRequest|Add-Member ScriptMethod Abort {$this.Aborted=$true}
        Mock Assert-RemoteLeaseOwned {}
        Mock Get-RemoteFileObservation {return $script:seedObservation}
        Mock New-FtpRequest {return $script:seedRequest}
    }
    AfterEach {
        $script:seedRequest.Output.Dispose();$script:seedReply.Stream.Dispose()
        $script:seedFilePaths=@();$script:seedTransferFiles=@();$script:seedOperationId=''
    }
    It 'uses one STOR request and the retained source handle to write more than one buffer' {
        $source=[IO.MemoryStream]::new($script:seedBody,$false)
        try {
            $source.Position=7
            $result=Send-RemoteSeedStream -Source $source -LogicalPath $script:seedStage
            $result.Bytes | Should Be $script:seedBody.Length
            $result.Sha256 | Should Be $script:seedTransferFiles[0].sha256
            (Get-CustodyByteSha256 -Bytes $script:seedRequest.Output.ToArray()) | Should Be $result.Sha256
            $script:seedRequest.ContentLength | Should Be $script:seedBody.Length
            $script:seedRequest.Output.CanWrite | Should Be $false
            $script:seedRequest.ResponseRequested | Should Be $true
            $script:seedReply.Disposed | Should Be $true
            $script:seedRequest.Aborted | Should Be $false
            $source.CanRead | Should Be $true
            Assert-MockCalled Assert-RemoteLeaseOwned -Times 1 -Exactly -Scope It
            Assert-MockCalled New-FtpRequest -Times 1 -Exactly -Scope It -ParameterFilter {$Method -ceq 'STOR' -and $Mutating -and $LogicalPath -ceq $script:seedStage}
        } finally {$source.Dispose()}
    }
    It 'rejects occupied staging and unowned source lengths before opening a request' {
        $source=[IO.MemoryStream]::new([byte[]](1,2,3),$false)
        try {{Send-RemoteSeedStream -Source $source -LogicalPath $script:seedStage} | Should Throw 'exact retained stream'}
        finally {$source.Dispose()}
        $source=[IO.MemoryStream]::new($script:seedBody,$false)
        try {
            $script:seedObservation.State='present'
            {Send-RemoteSeedStream -Source $source -LogicalPath $script:seedStage} | Should Throw 'not exactly absent'
            {Send-RemoteSeedStream -Source $source -LogicalPath $script:seedFilePaths[0]} | Should Throw 'not admitted'
            Assert-MockCalled New-FtpRequest -Times 0 -Exactly -Scope It
        } finally {$source.Dispose()}
    }
    It 'aborts a transfer whose consumed bytes changed without disposing the caller source' {
        $changed=[byte[]]$script:seedBody.Clone();$changed[0]=255
        $source=[IO.MemoryStream]::new($changed,$false)
        try {
            {Send-RemoteSeedStream -Source $source -LogicalPath $script:seedStage} | Should Throw 'hash differs'
            $script:seedRequest.Aborted | Should Be $true
            $script:seedRequest.Output.CanWrite | Should Be $false
            $script:seedRequest.ResponseRequested | Should Be $false
            $source.CanRead | Should Be $true
        } finally {$source.Dispose()}
    }
    It 'preserves an ambiguous upload response as failure and closes owned transport handles' {
        $script:seedRequest|Add-Member -Force ScriptMethod GetResponse {$this.ResponseRequested=$true;throw 'Synthetic response lost after write.'}
        $source=[IO.MemoryStream]::new($script:seedBody,$false)
        try {
            {Send-RemoteSeedStream -Source $source -LogicalPath $script:seedStage} | Should Throw 'response lost'
            $script:seedRequest.Output.CanWrite | Should Be $false
            $script:seedRequest.Output.ToArray().Length | Should Be $script:seedBody.Length
            $script:seedRequest.Aborted | Should Be $true
            $source.CanRead | Should Be $true
            Assert-MockCalled New-FtpRequest -Times 1 -Exactly -Scope It
        } finally {$source.Dispose()}
    }
    It 'streams exact staged and final readback bodies after SIZE admission' {
        $script:seedObservation=[pscustomobject]@{State='present';Size=$script:seedBody.Length}
        foreach($path in @($script:seedStage,$script:seedFilePaths[0])){
            $script:seedReply.Stream=[IO.MemoryStream]::new($script:seedBody,$false);$script:seedReply.Disposed=$false
            $result=Assert-RemoteSeedStreamIdentity -LogicalPath $path
            $result.Bytes | Should Be $script:seedBody.Length
            $result.Sha256 | Should Be $script:seedTransferFiles[0].sha256
            $script:seedReply.Stream.CanRead | Should Be $false
            $script:seedReply.Disposed | Should Be $true
        }
        Assert-MockCalled New-FtpRequest -Times 2 -Exactly -Scope It -ParameterFilter {$Method -ceq 'RETR' -and -not $Mutating}
    }
    It 'rejects missing or mismatched SIZE before download and detects truncated or changed bodies' {
        {Assert-RemoteSeedStreamIdentity -LogicalPath $script:seedStage} | Should Throw 'SIZE differs'
        $script:seedObservation=[pscustomobject]@{State='present';Size=3}
        {Assert-RemoteSeedStreamIdentity -LogicalPath $script:seedStage} | Should Throw 'SIZE differs'
        Assert-MockCalled New-FtpRequest -Times 0 -Exactly -Scope It
        $script:seedObservation.Size=$script:seedBody.Length
        $script:seedReply.Stream.Dispose();$script:seedReply.Stream=[IO.MemoryStream]::new([byte[]](1,2,3),$false)
        {Assert-RemoteSeedStreamIdentity -LogicalPath $script:seedStage} | Should Throw 'ended before'
        $script:seedReply.Stream.CanRead | Should Be $false
        $script:seedReply.Disposed | Should Be $true
        $changed=[byte[]]$script:seedBody.Clone();$changed[0]=255
        $script:seedReply.Stream=[IO.MemoryStream]::new($changed,$false);$script:seedReply.Disposed=$false
        {Assert-RemoteSeedStreamIdentity -LogicalPath $script:seedStage} | Should Throw 'hash differs'
        $script:seedReply.Disposed | Should Be $true
    }
    It 'rejects another operation, unregistered leaves and missing or malformed transfer identity' {
        {Get-RegisteredSeedTransferFile -LogicalPath ($script:seedStage -replace ('1'*32),('2'*32))} | Should Throw 'outside the registered'
        {Get-RegisteredSeedTransferFile -LogicalPath ($script:seedStage+'.extra')} | Should Throw 'outside the registered'
        $script:seedTransferFiles[0].sha256=('a'*64)+"`n"
        {Get-RegisteredSeedTransferFile -LogicalPath $script:seedStage} | Should Throw 'not admitted'
        $script:seedTransferFiles[0].sha256='a'*64
        foreach($length in @([int64]0,[int64]68719476737)){
            $script:seedTransferFiles[0].bytes=$length
            {Get-RegisteredSeedTransferFile -LogicalPath $script:seedStage} | Should Throw 'not admitted'
        }
        $script:seedTransferFiles=@()
        {Get-RegisteredSeedTransferFile -LogicalPath $script:seedStage} | Should Throw 'no admitted member closure'
        Assert-MockCalled New-FtpRequest -Times 0 -Exactly -Scope It
    }
}

Describe 'Exact bootstrap manifest admission' {
    BeforeEach {
        $script:manifestAuthority=Get-Content -LiteralPath (Join-Path $repositoryRoot 'deploy/publication-authority.v1.json') -Raw|ConvertFrom-Json
        $script:manifestText=Get-Content -LiteralPath (Join-Path $repositoryRoot 'deploy/root-bootstrap-manifest.v1.json') -Raw
    }
    It 'admits all three checked-in evidence identities without regex state replacing them' {
        $record=$script:manifestText|ConvertFrom-Json
        Assert-CustodyBootstrapManifest -Manifest $record -Authority $script:manifestAuthority
        $record.public_matches.Count | Should Be 3
        $record.public_matches[2].path | Should Be 'catalog/peer-announcements.v1.json'
    }
    It 'rejects unknown schema, unsafe inventories and incomplete or altered public evidence' {
        $mutations=@(
            {$args[0].schema='unknown'},
            {$args[0].observed_at_utc='yesterday'},
            {$args[0].root_entries=@('a','b')},
            {$args[0].root_entries=@('a','b','A')},
            {$args[0].root_entries=@('a','b','../outside')},
            {$args[0].root_entries=@('a','b',$script:manifestAuthority.transport.root_sentinel_path)},
            {$args[0].root_entries=@('a','b',$script:manifestAuthority.transport.lease_directory_path)},
            {$args[0].public_matches=@($args[0].public_matches[0])},
            {$args[0].public_matches[2].path='other.json'},
            {$args[0].public_matches[2].bytes=2097153},
            {$args[0].public_matches[2].bytes='263'},
            {$args[0].public_matches[2].sha256='G'*64}
        )
        foreach($mutate in $mutations){
            $record=$script:manifestText|ConvertFrom-Json
            & $mutate $record
            {Assert-CustodyBootstrapManifest -Manifest $record -Authority $script:manifestAuthority} | Should Throw
        }
    }
}

Describe 'Exact sentinel bootstrap with memory-only remote state' {
    BeforeEach {
        $script:sourceAuthorityAdmitted=$true;$script:phase='read-only';$script:lease=$null;$script:FtpExportPath='test-only-never-read'
        $script:bootstrapAuthority=Get-Content -LiteralPath (Join-Path $repositoryRoot 'deploy/publication-authority.v1.json') -Raw|ConvertFrom-Json
        $script:bootstrapAuthority.transport.username_sha256=Get-CustodyByteSha256 ([Text.Encoding]::UTF8.GetBytes('synthetic-user'))
        $script:bootstrapRecord=[pscustomobject]@{Value=$script:bootstrapAuthority;Sha256='a'*64}
        $script:bootstrapManifestValue=Get-Content -LiteralPath (Join-Path $repositoryRoot 'deploy/root-bootstrap-manifest.v1.json') -Raw|ConvertFrom-Json
        foreach($match in $script:bootstrapManifestValue.public_matches){$match.bytes=3;$match.sha256=Get-CustodyByteSha256 ([byte[]](1,2,3))}
        $script:bootstrapManifestBytes=[Text.Encoding]::UTF8.GetBytes(($script:bootstrapManifestValue|ConvertTo-Json -Depth 8))
        $script:bootstrapManifestRecord=[pscustomobject]@{Value=$script:bootstrapManifestValue;Sha256=(Get-CustodyByteSha256 $script:bootstrapManifestBytes)}
        $script:bootstrapSentinel=[IO.File]::ReadAllBytes((Join-Path $repositoryRoot $script:bootstrapAuthority.transport.root_sentinel_source_path))
        $script:bootstrapFinal=$null;$script:bootstrapStage=$null;$script:bootstrapStagePath=$null;$script:bootstrapLeaseExists=$false;$script:bootstrapFailure='';$script:bootstrapExtra=$null
        $script:bootstrapObserved=[pscustomobject]@{canonical_root=$repositoryRoot;root_has_reparse_point=$false;branch='master';upstream='origin/master';remotes=@('origin');fetch_url=$script:bootstrapAuthority.repository.remote_url;push_url=$script:bootstrapAuthority.repository.remote_url;head='b'*40;tracking_head='b'*40;remote_head='b'*40;status_count=0;source_is_ancestor=$true;source_tree=$script:bootstrapAuthority.candidate.source_tree;manifest_matches_head=$true;authority_matches_head=$true}
        Mock Get-RepositoryObservation {return $script:bootstrapObserved}
        Mock Read-CustodyJsonFile {return $script:bootstrapManifestRecord}
        Mock Get-CustodyGitBlobBytes {param($RelativePath) if($RelativePath -ceq 'deploy/root-bootstrap-manifest.v1.json'){return ,$script:bootstrapManifestBytes};return ,$script:bootstrapSentinel}
        Mock Get-PublicBytes {return ,([byte[]](1,2,3))}
        Mock Import-MiniModelCredentialModule {}
        Mock Get-CredentialProfile {return [pscustomobject]@{profile='MiniModel.org';host='server41.shared.spaceship.host';port=21;user='synthetic-user';password='synthetic-test-only'}}
        Mock Get-RemoteDirectoryEntries {
            $entries=@($script:bootstrapManifestValue.root_entries)
            if($null -ne $script:bootstrapFinal){$entries += $script:bootstrapAuthority.transport.root_sentinel_path}
            if($null -ne $script:bootstrapStage){$entries += $script:bootstrapStagePath}
            if($script:bootstrapLeaseExists){$entries += $script:bootstrapAuthority.transport.lease_directory_path}
            if($script:bootstrapExtra){$entries += $script:bootstrapExtra}
            return $entries
        }
        Mock Get-RemoteFileObservation {
            param($LogicalPath)
            if($LogicalPath -ceq $script:bootstrapAuthority.transport.root_sentinel_path){
                if($script:bootstrapFailure -ceq 'target-race' -and $null -ne $script:bootstrapStage){$script:bootstrapFinal=[byte[]](9)}
                return [pscustomobject]@{State=$(if($null -ne $script:bootstrapFinal){'present'}else{'absent'})}
            }
            return [pscustomobject]@{State=$(if($null -ne $script:bootstrapStage){'present'}else{'absent'})}
        }
        Mock Get-RemoteBoundedIdentity {
            param($LogicalPath)
            $body=if($LogicalPath -ceq $script:bootstrapAuthority.transport.root_sentinel_path){$script:bootstrapFinal}else{$script:bootstrapStage}
            if($null -eq $body){return [pscustomobject]@{State='absent';Bytes=$null;Sha256=$null}}
            return [pscustomobject]@{State='present';Bytes=$body.Length;Sha256=(Get-CustodyByteSha256 ([byte[]]$body))}
        }
        Mock Assert-RemoteExactIdentity {
            param($LogicalPath,$Bytes,$Sha256)
            $body=if($LogicalPath -ceq $script:bootstrapAuthority.transport.root_sentinel_path){$script:bootstrapFinal}elseif($LogicalPath -ceq $script:bootstrapStagePath){$script:bootstrapStage}else{[byte[]](1,2,3)}
            if($null -eq $body -or $body.Length -ne $Bytes -or (Get-CustodyByteSha256 ([byte[]]$body)) -cne $Sha256){throw 'Synthetic remote identity mismatch'}
        }
        Mock New-RemoteDeploymentLease {
            $script:bootstrapLeaseExists=$true
            if($script:bootstrapFailure -ceq 'lease-response-lost'){throw 'Synthetic lease response lost'}
            $script:lease=[pscustomobject]@{DirectoryPath=$script:bootstrapAuthority.transport.lease_directory_path}
            return $script:lease
        }
        Mock Assert-RemoteLeaseOwned {if(-not $script:bootstrapLeaseExists){throw 'Synthetic lost lease'}}
        Mock Send-RemoteFile {
            param($Payload,$LogicalPath)
            $script:bootstrapStagePath=$LogicalPath;$script:bootstrapStage=[byte[]]$Payload.Clone()
            if($script:bootstrapFailure -ceq 'partial-upload'){$script:bootstrapStage=[byte[]]$Payload[0..2];throw 'Synthetic upload interruption'}
            if($script:bootstrapFailure -ceq 'foreign-stage'){$script:bootstrapStage=[byte[]](99,98,97);throw 'Synthetic substituted stage'}
        }
        Mock Rename-RemoteFile {
            $script:bootstrapFinal=$script:bootstrapStage;$script:bootstrapStage=$null
            if($script:bootstrapFailure -ceq 'rename-response-lost'){throw 'Synthetic rename response lost'}
        }
        Mock Remove-RemoteExactFile {
            param($LogicalPath,$Bytes,$Sha256)
            if($LogicalPath -cne $script:bootstrapStagePath -or $Bytes -ne $script:bootstrapStage.Length -or $Sha256 -cne (Get-CustodyByteSha256 $script:bootstrapStage)){throw 'Unsafe synthetic removal'}
            $script:bootstrapStage=$null;return $true
        }
        Mock Remove-RemoteDeploymentLease {$script:bootstrapLeaseExists=$false;$script:lease=$null}
    }
    It 'installs exactly the sentinel under the existing lease and leaves the website untouched' {
        $result=Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord
        $result.status | Should Be 'installed-and-readback-verified'
        $result.sentinel_verified | Should Be $true
        $result.lease_retained_or_creation_ambiguous | Should Be $false
        $result.website_published | Should Be $false
        $result.seed_published | Should Be $false
        (Get-CustodyByteSha256 $script:bootstrapFinal) | Should Be $script:bootstrapAuthority.transport.root_sentinel_sha256
        Assert-MockCalled New-RemoteDeploymentLease -Times 1 -Exactly -Scope It
        Assert-MockCalled Send-RemoteFile -Times 1 -Exactly -Scope It
        Assert-MockCalled Rename-RemoteFile -Times 1 -Exactly -Scope It
        Assert-MockCalled Remove-RemoteDeploymentLease -Times 1 -Exactly -Scope It
        Assert-MockCalled Remove-RemoteExactFile -Times 0 -Exactly -Scope It
    }
    It 'verifies an existing exact sentinel without overwriting it' {
        $script:bootstrapFinal=$script:bootstrapSentinel
        (Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord).status | Should Be 'already-present-verified'
        Assert-MockCalled New-RemoteDeploymentLease -Times 0 -Exactly -Scope It
        Assert-MockCalled Send-RemoteFile -Times 0 -Exactly -Scope It
    }
    It 'rejects missing source authority and public drift before credential access' {
        $script:sourceAuthorityAdmitted=$false
        {Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord} | Should Throw 'before source authority'
        $script:sourceAuthorityAdmitted=$true
        Mock Get-PublicBytes {return ,([byte[]](9,8,7))}
        {Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord} | Should Throw 'correspondence changed'
        Assert-MockCalled Get-CredentialProfile -Times 0 -Exactly -Scope It
    }
    It 'refuses new root overlays before creating a lease' {
        $script:bootstrapExtra='unreviewed.txt'
        (Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord).status | Should Be 'failed'
        Assert-MockCalled New-RemoteDeploymentLease -Times 0 -Exactly -Scope It
        Assert-MockCalled Send-RemoteFile -Times 0 -Exactly -Scope It
    }
    It 'removes only an exact partial stage after interruption' {
        $script:bootstrapFailure='partial-upload'
        $result=Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord
        $result.status | Should Be 'failed'
        $result.recovery | Should Be 'owned-stage-removed-lease-released'
        $result.sentinel_verified | Should Be $false
        $null -eq $script:bootstrapStage | Should Be $true
        Assert-MockCalled Remove-RemoteExactFile -Times 1 -Exactly -Scope It
        Assert-MockCalled Rename-RemoteFile -Times 0 -Exactly -Scope It
    }
    It 'preserves successful promotion after a lost response and releases its owned lease' {
        $script:bootstrapFailure='rename-response-lost'
        $result=Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord
        $result.status | Should Be 'failed'
        $result.sentinel_verified | Should Be $true
        $result.lease_retained_or_creation_ambiguous | Should Be $false
        Assert-MockCalled Remove-RemoteExactFile -Times 0 -Exactly -Scope It
    }
    It 'retains an untrusted stage and lease instead of deleting substituted bytes' {
        $script:bootstrapFailure='foreign-stage'
        $result=Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord
        $result.recovery | Should Be 'retained-for-exact-ownership-review'
        $result.lease_retained_or_creation_ambiguous | Should Be $true
        Assert-MockCalled Remove-RemoteExactFile -Times 0 -Exactly -Scope It
        Assert-MockCalled Remove-RemoteDeploymentLease -Times 0 -Exactly -Scope It
    }
    It 'preserves a raced final target and identifies ambiguous lease creation' {
        $script:bootstrapFailure='target-race'
        $result=Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord
        $result.sentinel_verified | Should Be $false
        $result.lease_retained_or_creation_ambiguous | Should Be $true
        $script:bootstrapFinal[0] | Should Be 9
        Assert-MockCalled Rename-RemoteFile -Times 0 -Exactly -Scope It
        $script:bootstrapFinal=$null;$script:bootstrapStage=$null;$script:bootstrapLeaseExists=$false;$script:lease=$null;$script:bootstrapFailure='lease-response-lost'
        $result=Invoke-RootSentinelBootstrap -AuthorityRecord $script:bootstrapRecord
        $result.lease_retained_or_creation_ambiguous | Should Be $true
    }
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
    It 'delivers bounded metadata bytes to the real pinned seed validator and preserves its rejection' {
        $result=Invoke-VerifiedNode -Tool seed-admission -InputBytes ([Text.Encoding]::UTF8.GetBytes("{}`n"))
        $result.ExitCode | Should Be 1
        $result.Stdout | Should Be ''
        $result.Stderr.Trim() | Should Be 'static-seed-publication-envelope-fields'
    }
    It 'rejects absent, empty, oversized or argument-bearing seed input before starting a child' {
        {Invoke-VerifiedNode -Tool seed-admission} | Should Throw 'Invalid seed-admission input'
        {Invoke-VerifiedNode -Tool seed-admission -InputBytes ([byte[]]::new(0))} | Should Throw 'Invalid seed-admission input'
        {Invoke-VerifiedNode -Tool seed-admission -InputBytes ([byte[]]::new(3145729))} | Should Throw 'Invalid seed-admission input'
        {Invoke-VerifiedNode -Tool seed-admission -Arguments @('unexpected') -InputBytes ([byte[]](1))} | Should Throw 'Invalid seed-admission input'
        {Invoke-VerifiedNode -Tool manifest -Arguments @('a'*40) -InputBytes ([byte[]](1))} | Should Throw 'does not accept stdin'
        {Invoke-VerifiedNode -Tool source-authority -InputBytes ([byte[]](1))} | Should Throw 'does not accept stdin'
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

# This fixture exercises committed-byte plumbing; the actual qualification validator has its separate real semantic tests.
Describe 'Committed seed metadata custody' {
    BeforeEach {
        $script:seedProcessMode='pass'
        $script:seedGitFiles=@{
            'deploy/static-seed-namespace.v1.json'=[Text.Encoding]::UTF8.GetBytes("{`"schema`":`"minimodel.static-seed-namespace/1`",`"descriptorBase64`":`"e30K`"}`n")
            'catalog/compositions.v1.json'=[Text.Encoding]::UTF8.GetBytes('{"entries":[{"browser_admission":{"route":"/test-only/admission.json"},"qualification":{"receipt_route":"/test-only/qualification.json"}}]}')
            'catalog/peer-announcements.v1.json'=[Text.Encoding]::UTF8.GetBytes('{}')
            'test-only/admission.json'=[Text.Encoding]::UTF8.GetBytes('{"fixture":"admission"}')
            'test-only/qualification.json'=[Text.Encoding]::UTF8.GetBytes('{"fixture":"qualification"}')
        }
        Mock Get-CustodyGitBlobBytes {
            param($RepositoryRoot,$Commit,$RelativePath,$MaximumBytes)
            if (-not $script:seedGitFiles.ContainsKey($RelativePath)) {throw 'Unexpected committed metadata route.'}
            if($script:seedGitFiles[$RelativePath].Length -gt $MaximumBytes){throw 'Fixture read exceeds bound.'}
            return ,([byte[]]$script:seedGitFiles[$RelativePath].Clone())
        }
        Mock Invoke-VerifiedNode {
            param($Tool,$InputBytes)
            $Tool | Should Be 'seed-admission'
            $text=[Text.Encoding]::UTF8.GetString($InputBytes)
            $text.EndsWith("`n") | Should Be $true
            $script:seedEnvelope=$text|ConvertFrom-Json
            $metadata=[ordered]@{}
            foreach($property in $script:seedEnvelope.PSObject.Properties){
                $bytes=[Convert]::FromBase64String($property.Value)
                $metadata[$property.Name]=[ordered]@{bytes=$bytes.Length;sha256=(Get-CustodyByteSha256 -Bytes $bytes)}
            }
            $verdict=[ordered]@{schema='minimodel.qualified-static-seed-plan/1';status='metadata-admitted-payload-and-host-unverified';publicationAllowed=$false;metadata=$metadata}
            if($script:seedProcessMode -ceq 'shape'){$verdict.publicationAllowed=$true}
            if($script:seedProcessMode -ceq 'bytes'){$verdict.metadata.admission.sha256='0'*64}
            return [pscustomobject]@{ExitCode=$(if($script:seedProcessMode -ceq 'exit'){1}else{0});Stdout=($verdict|ConvertTo-Json -Depth 6 -Compress);Stderr=$(if($script:seedProcessMode -ceq 'stderr'){'unexpected'}else{''})}
        }
    }
    It 'binds five exact immutable inputs and both local routes to one observed commit' {
        $record=Get-StaticSeedPublicationInput -Commit ('a'*40)
        $record.Commit | Should Be ('a'*40)
        $record.RegistrySha256 | Should Be (Get-CustodyByteSha256 -Bytes $script:seedGitFiles['deploy/static-seed-namespace.v1.json'])
        ($record.Inputs.Keys -join ',') | Should Be 'descriptor,catalog,peerFeed,admission,qualificationReceipt'
        [Text.Encoding]::UTF8.GetString($record.Inputs.descriptor) | Should Be "{}`n"
        Assert-MockCalled Get-CustodyGitBlobBytes -Times 5 -Exactly -Scope It -ParameterFilter {$Commit -ceq ('a'*40)}
        $script:seedGitFiles['test-only/admission.json'][0]=0
        [Text.Encoding]::UTF8.GetString($record.Inputs.admission) | Should Be '{"fixture":"admission"}'
    }
    It 'rejects a missing registry descriptor and noncanonical registry before metadata admission' {
        $script:seedGitFiles['deploy/static-seed-namespace.v1.json']=[Text.Encoding]::UTF8.GetBytes('{"schema":"minimodel.static-seed-namespace/1","descriptorBase64":null}')
        {Get-StaticSeedPublicationInput -Commit ('a'*40)} | Should Throw 'No exact static seed descriptor'
        $script:seedGitFiles['deploy/static-seed-namespace.v1.json']=[Text.Encoding]::UTF8.GetBytes('{"schema":"minimodel.static-seed-namespace/1","descriptorBase64":"e30K"}')
        {Get-StaticSeedPublicationInput -Commit ('a'*40)} | Should Throw 'canonical descriptor bytes'
        Assert-MockCalled Invoke-VerifiedNode -Times 0 -Exactly -Scope It
    }
    It 'rejects unsafe metadata routes before reading them from Git' {
        foreach($route in @('https://other.invalid/file','//other.invalid/file','/../outside','/file?query=1')){
            $script:seedGitFiles['catalog/compositions.v1.json']=[Text.Encoding]::UTF8.GetBytes(('{"entries":[{"browser_admission":{"route":"'+$route+'"},"qualification":{"receipt_route":"/test-only/qualification.json"}}]}'))
            {Get-StaticSeedPublicationInput -Commit ('a'*40)} | Should Throw
        }
        Assert-MockCalled Invoke-VerifiedNode -Times 0 -Exactly -Scope It
    }
    It 'preserves actual validator failure and rejects changed result shape or consumed identity' {
        foreach($mode in @('exit','stderr','shape','bytes')){
            $script:seedProcessMode=$mode
            {Get-StaticSeedPublicationInput -Commit ('a'*40)} | Should Throw
        }
    }
}

# Gate responses are mocked at the verified process boundary; no executable or source copy is written.
function New-TestSeedTransactionFixture {
    $body=[byte[]](1,2,3);$digest=Get-CustodyByteSha256 $body
    $directory='tinyrustlm/direct-composition/v1/compositions/sha256:'+('a'*64)+'/members'
    $names=@('model.slm2','tokenizer.tokenizer2','template.template2','sampling.sampling2','prompt.prompt2','composition.acg2')
    $files=@($names|ForEach-Object {[pscustomobject]@{name=$_;remotePath="$directory/$_";url="https://minimodel.org/$directory/$_";bytes=3;sha256=$digest}})
    $configuration="Options -Indexes`n";$configurationBytes=[Text.Encoding]::UTF8.GetBytes($configuration)
    $plan=[pscustomobject]@{schema='minimodel.static-seed-preparation.v1';origin='https://minimodel.org';files=$files;remoteDirectory=$directory;
        configuration=[pscustomobject]@{remotePath="$directory/.htaccess";text=$configuration;bytes=$configurationBytes.Length;sha256=(Get-CustodyByteSha256 $configurationBytes)}}
    $inputRecord=[pscustomobject]@{Commit=('a'*40);RegistrySha256=('b'*64);Admission=[pscustomobject]@{plan=$plan;metadata=[pscustomobject]@{descriptor=[pscustomobject]@{bytes=3;sha256=$digest}};
        compositionId=('sha256:'+('a'*64));pieceSetId=('sha256:'+('b'*64));qualificationReceiptId='fixture-only'}}
    $sourceSet=[pscustomobject]@{Members=@($names|ForEach-Object {[pscustomobject]@{Name=$_;Bytes=3;Sha256=$digest;Stream=[IO.MemoryStream]::new($body,$false)}})}
    return [pscustomobject]@{Input=$inputRecord;Sources=$sourceSet;Plan=$plan}
}

Describe 'Exact seed transaction input assembly' {
    BeforeEach {$script:seedFixture=New-TestSeedTransactionFixture;$script:testTransfer=$null}
    AfterEach {if($script:testTransfer){$script:testTransfer.ConfigurationStream.Dispose()};Close-StaticSeedSourceSet $script:seedFixture.Sources}
    It 'binds exactly seven paths and retains the six already verified handles' {
        $script:testTransfer=New-StaticSeedTransferSet -InputRecord $script:seedFixture.Input -SourceSet $script:seedFixture.Sources -OperationId ('1'*32)
        $script:testTransfer.Files.Count | Should Be 7
        $script:testTransfer.BindingSha256.Length | Should Be 64
        for($index=0;$index -lt 6;$index++){[object]::ReferenceEquals($script:testTransfer.Files[$index].stream,$script:seedFixture.Sources.Members[$index].Stream) | Should Be $true}
        (Get-CustodyByteSha256 $script:testTransfer.ConfigurationStream.ToArray()) | Should Be $script:seedFixture.Plan.configuration.sha256
    }
    It 'rejects substituted source, configuration, origin and namespace before transport' {
        $member=$script:seedFixture.Sources.Members[0]
        $member.Sha256='0'*64
        {New-StaticSeedTransferSet -InputRecord $script:seedFixture.Input -SourceSet $script:seedFixture.Sources -OperationId ('1'*32)} | Should Throw 'source and descriptor'
        $member.Sha256=$script:seedFixture.Plan.files[0].sha256
        $script:seedFixture.Plan.configuration.text='different'
        {New-StaticSeedTransferSet -InputRecord $script:seedFixture.Input -SourceSet $script:seedFixture.Sources -OperationId ('1'*32)} | Should Throw 'configuration identity'
        $script:seedFixture.Plan.origin='https://other.invalid'
        {New-StaticSeedTransferSet -InputRecord $script:seedFixture.Input -SourceSet $script:seedFixture.Sources -OperationId ('1'*32)} | Should Throw 'exact admitted'
    }
}

Describe 'Staged seed publication with in-memory transport' {
    BeforeEach {
        $script:seedFixture=New-TestSeedTransactionFixture
        $script:seedOperationId='1'*32
        $script:testTransfer=New-StaticSeedTransferSet -InputRecord $script:seedFixture.Input -SourceSet $script:seedFixture.Sources -OperationId $script:seedOperationId
        $script:seedFilePaths=$script:testTransfer.FilePaths;$script:seedTransferFiles=$script:testTransfer.Files
        $script:seedJournal=[Collections.Generic.List[object]]::new()
        $script:seedDirectories=@{''=[Collections.Generic.List[string]]::new()};$script:seedRemote=@{}
        $script:seedTrace=[Collections.Generic.List[string]]::new();$script:seedFailStage=0;$script:seedUploads=0
        Mock Assert-RemoteLeaseOwned {$script:seedTrace.Add('lease')}
        Mock Get-RemoteDirectoryEntries {
            param($LogicalPath)
            if(-not $script:seedDirectories.ContainsKey($LogicalPath)){throw 'Fixture directory absent.'}
            return $script:seedDirectories[$LogicalPath].ToArray()
        }
        Mock New-FtpRequest {
            param($LogicalPath,$Method,$Mutating)
            if($Method -cne 'MKD' -or -not $Mutating){throw 'Unexpected fixture FTP method.'}
            $parts=Get-RemoteParentAndLeaf $LogicalPath
            $script:seedDirectories[$parts.Parent].Add($parts.Leaf)
            $script:seedDirectories[$LogicalPath]=[Collections.Generic.List[string]]::new()
            $reply=[pscustomobject]@{};$reply|Add-Member ScriptMethod Dispose {}
            $request=[pscustomobject]@{Reply=$reply};$request|Add-Member ScriptMethod GetResponse {return $this.Reply}
            return $request
        }
        Mock Get-RemoteFileObservation {
            param($LogicalPath)
            if($script:seedRemote.ContainsKey($LogicalPath)){return [pscustomobject]@{State='present';Size=$script:seedRemote[$LogicalPath].Length}}
            return [pscustomobject]@{State='absent';Size=$null}
        }
        Mock Send-RemoteSeedStream {
            param($Source,$LogicalPath)
            $script:seedUploads++;$script:seedTrace.Add('upload:'+($LogicalPath.Substring($LogicalPath.LastIndexOf('/')+1)))
            if($script:seedFailStage -eq $script:seedUploads){throw 'Synthetic upload response lost.'}
            $file=Get-RegisteredSeedTransferFile -LogicalPath $LogicalPath -RequireStage
            $Source.Position=0;$target=[IO.MemoryStream]::new()
            try {$null=Copy-StaticSeedVerifiedStream -Source $Source -Destination $target -ExpectedBytes $file.bytes -ExpectedSha256 $file.sha256;$script:seedRemote[$LogicalPath]=$target.ToArray()}
            finally {$target.Dispose()}
            $parts=Get-RemoteParentAndLeaf $LogicalPath;$script:seedDirectories[$parts.Parent].Add($parts.Leaf)
        }
        Mock Assert-RemoteSeedStreamIdentity {
            param($LogicalPath)
            $file=Get-RegisteredSeedTransferFile $LogicalPath
            if(-not $script:seedRemote.ContainsKey($LogicalPath) -or (Get-CustodyByteSha256 $script:seedRemote[$LogicalPath]) -cne $file.sha256){throw 'Fixture remote identity differs.'}
            $script:seedTrace.Add('verify:'+($LogicalPath.Substring($LogicalPath.LastIndexOf('/')+1)))
        }
        Mock Rename-RemoteFile {
            param($LogicalPath,$NewLeafName)
            $parts=Get-RemoteParentAndLeaf $LogicalPath;$target=$parts.Parent+'/'+$NewLeafName
            if($script:seedRemote.ContainsKey($target)){throw 'Fixture target occupied.'}
            $script:seedRemote[$target]=$script:seedRemote[$LogicalPath];$script:seedRemote.Remove($LogicalPath)
            $null=$script:seedDirectories[$parts.Parent].Remove($parts.Leaf);$script:seedDirectories[$parts.Parent].Add($NewLeafName)
            $script:seedTrace.Add('rename:'+$NewLeafName)
        }
        Mock Assert-PublicStaticSeedMembers {$script:seedTrace.Add('https')}
    }
    AfterEach {
        $script:testTransfer.ConfigurationStream.Dispose();Close-StaticSeedSourceSet $script:seedFixture.Sources
        $script:seedFilePaths=@();$script:seedTransferFiles=@();$script:seedOperationId=''
    }
    It 'verifies configuration first, all six stages before promotion, and the final exact closure' {
        $result=Invoke-StaticSeedMemberTransaction -Plan $script:seedFixture.Plan -Journal $script:seedJournal
        $result | Should Be 'installed-and-http-verified'
        $script:seedUploads | Should Be 7
        $trace=$script:seedTrace.ToArray()
        [Array]::IndexOf($trace,'rename:.htaccess') | Should BeLessThan ([Array]::IndexOf($trace,'upload:model.slm2.seed-'+('1'*32)+'.tmp'))
        [Array]::IndexOf($trace,'verify:composition.acg2.seed-'+('1'*32)+'.tmp') | Should BeLessThan ([Array]::IndexOf($trace,'rename:model.slm2'))
        $script:seedRemote.Count | Should Be 7
        (Get-StaticSeedNamespaceState $script:seedFixture.Plan) | Should Be 'complete'
        Assert-MockCalled Assert-PublicStaticSeedMembers -Times 1 -Exactly -Scope It
        $script:seedFixture.Sources.Members[0].Stream.CanRead | Should Be $true
    }
    It 're-verifies a complete existing namespace without overwriting or uploading it' {
        $null=Invoke-StaticSeedMemberTransaction -Plan $script:seedFixture.Plan -Journal $script:seedJournal
        $script:seedUploads=0
        (Invoke-StaticSeedMemberTransaction -Plan $script:seedFixture.Plan -Journal $script:seedJournal) | Should Be 'already-present-http-verified'
        $script:seedUploads | Should Be 0
        Assert-MockCalled Assert-PublicStaticSeedMembers -Times 2 -Exactly -Scope It
    }
    It 'preserves a failed stage and never promotes model bytes or calls HTTP after an upload failure' {
        $script:seedFailStage=4
        {Invoke-StaticSeedMemberTransaction -Plan $script:seedFixture.Plan -Journal $script:seedJournal} | Should Throw 'response lost'
        @($script:seedTrace|Where-Object {$_ -like 'rename:*' -and $_ -cne 'rename:.htaccess'}).Count | Should Be 0
        Assert-MockCalled Assert-PublicStaticSeedMembers -Times 0 -Exactly -Scope It
        {Get-StaticSeedNamespaceState $script:seedFixture.Plan} | Should Throw 'partial or contains unowned'
        $script:seedJournal[-1].step | Should Be 'member-stage-attempted'
    }
    It 'rejects unknown leaves in an otherwise complete namespace' {
        $null=Invoke-StaticSeedMemberTransaction -Plan $script:seedFixture.Plan -Journal $script:seedJournal
        $script:seedDirectories[$script:seedFixture.Plan.remoteDirectory].Add('unowned.txt')
        {Get-StaticSeedNamespaceState $script:seedFixture.Plan} | Should Throw 'partial or contains unowned'
    }
}

Describe 'Seed HTTP request and lifetime orchestration' {
    BeforeEach {
        $script:seedFixture=New-TestSeedTransactionFixture
        $script:httpRequests=[Collections.Generic.List[object]]::new();$script:httpBodies=[Collections.Generic.List[IO.Stream]]::new()
        $script:httpReplies=[Collections.Generic.Queue[Net.Http.HttpResponseMessage]]::new();$script:httpSent=0;$script:httpFailure=$false
        foreach($file in $script:seedFixture.Plan.files) {
            $file|Add-Member NoteProperty expectedHeaders ([pscustomobject]@{'content-type'='application/octet-stream';'content-length'='3';'content-encoding'=$null;
                'access-control-allow-origin'='https://tinyrustlm.com';'x-tinyrustlm-composition-id'=('sha256:'+('a'*64));'x-tinyrustlm-piece-set-id'=('sha256:'+('b'*64));
                'x-tinyrustlm-member-name'=$file.name;'x-tinyrustlm-member-id'=('sha256:'+$file.sha256)})
            foreach($method in @('GET','HEAD')) {
                $body=if($method -ceq 'GET'){,[byte[]](1,2,3)}else{,[byte[]]::new(0)}
                $stream=[IO.MemoryStream]::new($body,$false);$script:httpBodies.Add($stream)
                $reply=[Net.Http.HttpResponseMessage]::new([Net.HttpStatusCode]::OK);$reply.Content=[Net.Http.StreamContent]::new($stream)
                foreach($property in $file.expectedHeaders.PSObject.Properties) {
                    if($null -eq $property.Value){continue}
                    $headers=if($property.Name.StartsWith('content-')){,$reply.Content.Headers}else{,$reply.Headers}
                    $null=$headers.TryAddWithoutValidation($property.Name,[string]$property.Value)
                }
                $script:httpReplies.Enqueue($reply)
            }
        }
        $script:httpClient=[pscustomobject]@{Disposed=$false}
        $script:httpClient|Add-Member ScriptMethod Dispose {$this.Disposed=$true}
        $script:httpClient|Add-Member ScriptMethod SendAsync {
            param($Request,$Completion,$Token)
            $script:httpSent++
            $script:httpRequests.Add([pscustomobject]@{method=$Request.Method.Method;url=$Request.RequestUri.AbsoluteUri;completion=$Completion;cancelable=$Token.CanBeCanceled;
                origin=(@($Request.Headers.GetValues('Origin')) -join ',');encoding=($Request.Headers.AcceptEncoding.ToString())})
            $reply=$script:httpReplies.Dequeue()
            if($script:httpFailure){$reply.StatusCode=[Net.HttpStatusCode]::Redirect}
            $task=[pscustomobject]@{Reply=$reply}
            $task|Add-Member ScriptMethod GetAwaiter {return $this}
            $task|Add-Member ScriptMethod GetResult {return $this.Reply}
            return $task
        }
        Mock New-StaticSeedHttpClient {return $script:httpClient}
    }
    AfterEach {
        while($script:httpReplies.Count -gt 0){$script:httpReplies.Dequeue().Dispose()}
        foreach($body in $script:httpBodies){$body.Dispose()};Close-StaticSeedSourceSet $script:seedFixture.Sources
    }
    It 'sends the exact twelve GET/HEAD requests with streaming completion and disposes all responses' {
        $journal=[Collections.Generic.List[object]]::new()
        Assert-PublicStaticSeedMembers -Plan $script:seedFixture.Plan -Journal $journal
        $script:httpSent | Should Be 12
        $journal.Count | Should Be 12
        for($index=0;$index -lt 12;$index++) {
            $request=$script:httpRequests[$index]
            $request.method | Should Be $(if($index % 2 -eq 0){'GET'}else{'HEAD'})
            $request.url | Should Be $script:seedFixture.Plan.files[[Math]::Floor($index/2)].url
            $request.completion | Should Be ([Net.Http.HttpCompletionOption]::ResponseHeadersRead)
            $request.cancelable | Should Be $true
            $request.origin | Should Be 'https://tinyrustlm.com'
            $request.encoding | Should Be 'identity'
            $script:httpBodies[$index].CanRead | Should Be $false
        }
        $script:httpClient.Disposed | Should Be $true
    }
    It 'stops at a rejected response and disposes both response and client without retry' {
        $script:httpFailure=$true;$journal=[Collections.Generic.List[object]]::new()
        {Assert-PublicStaticSeedMembers -Plan $script:seedFixture.Plan -Journal $journal} | Should Throw 'complete 200'
        $script:httpSent | Should Be 1
        $journal.Count | Should Be 0
        $script:httpBodies[0].CanRead | Should Be $false
        $script:httpClient.Disposed | Should Be $true
    }
}

Describe 'Qualified seed entrypoint sequencing and exact failure receipts' {
    BeforeEach {
        $script:seedFixture=New-TestSeedTransactionFixture
        $script:sourceAuthorityAdmitted=$true;$script:lease=$null;$script:seedReadCount=0;$script:seedEntrypointFailure=''
        $script:seedAuthorityRecord=[pscustomobject]@{Sha256=('c'*64);Value=(Get-Content (Join-Path $repositoryRoot 'deploy/publication-authority.v1.json') -Raw|ConvertFrom-Json)}
        $script:FtpExportPath='not-read-fixture'
        Mock Get-RepositoryObservation {return [pscustomobject]@{head=('a'*40)}}
        Mock Assert-CustodyRepositoryIdentity {}
        Mock Get-StaticSeedPublicationInput {
            $script:seedReadCount++
            if($script:seedEntrypointFailure -ceq 'qualification' -or ($script:seedEntrypointFailure -ceq 'expiry' -and $script:seedReadCount -eq 2)){throw 'Synthetic admission rejected.'}
            return $script:seedFixture.Input
        }
        Mock Open-StaticSeedSourceSet {
            if($script:seedEntrypointFailure -ceq 'source'){throw 'Synthetic source hash mismatch.'}
            return $script:seedFixture.Sources
        }
        Mock Import-MiniModelCredentialModule {}
        Mock Get-CredentialProfile {return [pscustomobject]@{profile='fixture';host='fixture.invalid';port=21;user='fixture';password='not-a-real-credential'}}
        Mock Assert-CustodyTransportAuthority {}
        Mock Assert-RemoteExactIdentity {if($script:seedEntrypointFailure -ceq 'sentinel'){throw 'Synthetic wrong target.'}}
        Mock Get-StaticSeedNamespaceState {return 'absent'}
        Mock New-RemoteDeploymentLease {
            if($script:seedEntrypointFailure -ceq 'lease'){throw 'Synthetic lease response lost.'}
            $script:lease=[pscustomobject]@{OperationId=$script:seedOperationId}
            return $script:lease
        }
        Mock Invoke-StaticSeedMemberTransaction {
            param($Plan,$Journal)
            $Journal.Add([ordered]@{step='fixture-transaction';path=$Plan.remoteDirectory})
            if($script:seedEntrypointFailure -ceq 'transfer'){throw 'Synthetic ambiguous upload.'}
            return 'installed-and-http-verified'
        }
        Mock Remove-RemoteDeploymentLease {$script:lease=$null}
    }
    AfterEach {Close-StaticSeedSourceSet $script:seedFixture.Sources;$script:lease=$null;$script:credential=$null}
    It 'closes retained handles and releases its lease only after fresh admission and HTTP transaction success' {
        $receipt=Invoke-QualifiedStaticSeedPublication -AuthorityRecord $script:seedAuthorityRecord -SourceDirectory 'D:\LLMs\TinyRustLM\fixture-only'
        $receipt.status | Should Be 'installed-and-http-verified'
        $receipt.seed_published | Should Be $true
        $receipt.discovery_published | Should Be $false
        $receipt.website_published | Should Be $false
        $receipt.browser_origin_readback_verified | Should Be $false
        $receipt.lease_retained_or_creation_ambiguous | Should Be $false
        $script:seedReadCount | Should Be 2
        $script:seedFixture.Sources.Members[0].Stream.CanRead | Should Be $false
        $script:seedFilePaths.Count | Should Be 0
        Assert-MockCalled Remove-RemoteDeploymentLease -Times 1 -Exactly -Scope It
    }
    It 'rejects absent qualification before source or credential access' {
        $script:seedEntrypointFailure='qualification'
        {Invoke-QualifiedStaticSeedPublication -AuthorityRecord $script:seedAuthorityRecord -SourceDirectory 'D:\LLMs\TinyRustLM\fixture-only'} | Should Throw 'admission rejected'
        Assert-MockCalled Open-StaticSeedSourceSet -Times 0 -Exactly -Scope It
        Assert-MockCalled Get-CredentialProfile -Times 0 -Exactly -Scope It
    }
    It 'records source failure without opening credentials or touching the server' {
        $script:seedEntrypointFailure='source'
        $receipt=Invoke-QualifiedStaticSeedPublication -AuthorityRecord $script:seedAuthorityRecord -SourceDirectory 'D:\LLMs\TinyRustLM\fixture-only'
        $receipt.status | Should Be 'failed'
        $receipt.lease_retained_or_creation_ambiguous | Should Be $false
        Assert-MockCalled Get-CredentialProfile -Times 0 -Exactly -Scope It
        Assert-MockCalled New-RemoteDeploymentLease -Times 0 -Exactly -Scope It
    }
    It 'rejects a different target marker before acquiring a lease' {
        $script:seedEntrypointFailure='sentinel'
        $receipt=Invoke-QualifiedStaticSeedPublication -AuthorityRecord $script:seedAuthorityRecord -SourceDirectory 'D:\LLMs\TinyRustLM\fixture-only'
        $receipt.status | Should Be 'failed'
        $receipt.lease_retained_or_creation_ambiguous | Should Be $false
        Assert-MockCalled New-RemoteDeploymentLease -Times 0 -Exactly -Scope It
        $script:seedFixture.Sources.Members[0].Stream.CanRead | Should Be $false
    }
    It 'retains failed or expired transactions without retry, cleanup, or a false publication claim' {
        foreach($failure in @('transfer','expiry','lease')) {
            # Each invocation needs new caller-owned handles; no source files are used.
            Close-StaticSeedSourceSet $script:seedFixture.Sources
            $script:seedFixture=New-TestSeedTransactionFixture;$script:seedReadCount=0;$script:seedEntrypointFailure=$failure
            $receipt=Invoke-QualifiedStaticSeedPublication -AuthorityRecord $script:seedAuthorityRecord -SourceDirectory 'D:\LLMs\TinyRustLM\fixture-only'
            $receipt.status | Should Be 'failed'
            $receipt.seed_published | Should Be $false
            $receipt.lease_retained_or_creation_ambiguous | Should Be $true
            $receipt.recovery | Should Be 'retained-for-exact-ownership-review'
            $script:seedFixture.Sources.Members[0].Stream.CanRead | Should Be $false
        }
        Assert-MockCalled Remove-RemoteDeploymentLease -Times 0 -Exactly -Scope It
    }
    It 'rejects premature or unbound execution' {
        $script:sourceAuthorityAdmitted=$false
        {Invoke-QualifiedStaticSeedPublication -AuthorityRecord $script:seedAuthorityRecord -SourceDirectory 'D:\LLMs\TinyRustLM\fixture-only'} | Should Throw 'source authority'
        $script:sourceAuthorityAdmitted=$true;$script:seedAuthorityRecord.Value.transport.status='unbound'
        {Invoke-QualifiedStaticSeedPublication -AuthorityRecord $script:seedAuthorityRecord -SourceDirectory 'D:\LLMs\TinyRustLM\fixture-only'} | Should Throw 'bound transport'
        Assert-MockCalled Get-CredentialProfile -Times 0 -Exactly -Scope It
    }
}

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
