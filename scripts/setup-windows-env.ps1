$ErrorActionPreference = "Stop"

Set-Location (Join-Path $PSScriptRoot "..")

function Assert-Command([string]$Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Missing required command: $Name"
  }
}

Assert-Command "node"
Assert-Command "npm.cmd"
Assert-Command "rustc"
Assert-Command "cargo"
Assert-Command "winapp"

Write-Host "Node: $(node -v)"
Write-Host "npm: $(npm.cmd -v)"
Write-Host "rustc: $(rustc --version)"
Write-Host "cargo: $(cargo --version)"
Write-Host "winapp CLI detected"

Write-Host "Installing workspace dependencies..."
npm.cmd install

Write-Host "Checking local Tauri CLI..."
npm.cmd --workspace @imgstar/desktop exec tauri -- --version

Write-Host "Running full frontend quality gate..."
npm.cmd run check

Write-Host "Environment bootstrap completed."
