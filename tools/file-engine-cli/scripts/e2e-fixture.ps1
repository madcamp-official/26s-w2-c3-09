$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$cliDir = Resolve-Path (Join-Path $scriptDir "..")
$repoRoot = Resolve-Path (Join-Path $cliDir "..\..")
$fixtureRoot = Join-Path $repoRoot "test-fixtures\file-trees\basic"
$runRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("file-engine-e2e-" + [System.Guid]::NewGuid().ToString("N"))
$managedRoot = Join-Path $runRoot "basic"
$proposalPath = Join-Path $runRoot "proposal.json"
$decisionPath = Join-Path $runRoot "decision.jsonl"

New-Item -ItemType Directory -Path $runRoot | Out-Null
Copy-Item -Recurse -Path $fixtureRoot -Destination $managedRoot
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue (Join-Path $managedRoot ".mousekeeper")

Push-Location $cliDir
try {
    cargo run --quiet -- propose $managedRoot | Set-Content -Encoding utf8 $proposalPath
    $proposal = Get-Content -Raw $proposalPath | ConvertFrom-Json
    $firstProposal = @($proposal.proposals)[0]

    if ($null -eq $firstProposal) {
        throw "fixture did not produce any proposals"
    }

    @{
        proposal_id = $firstProposal.proposal_id
        decision = "approved"
    } | ConvertTo-Json -Compress | Set-Content -Encoding utf8 $decisionPath

    cargo run --quiet -- precheck $managedRoot --proposal $proposalPath --decision $decisionPath | Out-Null
    cargo run --quiet -- execute $managedRoot --proposal $proposalPath --decision $decisionPath | Out-Null

    if (-not (Test-Path (Join-Path $managedRoot $firstProposal.to))) {
        throw "execute did not create expected target: $($firstProposal.to)"
    }

    cargo run --quiet -- undo $managedRoot | Out-Null

    if (-not (Test-Path (Join-Path $managedRoot $firstProposal.from))) {
        throw "undo did not restore expected source: $($firstProposal.from)"
    }

    Write-Output "e2e fixture flow passed: $managedRoot"
}
finally {
    Pop-Location
}
