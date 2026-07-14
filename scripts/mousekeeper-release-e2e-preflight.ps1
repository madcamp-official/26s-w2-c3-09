#Requires -Version 5.1

[CmdletBinding()]
param(
  [string]$BaseUrl = $env:MOUSEKEEPER_API_URL,
  [switch]$SkipAndroid,
  [switch]$SkipDesktop,
  [switch]$AllowDirty,
  [switch]$RunLocalChecks
)

$ErrorActionPreference = 'Stop'

if (-not $BaseUrl -and $env:MOUSEKEEPER_SERVER_BASE_URL) {
  $BaseUrl = $env:MOUSEKEEPER_SERVER_BASE_URL
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$checks = New-Object System.Collections.Generic.List[object]

function Add-Check {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][ValidateSet('PASS', 'FAIL', 'UNCONFIGURED', 'SKIP')][string]$Status,
    [string]$Detail = ''
  )
  $script:checks.Add([pscustomobject]@{
    Name = $Name
    Status = $Status
    Detail = $Detail
  }) | Out-Null
}

function Test-CommandAvailable {
  param([Parameter(Mandatory = $true)][string]$Name)
  $command = Get-Command $Name -ErrorAction SilentlyContinue
  if ($command) {
    Add-Check $Name 'PASS' $command.Source
    return $command.Source
  }
  Add-Check $Name 'UNCONFIGURED' "$Name is not available on PATH"
  return $null
}

function Test-RequiredPath {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$RelativePath
  )
  $path = Join-Path $repoRoot $RelativePath
  if (Test-Path -LiteralPath $path) {
    Add-Check $Name 'PASS' $RelativePath
    return
  }
  Add-Check $Name 'FAIL' "Missing repo path: $RelativePath"
}

function Invoke-JsonGet {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$ExpectedStatus
  )
  $uri = "$($BaseUrl.TrimEnd('/'))$Path"
  try {
    $response = Invoke-WebRequest -Uri $uri -Method Get -UseBasicParsing -TimeoutSec 10
    $json = $response.Content | ConvertFrom-Json
    if ($response.StatusCode -ne 200) {
      Add-Check "GET $Path" 'FAIL' "HTTP $($response.StatusCode)"
      return
    }
    if ($json.status -ne $ExpectedStatus) {
      Add-Check "GET $Path" 'FAIL' "Expected status '$ExpectedStatus', got '$($json.status)'"
      return
    }
    Add-Check "GET $Path" 'PASS' "$uri returned status '$ExpectedStatus'"
  } catch {
    Add-Check "GET $Path" 'FAIL' "$uri unreachable or invalid JSON: $($_.Exception.Message)"
  }
}

function Resolve-AdbPath {
  $adb = Get-Command adb -ErrorAction SilentlyContinue
  if ($adb) { return $adb.Source }
  if ($env:LOCALAPPDATA) {
    $candidate = Join-Path $env:LOCALAPPDATA 'Android\Sdk\platform-tools\adb.exe'
    if (Test-Path -LiteralPath $candidate) { return $candidate }
  }
  return $null
}

Push-Location $repoRoot
try {
  Test-RequiredPath 'mobile app path' 'apps/mobile'
  Test-RequiredPath 'server app path' 'apps/server'
  Test-RequiredPath 'desktop tauri path' 'apps/desktop/src-tauri'
  Test-RequiredPath 'contracts path' 'packages/contracts'

  Test-CommandAvailable 'git' | Out-Null
  Test-CommandAvailable 'pnpm' | Out-Null

  if (-not $AllowDirty) {
    $dirty = git status --porcelain
    if ($LASTEXITCODE -ne 0) {
      Add-Check 'git worktree' 'FAIL' 'git status failed'
    } elseif ($dirty) {
      Add-Check 'git worktree' 'FAIL' 'Working tree is dirty; rerun with -AllowDirty while developing'
    } else {
      Add-Check 'git worktree' 'PASS' 'clean'
    }
  } else {
    Add-Check 'git worktree' 'SKIP' '-AllowDirty set'
  }

  if (-not $BaseUrl) {
    Add-Check 'API base URL' 'UNCONFIGURED' 'Set MOUSEKEEPER_API_URL or MOUSEKEEPER_SERVER_BASE_URL'
  } elseif ($BaseUrl -notmatch '^https?://') {
    Add-Check 'API base URL' 'FAIL' "BaseUrl must start with http:// or https://"
  } else {
    Add-Check 'API base URL' 'PASS' $BaseUrl
    Invoke-JsonGet -Path '/health' -ExpectedStatus 'ok'
    Invoke-JsonGet -Path '/ready' -ExpectedStatus 'ready'
  }

  $gradleFiles = @(
    'apps/mobile/android/app/build.gradle',
    'apps/mobile/android/app/build.gradle.kts'
  )
  $packageMatched = $false
  foreach ($relative in $gradleFiles) {
    $path = Join-Path $repoRoot $relative
    if ((Test-Path -LiteralPath $path) -and (Select-String -LiteralPath $path -SimpleMatch 'com.mousekeeper.app' -Quiet)) {
      $packageMatched = $true
      break
    }
  }
  if ($packageMatched) {
    Add-Check 'Android package id' 'PASS' 'com.mousekeeper.app'
  } else {
    Add-Check 'Android package id' 'FAIL' 'com.mousekeeper.app not found in mobile Gradle config'
  }

  if ($SkipAndroid) {
    Add-Check 'Android device' 'SKIP' '-SkipAndroid set'
  } else {
    $flutterPath = Test-CommandAvailable 'flutter'
    $adbPath = Resolve-AdbPath
    if ($adbPath) {
      Add-Check 'adb' 'PASS' $adbPath
    } else {
      Add-Check 'adb' 'UNCONFIGURED' 'Install Android platform-tools or add adb to PATH'
    }
    if ($flutterPath) {
      try {
        $devicesJson = flutter devices --machine | ConvertFrom-Json
        $androidDevices = @($devicesJson | Where-Object { $_.targetPlatform -like 'android-*' -or $_.platformType -eq 'android' })
        if ($androidDevices.Count -gt 0) {
          Add-Check 'Android device' 'PASS' (($androidDevices | ForEach-Object { "$($_.name) [$($_.id)]" }) -join ', ')
        } else {
          Add-Check 'Android device' 'UNCONFIGURED' 'No connected Android device from flutter devices --machine'
        }
      } catch {
        Add-Check 'Android device' 'FAIL' "flutter devices failed: $($_.Exception.Message)"
      }
    }
  }

  if ($SkipDesktop) {
    Add-Check 'Desktop Rust toolchain' 'SKIP' '-SkipDesktop set'
  } else {
    Test-CommandAvailable 'cargo' | Out-Null
  }

  if ($RunLocalChecks) {
    try {
      pnpm check:contracts
      if ($LASTEXITCODE -eq 0) {
        Add-Check 'pnpm check:contracts' 'PASS' ''
      } else {
        Add-Check 'pnpm check:contracts' 'FAIL' "exit code $LASTEXITCODE"
      }
    } catch {
      Add-Check 'pnpm check:contracts' 'FAIL' $_.Exception.Message
    }

    $flutterLocal = Get-Command flutter -ErrorAction SilentlyContinue
    if (-not $flutterLocal) {
      Add-Check 'mobile smart-cache regression tests' 'UNCONFIGURED' 'flutter is not available on PATH'
    } else {
      Push-Location (Join-Path $repoRoot 'apps/mobile')
      try {
        flutter test test/smart_cache_policy_test.dart test/verified_download_test.dart test/display_cache_test.dart
        if ($LASTEXITCODE -eq 0) {
          Add-Check 'mobile smart-cache regression tests' 'PASS' ''
        } else {
          Add-Check 'mobile smart-cache regression tests' 'FAIL' "exit code $LASTEXITCODE"
        }
      } catch {
        Add-Check 'mobile smart-cache regression tests' 'FAIL' $_.Exception.Message
      } finally {
        Pop-Location
      }
    }

    if (-not $SkipDesktop) {
      try {
        cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --features tauri-commands scheduled_ticks_keep_heartbeats_frequent_and_split_rest_reconcile
        if ($LASTEXITCODE -eq 0) {
          Add-Check 'desktop heartbeat split test' 'PASS' ''
        } else {
          Add-Check 'desktop heartbeat split test' 'FAIL' "exit code $LASTEXITCODE"
        }
      } catch {
        Add-Check 'desktop heartbeat split test' 'FAIL' $_.Exception.Message
      }
    }
  } else {
    Add-Check 'local regression tests' 'SKIP' 'Use -RunLocalChecks to run contract and desktop cadence checks'
  }
} finally {
  Pop-Location
}

Write-Host ''
Write-Host 'MouseKeeper release E2E preflight'
Write-Host '================================='
$checks | Format-Table -AutoSize

$blocking = @($checks | Where-Object { $_.Status -in @('FAIL', 'UNCONFIGURED') })
if ($blocking.Count -gt 0) {
  Write-Host ''
  Write-Host 'Blocking checks:'
  foreach ($check in $blocking) {
    Write-Host "- $($check.Status): $($check.Name) $($check.Detail)"
  }
  Write-Host ''
  Write-Host 'No release E2E pass is claimed. Fix the checks above, then run the manual scenarios in docs/e2e-scenarios.md.'
  exit 1
}

Write-Host ''
Write-Host 'Preflight passed. Next manual release E2E steps:'
Write-Host '1. Pair Android and Desktop with the same Firebase user.'
Write-Host '2. Register a disposable managed root fixture, not a real user folder.'
Write-Host '3. Run command -> proposal -> approval -> execution -> undo.'
Write-Host '4. Run browse -> download -> SHA-256 verify -> ACK/object cleanup.'
Write-Host '5. Revoke device and remove room from both sides, then confirm replay convergence.'
Write-Host '6. Record evidence with the template in docs/e2e-scenarios.md.'
exit 0
