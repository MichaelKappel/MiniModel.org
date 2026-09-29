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
}
