@echo off
:: Builds a local release ZIP of the full LX Coreutils suite for Windows (x86_64-pc-windows-msvc).
:: Usage:
::   scripts\build-release-zip.bat [VERSION]
:: VERSION defaults to "dev".
::
:: Requires PowerShell 7+ (pwsh) or Windows PowerShell 5.1+.

setlocal

set VERSION=%~1
if "%VERSION%"=="" set VERSION=dev

where pwsh >nul 2>&1
if %ERRORLEVEL%==0 (
    set PS=pwsh
) else (
    where powershell >nul 2>&1
    if %ERRORLEVEL%==0 (
        set PS=powershell
    ) else (
        echo error: PowerShell not found. Install PowerShell 7+ from https://aka.ms/powershell
        exit /b 1
    )
)

set SCRIPT_DIR=%~dp0
%PS% -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%build-release-zip.ps1" -Version "%VERSION%"
exit /b %ERRORLEVEL%
