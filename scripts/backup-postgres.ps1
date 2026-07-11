param(
  [Parameter(Mandatory = $true)]
  [string]$OutputPath
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($env:DATABASE_URL)) {
  throw 'UNCONFIGURED: DATABASE_URL'
}

$pgDump = Get-Command pg_dump -ErrorAction SilentlyContinue
if (-not $pgDump) {
  throw 'UNCONFIGURED: pg_dump'
}

$resolvedParent = Split-Path -Parent ([System.IO.Path]::GetFullPath($OutputPath))
if (-not (Test-Path -LiteralPath $resolvedParent -PathType Container)) {
  throw "Backup output directory does not exist: $resolvedParent"
}

& $pgDump.Source `
  --format=custom `
  --no-owner `
  --no-privileges `
  --file $OutputPath `
  $env:DATABASE_URL

if ($LASTEXITCODE -ne 0) {
  throw "pg_dump failed with exit code $LASTEXITCODE"
}

$backup = Get-Item -LiteralPath $OutputPath
if ($backup.Length -le 0) {
  throw 'Backup validation failed: empty output'
}

$checksum = Get-FileHash -LiteralPath $backup.FullName -Algorithm SHA256
Write-Output "BACKUP_OK path=$($backup.FullName) bytes=$($backup.Length) sha256=$($checksum.Hash.ToLowerInvariant())"
