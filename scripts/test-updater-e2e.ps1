$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-Checked {
  param(
    [Parameter(Mandatory)] [string] $Command,
    [Parameter(Mandatory)] [string[]] $Arguments,
    [Parameter(Mandatory)] [string] $Failure
  )
  & $Command @Arguments
  if ($LASTEXITCODE -ne 0) { throw "$Failure (exit code $LASTEXITCODE)" }
}

function Get-SingleItem {
  param(
    [Parameter(Mandatory)] [string] $Path,
    [Parameter(Mandatory)] [string] $Filter,
    [switch] $Directory
  )
  $arguments = @{ LiteralPath = $Path; Filter = $Filter }
  if ($Directory) { $arguments.Directory = $true } else { $arguments.File = $true }
  $items = @(Get-ChildItem @arguments)
  if ($items.Count -ne 1) {
    throw "Expected exactly one $Filter in $Path; found $($items.Count)"
  }
  $items[0]
}

$isWindowsRunner = $env:RUNNER_OS -eq 'Windows'
$isMacRunner = $env:RUNNER_OS -eq 'macOS'
if (-not $isWindowsRunner -and -not $isMacRunner) {
  throw "Updater E2E only supports Windows and macOS; found $($env:RUNNER_OS)"
}

$runnerTemp = if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
  [IO.Path]::GetTempPath()
} else {
  $env:RUNNER_TEMP
}
$baseVersion = (& node -p "JSON.parse(require('fs').readFileSync('package.json','utf8')).version").Trim()
if ($LASTEXITCODE -ne 0 -or $baseVersion -notmatch '^(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)$') {
  throw "Updater E2E requires a stable x.y.z package version; found $baseVersion"
}
$updateVersion = "$($Matches.major).$($Matches.minor).$([int]$Matches.patch + 1)"
$platform = if ($isWindowsRunner) { 'windows-x86_64' } else { 'darwin-aarch64' }
$bundleDirectory = if ($isWindowsRunner) {
  'src-tauri/target/release/bundle/nsis'
} else {
  'src-tauri/target/release/bundle/macos'
}

$key = Join-Path $runnerTemp 'updater-e2e.key'
$password = [Guid]::NewGuid().ToString('N')
Invoke-Checked 'node' @(
  'node_modules/@tauri-apps/cli/tauri.js', 'signer', 'generate', '--ci',
  '--password', $password, '--write-keys', $key
) 'Could not generate temporary updater key'
if (-not (Test-Path -LiteralPath "$key.pub")) { throw 'Temporary updater public key was not created' }
$env:TAURI_SIGNING_PRIVATE_KEY = [IO.File]::ReadAllText($key).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $password
$publicKey = [IO.File]::ReadAllText("$key.pub").Trim()

$bundleTarget = if ($isWindowsRunner) { 'nsis' } else { 'app' }
$bundle = [ordered]@{
  targets = @($bundleTarget)
  createUpdaterArtifacts = $true
  externalBin = @('binaries/job-assistant-sidecar')
}
if ($isWindowsRunner) {
  $bundle.windows = @{
    webviewInstallMode = @{ type = 'skip' }
  }
} else {
  $bundle.macOS = @{ hardenedRuntime = $false }
}
$testConfig = Join-Path $runnerTemp 'tauri.updater-e2e.json'
$config = [ordered]@{
  bundle = $bundle
  plugins = @{
    updater = @{
      endpoints = @('https://127.0.0.1:18443/latest.json')
      pubkey = $publicKey
      dangerousAcceptInvalidCerts = $true
      windows = @{ installMode = 'passive' }
    }
  }
}
[IO.File]::WriteAllText(
  $testConfig,
  ($config | ConvertTo-Json -Depth 10),
  [Text.UTF8Encoding]::new($false)
)

function Build-TestVersion([string] $Version) {
  Invoke-Checked 'node' @('scripts/set-version.mjs', $Version) "Could not set test version $Version"
  Invoke-Checked 'python' @('scripts/build_sidecar.py') "Could not build sidecar $Version"
  Invoke-Checked 'node' @(
    'node_modules/@tauri-apps/cli/tauri.js', 'build', '--features', 'updater-e2e',
    '--config', $testConfig
  ) "Could not build desktop test version $Version"
}

$serverRoot = Join-Path $runnerTemp 'updater-server'
New-Item -ItemType Directory -Force -Path $serverRoot | Out-Null
Build-TestVersion $updateVersion
if ($isWindowsRunner) {
  $update = Get-SingleItem $bundleDirectory "*$updateVersion*x64-setup.exe"
  $updateAssetName = 'update.exe'
} else {
  $update = Get-SingleItem $bundleDirectory '*.app.tar.gz'
  $updateAssetName = 'update.app.tar.gz'
}
if (-not (Test-Path -LiteralPath "$($update.FullName).sig")) {
  throw "Missing signed updater test artifact for $platform"
}
Copy-Item -LiteralPath $update.FullName -Destination (Join-Path $serverRoot $updateAssetName)
$signature = [IO.File]::ReadAllText("$($update.FullName).sig").Trim()

Build-TestVersion $baseVersion
if ($isWindowsRunner) {
  $base = Get-SingleItem $bundleDirectory "*$baseVersion*x64-setup.exe"
} else {
  $base = Get-SingleItem $bundleDirectory '*.app' -Directory
}

$platforms = @{}
$platforms[$platform] = @{
  signature = $signature
  url = "https://127.0.0.1:18443/$updateAssetName"
}
$manifest = @{
  version = $updateVersion
  notes = "Temporary $platform updater end-to-end validation"
  pub_date = [DateTimeOffset]::UtcNow.ToString('o')
  platforms = $platforms
}
[IO.File]::WriteAllText(
  (Join-Path $serverRoot 'latest.json'),
  ($manifest | ConvertTo-Json -Depth 8),
  [Text.UTF8Encoding]::new($false)
)

$certificate = Join-Path $runnerTemp 'updater-e2e.crt'
$privateKey = Join-Path $runnerTemp 'updater-e2e-tls.key'
Invoke-Checked 'openssl' @(
  'req', '-x509', '-newkey', 'rsa:2048', '-sha256', '-days', '1', '-nodes',
  '-keyout', $privateKey, '-out', $certificate, '-subj', '/CN=localhost',
  '-addext', 'subjectAltName=IP:127.0.0.1,DNS:localhost'
) 'Could not create updater test TLS certificate'

$serverArguments = @(
  'scripts/updater_test_server.py', '--directory', $serverRoot,
  '--certificate', $certificate, '--private-key', $privateKey
)
$server = if ($isWindowsRunner) {
  Start-Process python -ArgumentList $serverArguments -PassThru -WindowStyle Hidden
} else {
  Start-Process python -ArgumentList $serverArguments -PassThru
}
$application = $null
try {
  $ready = $false
  for ($attempt = 0; $attempt -lt 40; $attempt++) {
    try {
      Invoke-WebRequest -Uri 'https://127.0.0.1:18443/latest.json' -SkipCertificateCheck -TimeoutSec 2 | Out-Null
      $ready = $true
      break
    } catch {
      Start-Sleep -Milliseconds 250
    }
  }
  if (-not $ready) { throw 'Updater test HTTPS server did not start' }

  if ($isWindowsRunner) {
    $install = Start-Process $base.FullName -ArgumentList '/S' -PassThru -Wait
    if ($install.ExitCode -ne 0) { throw "Base updater test installer exited with $($install.ExitCode)" }
    $uninstallRoot = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall'
    $entry = Get-ChildItem $uninstallRoot | ForEach-Object { Get-ItemProperty $_.PSPath } |
      Where-Object { $_.DisplayIcon -match 'ai-job-app\.exe' } |
      Sort-Object DisplayVersion -Descending | Select-Object -First 1
    if (-not $entry) { throw 'Updater test uninstall registry entry was not found' }
    $applicationPath = [Environment]::ExpandEnvironmentVariables([string]$entry.DisplayIcon).Trim()
    if ($applicationPath -match '^"([^"]+)"') {
      $applicationPath = $Matches[1]
    } else {
      $applicationPath = $applicationPath -replace ',\d+$', ''
    }
    $applicationItem = Get-Item -LiteralPath $applicationPath -ErrorAction Stop
    $applicationPath = $applicationItem.FullName
    $sidecarPath = Join-Path $applicationItem.DirectoryName 'job-assistant-sidecar.exe'
  } else {
    $applicationPath = Join-Path $base.FullName 'Contents/MacOS/ai-job-app'
    $sidecarPath = Join-Path $base.FullName 'Contents/MacOS/job-assistant-sidecar'
    if (-not (Test-Path -LiteralPath $applicationPath)) {
      throw 'macOS updater test application executable was not found'
    }
  }
  if (-not (Test-Path -LiteralPath $sidecarPath)) {
    throw 'Installed updater test sidecar was not found'
  }

  $marker = Join-Path $runnerTemp "updater-e2e-result-$platform.json"
  Get-Process -Name 'ai-job-app' -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $marker -Force -ErrorAction SilentlyContinue
  $application = Start-Process $applicationPath -ArgumentList @(
    '--updater-e2e-result', $marker,
    '--updater-e2e-expected', $updateVersion
  ) -PassThru

  $deadline = (Get-Date).AddMinutes(8)
  $result = $null
  while ((Get-Date) -lt $deadline) {
    if (Test-Path -LiteralPath $marker) {
      try { $result = Get-Content -LiteralPath $marker -Raw -Encoding UTF8 | ConvertFrom-Json } catch { $result = $null }
      if ($result -and $result.stage -in @('restarted', 'failed')) { break }
    }
    if ($application.HasExited -and -not (Test-Path -LiteralPath $marker)) {
      throw "Updater test application exited with $($application.ExitCode) before writing a result"
    }
    Start-Sleep -Seconds 2
  }
  if (-not $result) { throw 'Updater test did not produce a result' }
  if ($result.stage -eq 'failed') { throw "Updater test failed: $($result.error)" }
  if (
    $result.stage -ne 'restarted' -or
    -not $result.ok -or
    $result.version -ne $updateVersion -or
    $result.progressEvents -lt 1 -or
    $result.downloadedBytes -lt 1
  ) {
    throw "Updater end-to-end assertions failed: $($result | ConvertTo-Json -Compress)"
  }
  Write-Host "Updater E2E passed for $platform ($baseVersion -> $updateVersion)"
} finally {
  Get-Process -Name 'ai-job-app' -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
  if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force }
}
