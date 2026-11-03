# Usage: .\scripts\bench-baseline.ps1 -Label <machine-label> [-Extra @("--repeats","7")]
param(
    [Parameter(Mandatory = $true)][string]$Label,
    [string[]]$Extra = @()
)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
if (git status --porcelain --untracked-files=no) {
    Write-Warning "working tree is dirty; results will be marked dirty"
}
$out = "benchmarks/baselines/$Label"
New-Item -ItemType Directory -Force -Path $out | Out-Null
cargo build --release -p vx-app
if ($LASTEXITCODE -ne 0) { throw "build failed" }
& .\target\release\vx.exe --probe | Tee-Object -FilePath "$out/adapter.txt"
$commit = git rev-parse --short=12 HEAD
foreach ($scenario in @("small", "default", "far")) {
    & .\target\release\vx.exe bench --scenario $scenario --out "$out/$scenario-$commit.json" @Extra
    if ($LASTEXITCODE -ne 0) { throw "bench $scenario failed" }
}
Write-Host "results in $out"
