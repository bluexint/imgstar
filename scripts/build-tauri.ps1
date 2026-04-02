$ErrorActionPreference = "Stop"

Set-Location (Join-Path $PSScriptRoot "..")

function Assert-Command([string]$Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Missing required command: $Name"
  }
}

function Get-FirstExistingPath([string[]]$Candidates) {
  foreach ($candidate in $Candidates) {
    if (Test-Path $candidate) {
      return (Resolve-Path $candidate).Path
    }
  }

  return $null
}

Assert-Command "npm.cmd"

$repoRoot = (Get-Location).Path
$tauriCli = Get-FirstExistingPath @(
  (Join-Path $repoRoot "node_modules\.bin\tauri.cmd"),
  (Join-Path $repoRoot "apps\desktop\node_modules\.bin\tauri.cmd")
)

if (-not $tauriCli) {
  throw "Missing local Tauri CLI at $tauriCli. Run npm.cmd install first."
}

& $tauriCli build --no-bundle

if ($LASTEXITCODE -ne 0) {
  throw "Tauri build failed with exit code $LASTEXITCODE."
}