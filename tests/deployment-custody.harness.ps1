param([Parameter(Mandatory)][string]$Case)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Import-Module (Join-Path $repositoryRoot "tools\MiniModel.DeploymentCustody.psm1") -Force
$authorityRecord = Read-CustodyJsonFile `
    -Path (Join-Path $repositoryRoot "deploy\publication-authority.v1.json") `
    -MaximumBytes 131072 `
    -Label "publication authority"
$authority = $authorityRecord.Value

function Copy-JsonValue {
    param([Parameter(Mandatory)]$Value)
    return ($Value | ConvertTo-Json -Depth 64) | ConvertFrom-Json
}

function Assert-Throws {
    param(
        [Parameter(Mandatory)][scriptblock]$Action,
        [Parameter(Mandatory)][string]$Pattern
    )

    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -notmatch $Pattern) {
            throw "Unexpected rejection: $($_.Exception.Message)"
        }
        return
    }
    throw "Expected rejection matching: $Pattern"
}

function Get-GitBytes {
    param([Parameter(Mandatory)][string]$Commit, [Parameter(Mandatory)][string]$Path)
    return Get-CustodyGitBlobBytes -RepositoryRoot $repositoryRoot -Commit $Commit -RelativePath $Path
}

switch ($Case) {
    "repository-identity" {
        $head = "a" * 40
        $observed = [pscustomobject]@{
            canonical_root = $repositoryRoot
            root_has_reparse_point = $false
            branch = "master"
            upstream = "origin/master"
            remotes = @("origin")
            fetch_url = [string]$authority.repository.remote_url
            push_url = [string]$authority.repository.remote_url
            head = $head
            tracking_head = $head
            remote_head = $head
            status_count = 0
            source_is_ancestor = $true
            source_tree = [string]$authority.candidate.source_tree
            manifest_matches_head = $true
            authority_matches_head = $true
        }
        Assert-CustodyRepositoryIdentity -Authority $authority -Observed $observed -ExpectedRepositoryRoot $repositoryRoot

        $wrongMirror = Copy-JsonValue $observed
        $wrongMirror.fetch_url = "https://github.com/example/MiniModel.org.git"
        Assert-Throws -Action {
            Assert-CustodyRepositoryIdentity -Authority $authority -Observed $wrongMirror -ExpectedRepositoryRoot $repositoryRoot
        } -Pattern "exact clean canonical pushed"

        $wrongRoot = Copy-JsonValue $observed
        $wrongRoot.canonical_root = "E:\\Source\\MiniModel.org"
        Assert-Throws -Action {
            Assert-CustodyRepositoryIdentity -Authority $authority -Observed $wrongRoot -ExpectedRepositoryRoot $repositoryRoot
        } -Pattern "exact clean canonical pushed"

        $staleRemote = Copy-JsonValue $observed
        $staleRemote.remote_head = "b" * 40
        Assert-Throws -Action {
            Assert-CustodyRepositoryIdentity -Authority $authority -Observed $staleRemote -ExpectedRepositoryRoot $repositoryRoot
        } -Pattern "exact clean canonical pushed"
    }

    "transport-authority" {
        $bound = Copy-JsonValue $authority
        $bound.transport.status = "bound"
        $bound.transport.host = "publish.minimodel.example"
        $bound.transport.port = 21
        $bound.transport.remote_root = "public_html"
        $bound.transport.username_sha256 = Get-CustodyByteSha256 -Bytes ([Text.UTF8Encoding]::new($false, $true).GetBytes("publisher"))
        $observed = [pscustomobject]@{
            profile_name = "MiniModel.org"
            protocol = "ftps-explicit"
            host = "publish.minimodel.example"
            port = 21
            username = "publisher"
            remote_root = "public_html"
        }
        Assert-CustodyTransportAuthority -Authority $bound -Observed $observed

        foreach ($mutation in @(
            @{ name = "host"; value = "mirror.minimodel.example" },
            @{ name = "port"; value = 990 },
            @{ name = "username"; value = "other-publisher" },
            @{ name = "remote_root"; value = "other-root" },
            @{ name = "profile_name"; value = "Other profile" }
        )) {
            $wrong = Copy-JsonValue $observed
            $wrong.($mutation.name) = $mutation.value
            Assert-Throws -Action { Assert-CustodyTransportAuthority -Authority $bound -Observed $wrong } -Pattern "immutable publication transport tuple"
        }

        Assert-Throws -Action { Assert-CustodyTransportAuthority -Authority $authority -Observed $observed } -Pattern "not bound.*credential access is forbidden"
    }

    "mutation-order" {
        Assert-Throws -Action { Assert-CustodyMutationAllowed -Phase "read-only" -MutationKind local } -Pattern "forbidden"
        Assert-Throws -Action { Assert-CustodyMutationAllowed -Phase "read-only" -MutationKind remote } -Pattern "forbidden"
        Assert-CustodyMutationAllowed -Phase "public-admitted" -MutationKind local
        Assert-Throws -Action { Assert-CustodyMutationAllowed -Phase "public-admitted" -MutationKind remote } -Pattern "forbidden"
        Assert-CustodyMutationAllowed -Phase "transport-admitted" -MutationKind local
        Assert-CustodyMutationAllowed -Phase "transport-admitted" -MutationKind remote
    }

    "public-predecessor" {
        $publicCommit = "77d65bdfab168bdd12d3b0ba90fa9d3cc349e7ab"
        $candidateCommit = [string]$authority.candidate.source_commit
        $arguments = @{
            Authority = $authority
            PublicReleaseBytes = Get-GitBytes -Commit $publicCommit -Path ([string]$authority.predecessor.release_path)
            PublicHistoryBytes = Get-GitBytes -Commit $publicCommit -Path ([string]$authority.predecessor.history_path)
            PublicCatalogBytes = Get-GitBytes -Commit $publicCommit -Path ([string]$authority.public_invariants.composition_catalog.path)
            PublicPeerBytes = Get-GitBytes -Commit $publicCommit -Path ([string]$authority.public_invariants.peer_feed.path)
            CandidateReleaseBytes = Get-GitBytes -Commit $candidateCommit -Path ([string]$authority.predecessor.release_path)
            CandidateHistoryBytes = Get-GitBytes -Commit $candidateCommit -Path ([string]$authority.predecessor.history_path)
            CandidateCatalogBytes = Get-GitBytes -Commit $candidateCommit -Path ([string]$authority.public_invariants.composition_catalog.path)
            CandidatePeerBytes = Get-GitBytes -Commit $candidateCommit -Path ([string]$authority.public_invariants.peer_feed.path)
        }
        Assert-CustodyPublicPredecessor @arguments

        $staleRelease = [byte[]]$arguments.PublicReleaseBytes.Clone()
        $staleRelease[$staleRelease.Length - 2] = $staleRelease[$staleRelease.Length - 2] -bxor 1
        $staleArguments = @{} + $arguments
        $staleArguments.PublicReleaseBytes = $staleRelease
        Assert-Throws -Action { Assert-CustodyPublicPredecessor @staleArguments } -Pattern "not exact 1.1.1"

        $staleHistory = [byte[]]$arguments.PublicHistoryBytes.Clone()
        $staleHistory[32] = $staleHistory[32] -bxor 1
        $staleHistoryArguments = @{} + $arguments
        $staleHistoryArguments.PublicHistoryBytes = $staleHistory
        Assert-Throws -Action { Assert-CustodyPublicPredecessor @staleHistoryArguments } -Pattern "exact 1.1.1 ledger"

        $rewrittenHistory = [Text.UTF8Encoding]::new($false, $true).GetString($arguments.CandidateHistoryBytes) | ConvertFrom-Json
        $rewrittenHistory.releases[1].summary = "rewritten predecessor"
        $rewrittenBytes = [Text.UTF8Encoding]::new($false).GetBytes(($rewrittenHistory | ConvertTo-Json -Depth 32))
        $rewrittenArguments = @{} + $arguments
        $rewrittenArguments.CandidateHistoryBytes = $rewrittenBytes
        Assert-Throws -Action { Assert-CustodyPublicPredecessor @rewrittenArguments } -Pattern "exact public predecessor prefix"
    }

    "remote-overlay" {
        $expected = @("index.html", "docs/catalog.md", ".minimodel-publication-root.v1.json")
        $valid = @{
            "" = @(".minimodel-publication-root.v1.json", "docs", "index.html")
            "docs" = @("catalog.md")
        }
        Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $expected -ObservedTree $valid

        $overlay = @{
            "" = @(".minimodel-publication-root.v1.json", "docs", "index.html", "stale.js")
            "docs" = @("catalog.md")
        }
        Assert-Throws -Action { Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $expected -ObservedTree $overlay } -Pattern "overlay|closure"

        $wrongDirectory = @{
            "" = @(".minimodel-publication-root.v1.json", "docs", "index.html")
            "other" = @("catalog.md")
        }
        Assert-Throws -Action { Assert-CustodyRemoteDirectoryTree -ExpectedFilePaths $expected -ObservedTree $wrongDirectory } -Pattern "unknown or missing directory|closure"
    }

    "remote-observation" {
        $absent = Resolve-CustodyRemoteFileObservation -Listed $false -SizeStatus "not-probed"
        if ($absent.State -cne "absent" -or $null -ne $absent.Size) { throw "Exact LIST absence was not preserved." }
        $present = Resolve-CustodyRemoteFileObservation -Listed $true -SizeStatus "success" -Size 42
        if ($present.State -cne "present" -or [int64]$present.Size -ne 42) { throw "Exact SIZE/LIST presence was not preserved." }

        Assert-Throws -Action {
            Resolve-CustodyRemoteFileObservation -Listed $true -SizeStatus "ftp-550"
        } -Pattern "ambiguous"
        Assert-Throws -Action {
            Resolve-CustodyRemoteFileObservation -Listed $false -SizeStatus "ftp-550"
        } -Pattern "ambiguous"
        Assert-Throws -Action {
            Resolve-CustodyRemoteFileObservation -Listed $false -SizeStatus "success" -Size 42
        } -Pattern "contradict"
        Assert-Throws -Action {
            Resolve-CustodyRemoteFileObservation -Listed $true -SizeStatus "not-probed"
        } -Pattern "lacks one exact SIZE"
    }

    "rollback-order" {
        $journal = @(
            [pscustomobject]@{ Path = "first.txt" },
            [pscustomobject]@{ Path = "docs/second.txt" },
            [pscustomobject]@{ Path = ".well-known/site-release.json" }
        )
        $ordered = @(Get-CustodyBoundedRollbackOrder -Journal $journal)
        if ($ordered.Count -ne 3 -or
            $ordered[0].Path -cne ".well-known/site-release.json" -or
            $ordered[1].Path -cne "docs/second.txt" -or
            $ordered[2].Path -cne "first.txt") {
            throw "Rollback journal was not reversed exactly."
        }
        Assert-Throws -Action {
            Get-CustodyBoundedRollbackOrder -Journal @(
                [pscustomobject]@{ Path = "same.txt" },
                [pscustomobject]@{ Path = "same.txt" }
            )
        } -Pattern "duplicate"
        $oversized = for ($index = 0; $index -lt 501; $index++) {
            [pscustomobject]@{ Path = "bounded/$index.txt" }
        }
        Assert-Throws -Action {
            Get-CustodyBoundedRollbackOrder -Journal @($oversized)
        } -Pattern "exceeds its bound"
    }

    "lease-ownership" {
        $operationId = "0123456789abcdef0123456789abcdef"
        $ownerBytes = New-CustodyLeaseOwnerBytes `
            -Authority $authority `
            -OperationId $operationId `
            -PublisherCommit ("a" * 40) `
            -ManifestSha256 ("b" * 64) `
            -AuthoritySha256 ("c" * 64)
        Assert-CustodyLeaseOwnership `
            -ExpectedOwnerBytes $ownerBytes `
            -ObservedOwnerBytes $ownerBytes `
            -OperationId $operationId

        $substituted = [byte[]]$ownerBytes.Clone()
        $substituted[$substituted.Length - 2] = $substituted[$substituted.Length - 2] -bxor 1
        Assert-Throws -Action {
            Assert-CustodyLeaseOwnership `
                -ExpectedOwnerBytes $ownerBytes `
                -ObservedOwnerBytes $substituted `
                -OperationId $operationId
        } -Pattern "not owned"
        Assert-Throws -Action {
            Assert-CustodyLeaseOwnership `
                -ExpectedOwnerBytes $ownerBytes `
                -ObservedOwnerBytes $ownerBytes `
                -OperationId "fedcba9876543210fedcba9876543210"
        } -Pattern "not owned"
    }

    "published-materialization" {
        $manifest = Get-Content -LiteralPath (Join-Path $repositoryRoot "deploy\site-deployment-manifest.v1.json") -Raw | ConvertFrom-Json
        $releaseMaterialization = @($manifest.materializations | Where-Object { $_.path -ceq $authority.predecessor.release_path })
        $historyMaterialization = @($manifest.materializations | Where-Object { $_.path -ceq $authority.predecessor.history_path })
        $releaseFile = @($manifest.files | Where-Object { $_.path -ceq $authority.predecessor.release_path })
        $historyFile = @($manifest.files | Where-Object { $_.path -ceq $authority.predecessor.history_path })
        if ($releaseMaterialization.Count -ne 1 -or $historyMaterialization.Count -ne 1 -or
            $releaseFile.Count -ne 1 -or $historyFile.Count -ne 1) {
            throw "Governed publication materializations are not singular."
        }

        $preparedRelease = Get-GitBytes -Commit ([string]$authority.candidate.source_commit) -Path ([string]$authority.predecessor.release_path)
        $preparedHistory = Get-GitBytes -Commit ([string]$authority.candidate.source_commit) -Path ([string]$authority.predecessor.history_path)
        $preparedReleaseValue = ConvertFrom-Json ([Text.UTF8Encoding]::new($false, $true).GetString($preparedRelease))
        $preparedHistoryValue = ConvertFrom-Json ([Text.UTF8Encoding]::new($false, $true).GetString($preparedHistory))
        if ($preparedReleaseValue.status -cne "prepared" -or $preparedReleaseValue.last_publish_date -cne "2026-08-29" -or
            $preparedHistoryValue.releases[0].status -cne "prepared" -or $null -ne $preparedHistoryValue.releases[0].last_publish_date) {
            throw "Source publication facts were rewritten before deployment."
        }

        $publishedRelease = ConvertTo-CustodyPublishedArtifactBytes `
            -Kind ([string]$releaseMaterialization[0].kind) `
            -SourceBytes $preparedRelease `
            -Authority $authority
        $publishedHistory = ConvertTo-CustodyPublishedArtifactBytes `
            -Kind ([string]$historyMaterialization[0].kind) `
            -SourceBytes $preparedHistory `
            -Authority $authority
        if ($publishedRelease.Length -ne [int64]$releaseFile[0].bytes -or
            (Get-CustodyByteSha256 -Bytes $publishedRelease) -cne [string]$releaseFile[0].sha256 -or
            $publishedHistory.Length -ne [int64]$historyFile[0].bytes -or
            (Get-CustodyByteSha256 -Bytes $publishedHistory) -cne [string]$historyFile[0].sha256) {
            throw "Runtime materialization does not match the manifest."
        }
        Assert-CustodyPublishedReleasePair `
            -Authority $authority `
            -PreparedReleaseBytes $preparedRelease `
            -PreparedHistoryBytes $preparedHistory `
            -PublishedReleaseBytes $publishedRelease `
            -PublishedHistoryBytes $publishedHistory
    }

    "sealed-snapshot" {
        $testRoot = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ("minimodel-custody-test-{0}" -f [Guid]::NewGuid().ToString("N"))))
        if (Test-Path -LiteralPath $testRoot) { throw "Test root must begin absent." }
        [IO.Directory]::CreateDirectory($testRoot) | Out-Null
        $marker = Join-Path $testRoot ".owned-test-root"
        [IO.File]::WriteAllText($marker, "owned`n", [Text.UTF8Encoding]::new($false))
        $sourcePath = Join-Path $testRoot "mutable-source.bin"
        $original = [Text.UTF8Encoding]::new($false).GetBytes("sealed-original`n")
        [IO.File]::WriteAllBytes($sourcePath, $original)
        $manifest = [pscustomobject]@{
            source_commit = "c" * 40
            source_tree = "d" * 40
            files = @(
                [pscustomobject]@{
                    path = "docs/test.txt"
                    bytes = $original.Length
                    sha256 = Get-CustodyByteSha256 -Bytes $original
                },
                [pscustomobject]@{
                    path = "index.html"
                    bytes = $original.Length
                    sha256 = Get-CustodyByteSha256 -Bytes $original
                }
            )
        }
        $snapshotParent = Join-Path $testRoot "snapshots"
        $blobReader = { param($commit, $path) [IO.File]::ReadAllBytes($sourcePath) }.GetNewClosure()
        $snapshot = $null
        try {
            $snapshot = New-CustodySealedSnapshot `
                -Phase "public-admitted" `
                -RepositoryRoot $repositoryRoot `
                -Manifest $manifest `
                -ManifestSha256 ("e" * 64) `
                -SnapshotParent $snapshotParent `
                -OperationId "0123456789abcdef0123456789abcdef" `
                -BlobReader $blobReader
            [IO.File]::WriteAllText($sourcePath, "mutated-after-seal`n", [Text.UTF8Encoding]::new($false))
            Assert-CustodySealedSnapshot `
                -SnapshotRoot $snapshot.Root `
                -SnapshotParent $snapshotParent `
                -Manifest $manifest `
                -SealSha256 $snapshot.SealSha256 `
                -OperationId $snapshot.OperationId `
                -RootIdentity $snapshot.RootIdentity
            $snapshotBytes = [IO.File]::ReadAllBytes((Join-Path $snapshot.Root "index.html"))
            if ((Get-CustodyByteSha256 -Bytes $snapshotBytes) -cne (Get-CustodyByteSha256 -Bytes $original)) {
                throw "Snapshot followed mutable source after sealing."
            }

            [IO.File]::WriteAllText((Join-Path $snapshot.Root "overlay.txt"), "overlay", [Text.UTF8Encoding]::new($false))
            Assert-Throws -Action {
                Assert-CustodySealedSnapshot `
                    -SnapshotRoot $snapshot.Root `
                    -SnapshotParent $snapshotParent `
                    -Manifest $manifest `
                    -SealSha256 $snapshot.SealSha256 `
                    -OperationId $snapshot.OperationId `
                    -RootIdentity $snapshot.RootIdentity
            } -Pattern "overlay"
            Remove-Item -LiteralPath (Join-Path $snapshot.Root "overlay.txt") -Force
            Remove-CustodySealedSnapshot `
                -Phase "snapshot-sealed" `
                -SnapshotRoot $snapshot.Root `
                -SnapshotParent $snapshotParent `
                -RepositoryRoot $repositoryRoot `
                -Manifest $manifest `
                -SealSha256 $snapshot.SealSha256 `
                -OperationId $snapshot.OperationId `
                -RootIdentity $snapshot.RootIdentity
            $snapshot = $null

            [IO.File]::WriteAllBytes($sourcePath, $original)
            $reparseSnapshot = New-CustodySealedSnapshot `
                -Phase "public-admitted" `
                -RepositoryRoot $repositoryRoot `
                -Manifest $manifest `
                -ManifestSha256 ("e" * 64) `
                -SnapshotParent $snapshotParent `
                -OperationId "11111111111111111111111111111111" `
                -BlobReader $blobReader
            $junctionTarget = Join-Path $testRoot "junction-target"
            [IO.Directory]::CreateDirectory($junctionTarget) | Out-Null
            $junctionPath = Join-Path $reparseSnapshot.Root "substituted-link"
            New-Item -ItemType Junction -Path $junctionPath -Target $junctionTarget | Out-Null
            Assert-Throws -Action {
                Remove-CustodySealedSnapshot `
                    -Phase "snapshot-sealed" `
                    -SnapshotRoot $reparseSnapshot.Root `
                    -SnapshotParent $snapshotParent `
                    -RepositoryRoot $repositoryRoot `
                    -Manifest $manifest `
                    -SealSha256 $reparseSnapshot.SealSha256 `
                    -OperationId $reparseSnapshot.OperationId `
                    -RootIdentity $reparseSnapshot.RootIdentity
            } -Pattern "reparse|overlay"
            Remove-Item -LiteralPath $junctionPath -Force
            Remove-CustodySealedSnapshot `
                -Phase "snapshot-sealed" `
                -SnapshotRoot $reparseSnapshot.Root `
                -SnapshotParent $snapshotParent `
                -RepositoryRoot $repositoryRoot `
                -Manifest $manifest `
                -SealSha256 $reparseSnapshot.SealSha256 `
                -OperationId $reparseSnapshot.OperationId `
                -RootIdentity $reparseSnapshot.RootIdentity

            [IO.File]::WriteAllBytes($sourcePath, $original)
            $swapSnapshot = New-CustodySealedSnapshot `
                -Phase "public-admitted" `
                -RepositoryRoot $repositoryRoot `
                -Manifest $manifest `
                -ManifestSha256 ("e" * 64) `
                -SnapshotParent $snapshotParent `
                -OperationId "22222222222222222222222222222222" `
                -BlobReader $blobReader
            $originalRoot = "$($swapSnapshot.Root)-original"
            Move-Item -LiteralPath $swapSnapshot.Root -Destination $originalRoot
            [IO.Directory]::CreateDirectory($swapSnapshot.Root) | Out-Null
            Get-ChildItem -LiteralPath $originalRoot -Force | ForEach-Object {
                Copy-Item -LiteralPath $_.FullName -Destination $swapSnapshot.Root -Recurse -Force
            }
            Assert-Throws -Action {
                Remove-CustodySealedSnapshot `
                    -Phase "snapshot-sealed" `
                    -SnapshotRoot $swapSnapshot.Root `
                    -SnapshotParent $snapshotParent `
                    -RepositoryRoot $repositoryRoot `
                    -Manifest $manifest `
                    -SealSha256 $swapSnapshot.SealSha256 `
                    -OperationId $swapSnapshot.OperationId `
                    -RootIdentity $swapSnapshot.RootIdentity
            } -Pattern "replaced|operation-owned"
        }
        finally {
            if ($snapshot -and (Test-Path -LiteralPath $snapshot.Root)) {
                Remove-CustodySealedSnapshot `
                    -Phase "snapshot-sealed" `
                    -SnapshotRoot $snapshot.Root `
                    -SnapshotParent $snapshotParent `
                    -RepositoryRoot $repositoryRoot `
                    -Manifest $manifest `
                    -SealSha256 $snapshot.SealSha256 `
                    -OperationId $snapshot.OperationId `
                    -RootIdentity $snapshot.RootIdentity
            }
            $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
            if (-not $testRoot.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
                -not (Test-Path -LiteralPath $marker -PathType Leaf)) {
                throw "Refusing unsafe test cleanup."
            }
            Remove-Item -LiteralPath $testRoot -Recurse -Force
            if (Test-Path -LiteralPath $testRoot) { throw "Test cleanup failed." }
        }
    }

    default {
        throw "Unknown deployment custody test case."
    }
}

Write-Output "PASS $Case"
