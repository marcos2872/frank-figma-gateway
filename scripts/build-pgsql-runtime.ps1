param(
  [string]$Version = "16.9",
  [string]$Build = "1",
  [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$out = Join-Path $repo $OutDir
$work = Join-Path $env:RUNNER_TEMP "ffg-pgsql-$Version-$Build"
$zipName = "pgsql-$Version-win-x64.zip"
$zipPath = Join-Path $out $zipName
$url = "https://get.enterprisedb.com/postgresql/postgresql-$Version-$Build-windows-x64-binaries.zip"

New-Item -ItemType Directory -Force -Path $out | Out-Null
if (Test-Path $work) { Remove-Item -Recurse -Force $work }
New-Item -ItemType Directory -Force -Path $work | Out-Null

Write-Host "baixando Postgres $Version-$Build de $url"
Invoke-WebRequest -Uri $url -OutFile (Join-Path $work "pgsql.zip")
Expand-Archive -Path (Join-Path $work "pgsql.zip") -DestinationPath $work -Force

$runtime = Join-Path $work "pgsql"
if (-not (Test-Path (Join-Path $runtime "bin\postgres.exe"))) {
  throw "postgres.exe não encontrado no pacote baixado"
}

& (Join-Path $runtime "bin\postgres.exe") --version
if ($LASTEXITCODE -ne 0) {
  throw "postgres --version falhou"
}

if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
Compress-Archive -Path (Join-Path $runtime "*") -DestinationPath $zipPath -CompressionLevel Optimal
Write-Host "artefato gerado: $zipPath"
