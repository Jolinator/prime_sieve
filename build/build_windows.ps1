$ErrorActionPreference = 'Stop'

if ($env:OS -ne 'Windows_NT') {
    throw 'This helper must be run on Windows.'
}

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDirectory = Split-Path -Parent $scriptDirectory
$outputDirectory = Join-Path $scriptDirectory 'windows'
$targetDirectory = Join-Path $projectDirectory 'target'

Push-Location $projectDirectory
try {
    $rustInfo = & rustc -vV
    if ($LASTEXITCODE -ne 0) {
        throw "rustc failed with exit code $LASTEXITCODE."
    }
    $hostLine = @($rustInfo | Where-Object { $_ -match '^host: ' })
    if ($hostLine.Count -ne 1) {
        throw 'Could not determine the Rust host target.'
    }
    $hostTarget = $hostLine[0].Substring(6).Trim()
    if (-not $hostTarget) {
        throw 'Could not determine the Rust host target.'
    }

    & cargo build --release --target $hostTarget --target-dir $targetDirectory
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE."
    }

    New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
    $executable = Join-Path $targetDirectory "$hostTarget\release\prime_sieve.exe"
    Copy-Item -LiteralPath $executable -Destination (Join-Path $outputDirectory 'prime_sieve.exe') -Force
    Copy-Item -LiteralPath (Join-Path $projectDirectory 'collect_windows.ps1') -Destination (Join-Path $outputDirectory 'collect_windows.ps1') -Force
    Write-Output "Windows package created in $outputDirectory"
}
finally {
    Pop-Location
}
