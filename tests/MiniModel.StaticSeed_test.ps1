$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot '../tools/MiniModel.StaticSeed.psm1') -Force
Import-Module (Join-Path $PSScriptRoot '../tools/MiniModel.DeploymentCustody.psm1') -Force

Describe 'Bounded static seed stream identity' {
    It 'copies more than one buffer and leaves caller streams open' {
        $bytes=[byte[]]::new(131079)
        for($i=0;$i -lt $bytes.Length;$i++){$bytes[$i]=[byte]($i % 251)}
        $source=[IO.MemoryStream]::new($bytes,$false);$target=[IO.MemoryStream]::new()
        try {
            $digest=Get-CustodyByteSha256 $bytes
            $result=Copy-StaticSeedVerifiedStream -Source $source -Destination $target -ExpectedBytes $bytes.Length -ExpectedSha256 $digest
            $result.Bytes | Should Be $bytes.Length
            $result.Sha256 | Should Be $digest
            (Get-CustodyByteSha256 $target.ToArray()) | Should Be $digest
            $source.CanRead | Should Be $true
            $target.CanWrite | Should Be $true
        } finally {$source.Dispose();$target.Dispose()}
    }
    It 'hashes without allocating a destination or accepting extra bytes' {
        $source=[IO.MemoryStream]::new([byte[]](1,2,3),$false)
        try {
            $digest=Get-CustodyByteSha256 ([byte[]](1,2,3))
            (Copy-StaticSeedVerifiedStream -Source $source -ExpectedBytes 3 -ExpectedSha256 $digest).Bytes | Should Be 3
            $source.Position=0
            {Copy-StaticSeedVerifiedStream -Source $source -ExpectedBytes 2 -ExpectedSha256 $digest} | Should Throw 'exceeded its exact length'
        } finally {$source.Dispose()}
    }
    It 'rejects truncation, int32-overflow lengths and a changed body' {
        foreach($expected in @([int64]4,[int64]2147483648)){
            $source=[IO.MemoryStream]::new([byte[]](1,2,3),$false)
            try { {Copy-StaticSeedVerifiedStream -Source $source -ExpectedBytes $expected -ExpectedSha256 ('a'*64)} | Should Throw 'ended before its exact length' }
            finally {$source.Dispose()}
        }
        $source=[IO.MemoryStream]::new([byte[]](1,2,3),$false)
        try {{Copy-StaticSeedVerifiedStream -Source $source -ExpectedBytes 3 -ExpectedSha256 ('a'*64)} | Should Throw 'hash differs'}
        finally {$source.Dispose()}
    }
    It 'rejects invalid identity bounds and unsupported stream directions before copying' {
        $source=[IO.MemoryStream]::new([byte[]](1,2,3),$false)
        $readonly=[IO.MemoryStream]::new([byte[]](4,5,6),$false)
        try {
            foreach($length in @([int64]0,[int64]-1,[int64]68719476737)){
                {Copy-StaticSeedVerifiedStream -Source $source -ExpectedBytes $length -ExpectedSha256 ('a'*64)} | Should Throw 'contract is invalid'
            }
            foreach($hash in @(('a'*63),('A'*64),(('a'*63)+'z'))){
                {Copy-StaticSeedVerifiedStream -Source $source -ExpectedBytes 3 -ExpectedSha256 $hash} | Should Throw 'contract is invalid'
            }
            {Copy-StaticSeedVerifiedStream -Source $source -Destination $readonly -ExpectedBytes 3 -ExpectedSha256 ('a'*64)} | Should Throw 'contract is invalid'
            {Copy-StaticSeedVerifiedStream -Source $source -Destination $source -ExpectedBytes 3 -ExpectedSha256 ('a'*64)} | Should Throw 'contract is invalid'
            $source.Position | Should Be 0
            $source.Dispose()
            {Copy-StaticSeedVerifiedStream -Source $source -ExpectedBytes 3 -ExpectedSha256 ('a'*64)} | Should Throw 'contract is invalid'
        } finally {$source.Dispose();$readonly.Dispose()}
    }
}

Describe 'Actual HTTP message seed readback without a listener' {
    BeforeEach {
        $script:httpBody=[byte[]](1,2,3)
        $script:httpMember=[pscustomobject]@{bytes=3;sha256=(Get-CustodyByteSha256 $script:httpBody);expectedHeaders=[pscustomobject]@{
            'content-type'='application/octet-stream';'content-length'='3';'content-encoding'=$null;
            'access-control-allow-origin'='https://tinyrustlm.com';'x-tinyrustlm-composition-id'=('sha256:'+('a'*64));
            'x-tinyrustlm-piece-set-id'=('sha256:'+('b'*64));'x-tinyrustlm-member-name'='model.slm2';
            'x-tinyrustlm-member-id'=('sha256:'+(Get-CustodyByteSha256 $script:httpBody))}}
        $script:httpResponse=[Net.Http.HttpResponseMessage]::new([Net.HttpStatusCode]::OK)
        $script:httpResponse.Content=[Net.Http.StreamContent]::new([IO.MemoryStream]::new($script:httpBody,$false))
        foreach($property in $script:httpMember.expectedHeaders.PSObject.Properties) {
            if($null -eq $property.Value){continue}
            $headers=if($property.Name.StartsWith('content-')){,$script:httpResponse.Content.Headers}else{,$script:httpResponse.Headers}
            $null=$headers.TryAddWithoutValidation($property.Name,[string]$property.Value)
        }
    }
    AfterEach {$script:httpResponse.Dispose()}
    It 'checks all headers, hashes the complete body and leaves response disposal to its caller' {
        $result=Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember
        $result.Bytes | Should Be 3
        $result.Sha256 | Should Be $script:httpMember.sha256
        $script:httpResponse.Content.ReadAsStreamAsync().Result.CanRead | Should Be $true
    }
    It 'checks a HEAD response without reading a nonexistent body' {
        $stream=$script:httpResponse.Content.ReadAsStreamAsync().Result
        Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember -HeadersOnly
        $stream.Position | Should Be 0
    }
    It 'rejects redirects and other successful status codes' {
        foreach($status in @(301,204,206,404)) {
            $script:httpResponse.StatusCode=[Net.HttpStatusCode]$status
            {Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember} | Should Throw 'complete 200'
        }
    }
    It 'rejects missing, changed and duplicate custom identity headers' {
        $name='X-TinyRustLM-Member-Name'
        $null=$script:httpResponse.Headers.Remove($name)
        {Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember} | Should Throw 'header identity'
        $null=$script:httpResponse.Headers.TryAddWithoutValidation($name,'MODEL.slm2')
        {Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember} | Should Throw 'header identity'
        $null=$script:httpResponse.Headers.Remove($name)
        $null=$script:httpResponse.Headers.TryAddWithoutValidation($name,[string[]]@('model.slm2','model.slm2'))
        {Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember} | Should Throw 'header identity'
    }
    It 'rejects encoding, credential CORS and chunked framing' {
        $null=$script:httpResponse.Content.Headers.TryAddWithoutValidation('Content-Encoding','gzip')
        {Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember} | Should Throw 'forbidden header'
        $null=$script:httpResponse.Content.Headers.Remove('Content-Encoding')
        foreach($name in @('Access-Control-Allow-Credentials','Transfer-Encoding')) {
            $null=$script:httpResponse.Headers.TryAddWithoutValidation($name,'true')
            {Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember} | Should Throw 'forbidden header'
            $null=$script:httpResponse.Headers.Remove($name)
        }
    }
    It 'rejects same-size changed bodies and truncated bodies despite matching headers' {
        $script:httpMember.sha256='0'*64
        {Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember} | Should Throw 'hash differs'
        {Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember} | Should Throw 'ended before'
    }
    It 'honors cancellation before body consumption without closing caller streams' {
        $cts=[Threading.CancellationTokenSource]::new()
        try {
            $cts.Cancel()
            {Assert-StaticSeedHttpMember -Response $script:httpResponse -Member $script:httpMember -CancellationToken $cts.Token} | Should Throw
            $stream=$script:httpResponse.Content.ReadAsStreamAsync().Result
            $stream.Position | Should Be 0
            {Copy-StaticSeedVerifiedStream -Source $stream -ExpectedBytes 3 -ExpectedSha256 $script:httpMember.sha256 -CancellationToken $cts.Token} | Should Throw
            $stream.CanRead | Should Be $true
        } finally {$cts.Dispose()}
    }
    It 'closes the owned HTTP response from a CLR deadline callback' {
        $stream=$script:httpResponse.Content.ReadAsStreamAsync().Result
        $cts=[Threading.CancellationTokenSource]::new()
        $registration=Register-StaticSeedResponseCancellation -Response $script:httpResponse -CancellationToken $cts.Token
        try {$cts.Cancel();$stream.CanRead | Should Be $false}
        finally {$registration.Dispose();$cts.Dispose()}
    }
}

InModuleScope MiniModel.StaticSeed {
    Describe 'Canonical retained seed source handles without filesystem payloads' {
        BeforeEach {
            $script:sourceBodies=[byte[]](1,2,3)
            $sha=[Security.Cryptography.SHA256]::Create()
            try {$digest=([BitConverter]::ToString($sha.ComputeHash($script:sourceBodies))).Replace('-','').ToLowerInvariant()}
            finally {$sha.Dispose()}
            $script:sourceMembers=@('model.slm2','tokenizer.tokenizer2','template.template2','sampling.sampling2','prompt.prompt2','composition.acg2')|ForEach-Object {[pscustomobject]@{name=$_;bytes=3;sha256=$digest}}
            $script:openedSources=[Collections.Generic.List[IO.Stream]]::new()
            $script:openedPaths=[Collections.Generic.List[string]]::new()
            $script:failSourceIndex=-1
            Mock Assert-StaticSeedPlainPath {}
            Mock Open-StaticSeedSourceFile {
                param($Path)
                if($script:openedSources.Count -eq $script:failSourceIndex){throw 'Synthetic canonical file open failure.'}
                $stream=[IO.MemoryStream]::new($script:sourceBodies,$false)
                $script:openedSources.Add($stream);$script:openedPaths.Add($Path)
                return $stream
            }
        }
        AfterEach {foreach($stream in $script:openedSources){$stream.Dispose()}}
        It 'hashes all six unchanged handles, rewinds them and releases only on caller close' {
            $source=Open-StaticSeedSourceSet -Directory 'D:\LLMs\TinyRustLM\fixture-only' -Members $script:sourceMembers
            $source.Members.Count | Should Be 6
            for($index=0;$index -lt 6;$index++){
                $member=$source.Members[$index]
                $member.Name | Should Be $script:sourceMembers[$index].name
                [object]::ReferenceEquals($member.Stream,$script:openedSources[$index]) | Should Be $true
                $member.Stream.CanRead | Should Be $true
                $member.Stream.Position | Should Be 0
                $script:openedPaths[$index] | Should Be ('D:\LLMs\TinyRustLM\fixture-only\'+$member.Name)
            }
            Close-StaticSeedSourceSet -SourceSet $source
            foreach($stream in $script:openedSources){$stream.CanRead | Should Be $false}
        }
        It 'rejects alternate stores, relative paths and unknown member names before opening anything' {
            foreach($directory in @('fixture','E:\Temp\fixture','D:\LLMs\TinyRustLM-other\fixture','X:\LLMS\fixture','D:\LLMs\TinyRustLM\fixture:stream')){
                {Open-StaticSeedSourceSet -Directory $directory -Members $script:sourceMembers} | Should Throw
            }
            $script:sourceMembers[1].name='../tokenizer.tokenizer2'
            {Open-StaticSeedSourceSet -Directory 'D:\LLMs\TinyRustLM\fixture-only' -Members $script:sourceMembers} | Should Throw 'member identity'
            Assert-MockCalled Open-StaticSeedSourceFile -Times 0 -Exactly -Scope It
        }
        It 'closes already opened handles on source-open, length or hash failure' {
            $script:failSourceIndex=3
            {Open-StaticSeedSourceSet -Directory 'D:\LLMs\TinyRustLM\fixture-only' -Members $script:sourceMembers} | Should Throw 'file open failure'
            $script:openedSources.Count | Should Be 3
            foreach($stream in $script:openedSources){$stream.CanRead | Should Be $false}
            $script:openedSources.Clear();$script:failSourceIndex=-1
            $script:sourceMembers[0].bytes=4
            {Open-StaticSeedSourceSet -Directory 'D:\LLMs\TinyRustLM\fixture-only' -Members $script:sourceMembers} | Should Throw 'different length'
            $script:openedSources[0].CanRead | Should Be $false
            $script:openedSources.Clear();$script:sourceMembers[0].bytes=3;$script:sourceMembers[2].sha256='a'*64
            {Open-StaticSeedSourceSet -Directory 'D:\LLMs\TinyRustLM\fixture-only' -Members $script:sourceMembers} | Should Throw 'hash differs'
            $script:openedSources.Count | Should Be 3
            foreach($stream in $script:openedSources){$stream.CanRead | Should Be $false}
        }
        It 'rejects incomplete closures and invalid numerical or digest identities before source probing' {
            {Open-StaticSeedSourceSet -Directory 'D:\LLMs\TinyRustLM\fixture-only' -Members @($script:sourceMembers[0])} | Should Throw 'exactly six'
            foreach($length in @([int64]0,[int64]68719476737)){
                $script:sourceMembers[0].bytes=$length
                {Open-StaticSeedSourceSet -Directory 'D:\LLMs\TinyRustLM\fixture-only' -Members $script:sourceMembers} | Should Throw 'member identity'
            }
            $script:sourceMembers[0].bytes=3;$script:sourceMembers[0].sha256=('a'*64)+"`n"
            {Open-StaticSeedSourceSet -Directory 'D:\LLMs\TinyRustLM\fixture-only' -Members $script:sourceMembers} | Should Throw 'member identity'
            Assert-MockCalled Assert-StaticSeedPlainPath -Times 0 -Exactly -Scope It
        }
    }
}

Describe 'Exact static seed remote namespace' {
    BeforeEach {
        $directory='tinyrustlm/direct-composition/v1/compositions/sha256:'+('a'*64)+'/members'
        $seedPaths=@('model.slm2','tokenizer.tokenizer2','template.template2','sampling.sampling2','prompt.prompt2','composition.acg2','.htaccess') | ForEach-Object {"$directory/$_"}
    }
    It 'admits exact registered files and explicit ancestors without changing website paths' {
        foreach($path in $seedPaths){(Assert-CustodyRemotePath -Path $path -SeedFilePaths $seedPaths) | Should Be $path}
        (Assert-CustodyRemotePath -Path $directory -SeedFilePaths $seedPaths -AllowAncestor) | Should Be $directory
        {Assert-CustodyRemotePath -Path $directory -SeedFilePaths $seedPaths} | Should Throw 'outside the registered'
        (Assert-CustodyRemotePath -Path 'catalog/compositions.v1.json') | Should Be 'catalog/compositions.v1.json'
        {Assert-CustodyRemotePath -Path '../catalog/compositions.v1.json'} | Should Throw 'Unsafe relative path'
    }
    It 'rejects unregistered colon paths, extra leaves, aliases and incomplete registration' {
        foreach($path in @($seedPaths[0],($seedPaths[0] -replace 'sha256:a','sha256:b'),"$directory/extra.bin","$directory/../model.slm2",'catalog:other/file')){
            {Assert-CustodyRemotePath -Path $path} | Should Throw 'outside the registered'
        }
        foreach($path in @("$directory/extra.bin",$seedPaths[0].ToUpperInvariant(),"$directory/../model.slm2")){
            {Assert-CustodyRemotePath -Path $path -SeedFilePaths $seedPaths -AllowAncestor} | Should Throw 'outside the registered'
        }
        {Assert-CustodyRemotePath -Path $seedPaths[0] -SeedFilePaths @($seedPaths[0])} | Should Throw 'namespace is invalid'
        $changed=@($seedPaths);$changed[6]="$directory/extra.bin"
        {Assert-CustodyRemotePath -Path $seedPaths[0] -SeedFilePaths $changed} | Should Throw 'exact member closure'
    }
    It 'requires every member and rejects partial or unknown remote trees' {
        $paths=@('index.html')+$seedPaths
        $tree=Get-CustodyExpectedDirectoryTree -FilePaths $paths -SeedFilePaths $seedPaths
        {Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $paths -ObservedTree $tree -SeedFilePaths $seedPaths} | Should Not Throw
        $null=$tree[$directory].Remove('model.slm2')
        {Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $paths -ObservedTree $tree -SeedFilePaths $seedPaths} | Should Throw 'overlay or missing'
        $null=$tree[$directory].Add('model.slm2');$null=$tree[$directory].Add('extra.bin')
        {Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $paths -ObservedTree $tree -SeedFilePaths $seedPaths} | Should Throw 'overlay or missing'
    }
    It 'admits only the seven exact staging leaves belonging to one operation' {
        $operation='1'*32
        $stages=@($seedPaths | ForEach-Object {"$_.seed-$operation.tmp"})
        foreach($stage in $stages){
            (Assert-CustodyRemotePath -Path $stage -SeedFilePaths $seedPaths -SeedOperationId $operation) | Should Be $stage
            {Assert-CustodyRemotePath -Path $stage -SeedFilePaths $seedPaths} | Should Throw 'outside the registered'
            {Assert-CustodyRemotePath -Path $stage -SeedFilePaths $seedPaths -SeedOperationId ('2'*32)} | Should Throw 'outside the registered'
        }
        foreach($bad in @(('1'*31),('G'*32),'../operation')){
            {Assert-CustodyRemotePath -Path $seedPaths[0] -SeedFilePaths $seedPaths -SeedOperationId $bad} | Should Throw 'operation identity'
        }
        {Assert-CustodyRemotePath -Path 'index.html' -SeedOperationId $operation} | Should Throw 'operation identity'
        {Assert-CustodyRemotePath -Path ($stages[0]+'.extra') -SeedFilePaths $seedPaths -SeedOperationId $operation} | Should Throw 'outside the registered'
        $tree=Get-CustodyExpectedDirectoryTree -FilePaths ($seedPaths+$stages) -SeedFilePaths $seedPaths -SeedOperationId $operation
        {Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths ($seedPaths+$stages) -ObservedTree $tree -SeedFilePaths $seedPaths -SeedOperationId $operation} | Should Not Throw
        {Get-CustodyExpectedDirectoryTree -FilePaths $stages -SeedFilePaths $seedPaths} | Should Throw 'outside the registered'
        $listing=[Text.Encoding]::UTF8.GetBytes(($stages | ForEach-Object {($_ -split '/')[-1]}) -join "`n")
        @(ConvertFrom-CustodyDirectoryListing -Bytes $listing -DirectoryPath $directory -SeedFilePaths $seedPaths -SeedOperationId $operation).Count | Should Be 7
        {ConvertFrom-CustodyDirectoryListing -Bytes $listing -DirectoryPath $directory -SeedFilePaths $seedPaths} | Should Throw 'outside the registered'
    }
}
