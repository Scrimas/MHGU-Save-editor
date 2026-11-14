# Windows' touch keyboard opens when a touch tap focuses a field that UI Automation reports
# as editable text. Starts the editor on the Items page (a search field) and checks that the
# window's UI Automation tree has an Edit element with ValuePattern or TextPattern that is
# not read-only and takes the keyboard (Slint's accessibility feature, AccessKit).
# Windows PowerShell 5.1 (its .NET Framework has the UI Automation client).
#   powershell -File uia-check.ps1 <editor.exe> <save file>
param([Parameter(Mandatory)] [string]$Exe, [Parameter(Mandatory)] [string]$Save)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]

$p = Start-Process -FilePath $Exe -ArgumentList @("`"$Save`"", 'items') -PassThru
try {
    $deadline = (Get-Date).AddSeconds(120)
    $win = $null
    while (-not $win) {
        if ((Get-Date) -gt $deadline) { throw 'no window after 120 s' }
        Start-Sleep -Milliseconds 500
        $p.Refresh()
        if ($p.HasExited) { throw "the editor quit (exit code $($p.ExitCode))" }
        if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $win = $A::FromHandle($p.MainWindowHandle) }
    }
    $isEdit = New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Edit)
    $edit = $null
    while (-not $edit) {
        if ((Get-Date) -gt $deadline) { throw 'no Edit element in the UI Automation tree' }
        $edit = $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $isEdit)
        if (-not $edit) { Start-Sleep -Milliseconds 500 }
    }
    $patterns = @($edit.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    "Edit '$($edit.Current.Name)', focusable: $($edit.Current.IsKeyboardFocusable), patterns: $($patterns -join ', ')"
    if (-not $edit.Current.IsKeyboardFocusable) { throw 'the Edit element does not take the keyboard' }
    $value = $null
    if ($edit.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$value)) {
        if ($value.Current.IsReadOnly) { throw 'the Edit element is read-only' }
    } elseif (-not ($patterns -match 'TextPattern')) {
        throw 'the Edit element has neither ValuePattern nor TextPattern'
    }
    'OK: a touch tap on this field opens the touch keyboard'
} finally {
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
}
