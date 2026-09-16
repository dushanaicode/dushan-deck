param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)][string]$StateRoot
)

$ErrorActionPreference = 'Stop'
$currentSession = (Get-Process -Id $PID).SessionId
$running = @(Get-CimInstance Win32_Process -Filter "Name = 'dushan-deck.exe'" | Where-Object {
    $_.ExecutablePath -eq $Executable -and $_.SessionId -eq $currentSession
})
if ($running.Count -eq 0) { exit 10 }

# The existing Tauri single-instance handler shows the main window before opening storage.
& $Executable --state-root $StateRoot
exit $LASTEXITCODE
