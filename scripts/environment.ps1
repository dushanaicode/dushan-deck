$ErrorActionPreference = 'Stop'
$deckRoot = (Get-Location).Path
if (-not (Test-Path -LiteralPath (Join-Path $deckRoot 'scripts/environment.ps1'))) {
    throw 'Run from the Dushan Deck project root.'
}
$deckTemp = Join-Path $deckRoot 'Temp/tooling'
New-Item -ItemType Directory -Force -Path $deckTemp | Out-Null
$env:TEMP = $deckTemp
$env:TMP = $deckTemp
$env:TMPDIR = $deckTemp
$env:CARGO_HOME = Join-Path $deckTemp 'cargo-home'
$env:CARGO_TARGET_DIR = Join-Path $deckRoot 'Temp/build/rust'
$env:npm_config_cache = Join-Path $deckTemp 'npm-cache'
$env:npm_config_update_notifier = 'false'
$env:npm_config_audit = 'false'
$env:npm_config_fund = 'false'
$env:PLAYWRIGHT_BROWSERS_PATH = Join-Path $deckTemp 'browsers'
$env:RUSTUP_AUTO_INSTALL = '0'
$env:PYTHONDONTWRITEBYTECODE = '1'
$env:DECK_TEST_ROOT = Join-Path $deckRoot 'Temp/verification'
