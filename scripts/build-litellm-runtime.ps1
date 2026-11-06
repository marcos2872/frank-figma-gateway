param(
  [string]$Version = "1.108.1",
  [string]$Python = "python",
  [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$out = Join-Path $repo $OutDir
$work = Join-Path $env:RUNNER_TEMP "ffg-litellm-$Version"
$zipName = "litellm-runtime-$Version-win-x64.zip"
$zipPath = Join-Path $out $zipName

New-Item -ItemType Directory -Force -Path $out | Out-Null
if (Test-Path $work) { Remove-Item -Recurse -Force $work }
New-Item -ItemType Directory -Force -Path $work | Out-Null

$venv = Join-Path $work "venv"
& $Python -m venv $venv
$py = Join-Path $venv "Scripts\python.exe"
if (-not (Test-Path $py)) { throw "python da venv não encontrado" }

& $py -m pip install --upgrade pip
& $py -m pip install "litellm[proxy]==$Version" pyinstaller
if ($LASTEXITCODE -ne 0) { throw "pip install do LiteLLM falhou" }

# O entrypoint vive num .py versionado: embutir Python num here-string do
# PowerShell fazia a sintaxe do shell comer crases e `$` do código.
$template = Join-Path $PSScriptRoot "runtime\litellm-entrypoint.py"
if (-not (Test-Path $template)) { throw "template do entrypoint não encontrado: $template" }

$entrySource = Get-Content -Raw -Encoding utf8 $template
if (-not $entrySource.Contains("__LITELLM_VERSION__")) {
  throw "template sem o marcador __LITELLM_VERSION__: $template"
}

$entry = Join-Path $work "entrypoint.py"
Set-Content -Path $entry -Value $entrySource.Replace("__LITELLM_VERSION__", $Version) -Encoding utf8 -NoNewline

Push-Location $work
try {
  & $py -m PyInstaller `
    --noconfirm `
    --clean `
    --onedir `
    --name litellm `
    --collect-all litellm `
    --collect-all tiktoken `
    --collect-all tiktoken_ext `
    --copy-metadata litellm `
    $entry
  if ($LASTEXITCODE -ne 0) { throw "PyInstaller falhou" }
} finally {
  Pop-Location
}

$dist = Join-Path $work "dist\litellm"
$exe = Join-Path $dist "litellm.exe"
if (-not (Test-Path $exe)) { throw "litellm.exe não foi gerado" }
& $exe --version
if ($LASTEXITCODE -ne 0) { throw "litellm --version falhou" }

if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
Compress-Archive -Path (Join-Path $dist "*") -DestinationPath $zipPath -CompressionLevel Optimal
Write-Host "artefato gerado: $zipPath"

# TODO(F2 Windows validation): confirmar hidden imports do LiteLLM e ajustar
# o entrypoint caso o módulo do proxy mude na versão fixada.
