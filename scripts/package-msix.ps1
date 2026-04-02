$ErrorActionPreference = "Stop"

Set-Location (Join-Path $PSScriptRoot "..")

function Assert-Command([string]$Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Missing required command: $Name"
  }
}

function Convert-ToMsixVersion([string]$Version) {
  $baseVersion = $Version.Split("-")[0]
  $segments = @($baseVersion.Split("."))

  if ($segments.Count -gt 4) {
    $segments = $segments[0..3]
  }

  while ($segments.Count -lt 4) {
    $segments += "0"
  }

  return ($segments -join ".")
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
Assert-Command "winapp"

$repoRoot = (Get-Location).Path
$rootPackage = Get-Content (Join-Path $repoRoot "package.json") -Raw | ConvertFrom-Json
$tauriConfig = Get-Content (Join-Path $repoRoot "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json

$productName = $tauriConfig.productName
$packageName = $tauriConfig.identifier
$msixVersion = Convert-ToMsixVersion $rootPackage.version
$publisherName = "CN=$productName"
$logoPath = Join-Path $repoRoot "src-tauri\icons\icon.png"
$releaseDir = Join-Path $repoRoot "src-tauri\target\release"
$stagingDir = Join-Path $repoRoot "dist\msix"
$outputMsix = Join-Path $stagingDir "$productName.msix"
$certPath = Join-Path $repoRoot "dist\devcert.pfx"

if (Test-Path $stagingDir) {
  Remove-Item $stagingDir -Recurse -Force
}

New-Item -ItemType Directory -Path $stagingDir | Out-Null

Write-Host "Building Tauri release..."
& (Join-Path $repoRoot "scripts\build-tauri.ps1")

$exePath = Get-FirstExistingPath @(
  (Join-Path $releaseDir "$productName.exe"),
  (Join-Path $releaseDir "app.exe")
)

if (-not $exePath) {
  $fallbackExe = Get-ChildItem -Path $releaseDir -Filter *.exe -File -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($fallbackExe) {
    $exePath = $fallbackExe.FullName
  }
}

if (-not $exePath) {
  throw "Could not find a release executable in $releaseDir after tauri build."
}

$exeName = Split-Path $exePath -Leaf
Copy-Item $exePath (Join-Path $stagingDir $exeName) -Force
$stagedExePath = Join-Path $stagingDir $exeName

Write-Host "Generating Appx manifest..."
winapp manifest generate $stagingDir `
  --package-name $packageName `
  --publisher-name $publisherName `
  --version $msixVersion `
  --description $productName `
  --entrypoint $stagedExePath `
  --template packaged `
  --logo-path $logoPath `
  --if-exists overwrite

if ($LASTEXITCODE -ne 0) {
  throw "Appx manifest generation failed with exit code $LASTEXITCODE."
}

Write-Host "Generating signing certificate..."
winapp cert generate `
  --publisher $publisherName `
  --output $certPath `
  --if-exists skip

if ($LASTEXITCODE -ne 0) {
  throw "Certificate generation failed with exit code $LASTEXITCODE."
}

Write-Host "Packaging MSIX..."
winapp pack $stagingDir `
  --manifest (Join-Path $stagingDir "AppxManifest.xml") `
  --cert $certPath `
  --output $outputMsix `
  --executable $exeName

if ($LASTEXITCODE -ne 0) {
  throw "MSIX packaging failed with exit code $LASTEXITCODE."
}

Write-Host "MSIX package created at $outputMsix"