param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+$')]
    [string]$Version
)

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
    function Clear-PackageTestFiles {
        foreach ($name in @('.env', 'results')) {
            $path = Join-Path $outputDirectory $name
            if (Test-Path -LiteralPath $path) {
                Remove-Item -LiteralPath $path -Recurse -Force
            }
        }
    }
    try {
        Clear-PackageTestFiles
        Push-Location $outputDirectory
        try {
            & (Join-Path $outputDirectory 'collect_windows.ps1')
            & (Join-Path $outputDirectory 'prime_sieve.exe') count 1000000
            if ($LASTEXITCODE -ne 0) {
                throw "Count smoke test failed with exit code $LASTEXITCODE."
            }
            $reports = @(Get-ChildItem -Path (Join-Path $outputDirectory 'results\result_test_count_1000000_*.txt') -File)
            if ($reports.Count -ne 1 -or -not (Select-String -LiteralPath $reports[0].FullName -Pattern '^Limit: 1000000, Prime count: 78498,' -Quiet)) {
                throw 'Count smoke test failed: expected 78498 primes up to 1000000.'
            }
        }
        finally {
            Pop-Location
        }
    }
    finally {
        Clear-PackageTestFiles
    }
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = Join-Path $scriptDirectory "V.$Version-windows.zip"
    $temporaryArchive = Join-Path $scriptDirectory ([System.IO.Path]::GetRandomFileName())
    try {
        $zip = [System.IO.Compression.ZipFile]::Open($temporaryArchive, [System.IO.Compression.ZipArchiveMode]::Create)
        try {
            foreach ($item in Get-ChildItem -LiteralPath $outputDirectory -Recurse -Force) {
                $relativePath = $item.FullName.Substring($outputDirectory.Length + 1).Replace('\', '/')
                $entryName = "windows/$relativePath"
                if ($item.PSIsContainer) {
                    $zip.CreateEntry("$entryName/") | Out-Null
                }
                else {
                    [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                        $zip, $item.FullName, $entryName,
                        [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
                }
            }
        }
        finally {
            $zip.Dispose()
        }
        Move-Item -LiteralPath $temporaryArchive -Destination $archive -Force
    }
    finally {
        if (Test-Path -LiteralPath $temporaryArchive) {
            Remove-Item -LiteralPath $temporaryArchive -Force
        }
    }
    Write-Output "Windows package created in $outputDirectory"
    Write-Output "Archive created in $archive"
}
finally {
    Pop-Location
}
