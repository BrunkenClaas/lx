# LX Coreutils shell integration for PowerShell
# Add to $PROFILE:
#   . /path/to/shell-integration/lx.ps1

# lx tools emit UTF-8 (bullets, accented chars). Ensure the console decodes it.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

# Ctrl+K: send current buffer to lxsh
Set-PSReadLineKeyHandler -Chord Ctrl+k -ScriptBlock {
    $line = $null; $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    if ($line) {
        $result = $line | lxsh 2>$null
        if ($result) {
            [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
            [Microsoft.PowerShell.PSConsoleReadLine]::Insert($result)
        }
    }
}

# Ctrl+E: explain current buffer before running.
# Echoes the original command, clears the buffer and accepts the (now empty)
# line for a clean prompt cycle, then prints the explanation below it.
Set-PSReadLineKeyHandler -Chord Ctrl+e -ScriptBlock {
    $line = $null; $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    if ($line) {
        $explanation = ($line | lxexplain 2>$null) -join "`n"
        if ($explanation) { 
            [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
            Write-Host $line
            [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
            Write-Host $explanation
        }
    }
}

