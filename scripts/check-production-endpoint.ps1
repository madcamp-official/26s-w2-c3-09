param(
  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$BaseUrl
)

$ErrorActionPreference = 'Stop'
$uri = [Uri]$BaseUrl
if ($uri.Scheme -ne 'https') {
  throw 'Production endpoint must use https.'
}

function Assert-HousemouseEndpoint {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [Parameter(Mandatory = $true)]
    [string]$ExpectedStatus
  )

  $target = [Uri]::new($uri, $Path)
  $body = Invoke-RestMethod -Uri $target -TimeoutSec 15
  if ($body.status -ne $ExpectedStatus) {
    throw "$Path returned an unexpected status payload."
  }
  "CHECK Url=$($target.AbsoluteUri) HttpStatus=200 Status=$($body.status)"
}

Resolve-DnsName $uri.Host -Type A -ErrorAction Stop |
  Where-Object Type -eq 'A' |
  ForEach-Object { "DNS Name=$($_.Name) IPAddress=$($_.IPAddress)" }
Assert-HousemouseEndpoint -Path '/health' -ExpectedStatus 'ok'
Assert-HousemouseEndpoint -Path '/ready' -ExpectedStatus 'ready'
