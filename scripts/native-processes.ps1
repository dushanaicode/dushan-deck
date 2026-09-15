# Only report processes belonging to the explicitly named synthetic test state.
$ErrorActionPreference = 'Stop'
if (-not $env:DECK_VERIFY_STATE_ROOT) { throw 'Missing DECK_VERIFY_STATE_ROOT' }
$owned = @(Get-CimInstance Win32_Process -Filter "Name = 'dushan-deck.exe' OR Name = 'msedgewebview2.exe'" | Where-Object {
    $_.CommandLine -and $_.CommandLine.Contains($env:DECK_VERIFY_STATE_ROOT)
} | ForEach-Object {
    [PSCustomObject]@{
        pid = $_.ProcessId
        parent = $_.ParentProcessId
        name = $_.Name
        workingSet = [long]$_.WorkingSetSize
        cpu100ns = [long]$_.KernelModeTime + [long]$_.UserModeTime
    }
})
ConvertTo-Json -InputObject $owned -Compress
