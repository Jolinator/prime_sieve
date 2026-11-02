param(
    [string]$OutputPath
)

# Collect the Windows host details this program uses for tuning (cores, cache sizes, memory).
# Usage: .\collect_windows.ps1 [output.env]
$ErrorActionPreference = 'Stop'

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $scriptDirectory '.env'
}
$outputDirectory = Split-Path -Parent $OutputPath
if ([string]::IsNullOrWhiteSpace($outputDirectory)) {
    $outputDirectory = (Get-Location).Path
}
if (-not (Test-Path -LiteralPath $outputDirectory)) {
    New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
}

function Get-CimInstances {
    param(
        [Parameter(Mandatory = $true)][string]$ClassName,
        [string]$Namespace = 'root/cimv2'
    )
    try {
        return @(Get-CimInstance -Namespace $Namespace -ClassName $ClassName -ErrorAction Stop)
    }
    catch {
        return @()
    }
}

# Returns "level|type|sizeBytes|lineSize" for every cache the OS reports.
function Get-WindowsCacheRecords {
    try {
        if (-not ('CacheTopologySetsNative' -as [type])) {
            Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class CacheTopologySetsNative {
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetLogicalProcessorInformationEx(
        int relationshipType,
        IntPtr buffer,
        ref uint returnedLength);

    public static string[] GetCacheRecords() {
        uint length = 0;
        GetLogicalProcessorInformationEx(2, IntPtr.Zero, ref length);
        if (length == 0) return new string[0];

        IntPtr buffer = Marshal.AllocHGlobal((int)length);
        try {
            if (!GetLogicalProcessorInformationEx(2, buffer, ref length))
                return new string[0];

            int totalLength = (int)length;
            int offset = 0;
            List<string> records = new List<string>();
            while (offset <= totalLength - 8) {
                int relationship = Marshal.ReadInt32(buffer, offset);
                int structureSize = Marshal.ReadInt32(buffer, offset + 4);
                if (structureSize < 8 || structureSize > totalLength - offset)
                    break;

                if (relationship == 2 && structureSize >= 20) {
                    int level = Marshal.ReadByte(buffer, offset + 8);
                    int lineSize = (ushort)Marshal.ReadInt16(buffer, offset + 10);
                    uint cacheSize = unchecked((uint)Marshal.ReadInt32(buffer, offset + 12));
                    int type = Marshal.ReadInt32(buffer, offset + 16);
                    records.Add(level + "|" + type + "|" + cacheSize + "|" + lineSize);
                }
                offset += structureSize;
            }
            return records.ToArray();
        }
        finally {
            Marshal.FreeHGlobal(buffer);
        }
    }
}
'@ -ErrorAction Stop | Out-Null
        }
        return @([CacheTopologySetsNative]::GetCacheRecords())
    }
    catch {
        return @()
    }
}

function ConvertTo-EnvString {
    param([AllowNull()][object]$Value)
    if ($null -eq $Value) {
        return ''
    }
    if ($Value -is [System.Array]) {
        $Value = ($Value | ForEach-Object { [string]$_ }) -join ','
    }
    $text = [string]$Value
    $text = $text.Replace('\', '\\').Replace('"', '\"')
    $text = $text.Replace("`r", '\r').Replace("`n", '\n')
    return '"' + $text + '"'
}

$os = Get-CimInstances 'Win32_OperatingSystem' | Select-Object -First 1
$processors = @(Get-CimInstances 'Win32_Processor')

$physicalCores = 0
$logicalCores = 0
$processorNames = @()
foreach ($processor in $processors) {
    $physicalCores += [int]$processor.NumberOfCores
    $logicalCores += [int]$processor.NumberOfLogicalProcessors
    if ($processor.Name) { $processorNames += [string]$processor.Name.Trim() }
}

# Type codes from GetLogicalProcessorInformationEx: 0 unified, 1 instruction, 2 data.
$l1dSizes = @()
$l2Sizes = @()
$l3Sizes = @()
$lineSizes = @()
foreach ($record in @(Get-WindowsCacheRecords)) {
    $parts = $record -split '\|'
    if ($parts.Count -ne 4) { continue }
    $level = [int]$parts[0]
    $type = [int]$parts[1]
    $sizeKb = [int][Math]::Floor([long]$parts[2] / 1KB)
    $lineSize = [int]$parts[3]
    if ($lineSize -gt 0) { $lineSizes += $lineSize }
    if ($level -eq 1 -and $type -eq 2) { $l1dSizes += $sizeKb }
    elseif ($level -eq 2 -and $type -ne 1) { $l2Sizes += $sizeKb }
    elseif ($level -eq 3 -and $type -ne 1) { $l3Sizes += $sizeKb }
}

function Join-Unique {
    param([object[]]$Items)
    return (@($Items | Select-Object -Unique | Sort-Object) -join ',')
}

# Cache sizes are comma separated when a hybrid CPU has different core types.
$values = [ordered]@{
    CPU_MODEL = (@($processorNames | Select-Object -Unique) -join '; ')
    CPU_PHYSICAL_CORES = [string]$physicalCores
    CPU_LOGICAL_CORES = [string]$logicalCores
    CPU_CACHE_LINE_SIZE_BYTES = (Join-Unique $lineSizes)
    CPU_L1D_CACHE_SIZE_PER_INSTANCE_KB = (Join-Unique $l1dSizes)
    CPU_L2_CACHE_SIZE_PER_INSTANCE_KB = (Join-Unique $l2Sizes)
    CPU_L3_CACHE_SIZE_PER_INSTANCE_KB = (Join-Unique $l3Sizes)
    PHYSICAL_MEMORY_TOTAL_KB = if ($os.TotalVisibleMemorySize) { [string]$os.TotalVisibleMemorySize } else { '' }
}

$lines = foreach ($entry in $values.GetEnumerator()) {
    if ([string]::IsNullOrWhiteSpace([string]$entry.Value)) { continue }
    '{0}={1}' -f $entry.Key, (ConvertTo-EnvString $entry.Value)
}
[System.IO.File]::WriteAllLines([System.IO.Path]::GetFullPath($OutputPath), [string[]]$lines, (New-Object -TypeName System.Text.UTF8Encoding -ArgumentList $false))
Write-Output "Wrote system details to $([System.IO.Path]::GetFullPath($OutputPath))"
