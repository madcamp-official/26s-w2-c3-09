param(
  [Parameter(Mandatory = $true)]
  [string]$BackupPath,

  [switch]$Apply
)

$ErrorActionPreference = 'Stop'

if (-not $Apply) {
  throw 'Restore is destructive. Re-run with -Apply after verifying the target DATABASE_URL.'
}
if ([string]::IsNullOrWhiteSpace($env:DATABASE_URL)) {
  throw 'UNCONFIGURED: DATABASE_URL'
}
if (-not (Test-Path -LiteralPath $BackupPath -PathType Leaf)) {
  throw "Backup file does not exist: $BackupPath"
}

$pgRestore = Get-Command pg_restore -ErrorAction SilentlyContinue
if (-not $pgRestore) {
  throw 'UNCONFIGURED: pg_restore'
}

& $pgRestore.Source `
  --exit-on-error `
  --no-owner `
  --no-privileges `
  --clean `
  --if-exists `
  --dbname $env:DATABASE_URL `
  $BackupPath

if ($LASTEXITCODE -ne 0) {
  throw "pg_restore failed with exit code $LASTEXITCODE"
}

Write-Output 'RESTORE_OK'
