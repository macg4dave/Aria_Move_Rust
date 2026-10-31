$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Set-Location $repo
$version = (Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$output = Join-Path $repo 'target/release-check'
$stage = Join-Path $output "aria_move-$version-windows-x86_64"
New-Item -ItemType Directory -Force (Join-Path $stage 'bin') | Out-Null
function Invoke-Cargo {
    & cargo @args
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed: $args" }
}
Start-Transcript -Path (Join-Path $output 'windows-check.log') -Force
try {
    Invoke-Cargo @('fmt', '--all', '--', '--check')
    Invoke-Cargo @('clippy', '--locked', '--workspace', '--all-targets', '--all-features', '--', '-D', 'warnings')
    Invoke-Cargo test --locked --workspace --all-targets --all-features
    Invoke-Cargo test --locked --workspace --doc --all-features
    Invoke-Cargo build --locked --release --target x86_64-pc-windows-msvc
    $executable = Join-Path $output "aria_move-$version-windows-x86_64.exe"
    Copy-Item target/x86_64-pc-windows-msvc/release/aria_move.exe $executable -Force
    & $executable --version
    if ($LASTEXITCODE -ne 0) { throw 'Release executable failed to start' }
    Copy-Item $executable (Join-Path $stage 'bin/aria_move.exe') -Force
    Copy-Item README.md,LICENSE $stage -Force
    Copy-Item docs $stage -Recurse -Force
    $archive = "$stage.zip"
    Compress-Archive -Path "$stage/*" -DestinationPath $archive -Force
    foreach ($artifact in @($executable, $archive)) {
        $digest = (Get-FileHash $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
        [IO.File]::WriteAllText("$artifact.sha256", "$digest  $(Split-Path $artifact -Leaf)`n")
    }
    Write-Host "Verified Windows release artifacts: $output"
} finally {
    Stop-Transcript
}
