$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$LocalBinary = Join-Path $Root "funo.exe"
$BuiltBinary = Join-Path $Root "src-tauri\target\release\funo.exe"

if (Test-Path $LocalBinary) {
    $Cli = $LocalBinary
} elseif (Test-Path $BuiltBinary) {
    $Cli = $BuiltBinary
} else {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "Установите Rust или положите готовый funo.exe рядом с проектом."
    }
    Write-Host "Собираю Funo CLI..." -ForegroundColor Cyan
    cargo build --release --locked --manifest-path (Join-Path $Root "src-tauri\Cargo.toml") --bin funo
    if ($LASTEXITCODE -ne 0) { throw "Сборка Funo CLI завершилась с ошибкой." }
    $Cli = $BuiltBinary
}

& $Cli setup
if ($LASTEXITCODE -ne 0) { throw "Не удалось добавить Funo в PATH." }
Write-Host "`nГотово. Откройте новый терминал и проверьте: funo --version" -ForegroundColor Green
