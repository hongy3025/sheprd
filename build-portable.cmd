@echo off
setlocal EnableExtensions

set "ROOT=%~dp0"
if "%ROOT:~-1%"=="\" set "ROOT=%ROOT:~0,-1%"

set "DO_BUILD=0"
set "OUT_DIR="

:parse_args
if "%~1"=="" goto args_done
if /I "%~1"=="--build" (
    set "DO_BUILD=1"
    shift
    goto parse_args
)
if "%OUT_DIR%"=="" (
    set "OUT_DIR=%~1"
    shift
    goto parse_args
)
echo Unexpected argument: %~1
exit /b 1

:args_done
if "%OUT_DIR%"=="" set "OUT_DIR=%ROOT%\dist\sheprd-portable"

set "REL_DIR=%ROOT%\target\release"
set "HERDR_DIST_DIR=D:\AI\herdr\dist"
set "HERDR_ZIP=%HERDR_DIST_DIR%\herdr-dist.zip"
set "HERDR_WIN_DIR=%HERDR_DIST_DIR%\windows"
set "INSTALLED_CORE=%LOCALAPPDATA%\Programs\Herdr UI\herdr-core.exe"

set "BINS=sheprd.exe sheprd-launcher.exe sheprd-acp.exe sheprd-automate.exe sheprd-nearby.exe sheprd-remote-control.exe"

set "NEED_BUILD=%DO_BUILD%"
if "%NEED_BUILD%"=="0" (
    for %%B in (%BINS%) do (
        if not exist "%REL_DIR%\%%B" set "NEED_BUILD=1"
    )
)

if "%NEED_BUILD%"=="1" (
    echo [1/5] Building release binaries ^(sheprd-gpui, sheprd-launcher, sheprd-acp, sheprd-automate, sheprd-nearby, sheprd-remote-control^)...
    cargo build --locked --release --manifest-path "%ROOT%\Cargo.toml" -p sheprd-gpui -p sheprd-launcher -p sheprd-acp -p sheprd-automate -p sheprd-nearby -p sheprd-remote-control
    if errorlevel 1 exit /b 1
) else (
    echo [1/5] Using existing release binaries in %REL_DIR%
)

for %%B in (%BINS%) do (
    if not exist "%REL_DIR%\%%B" (
        echo Error: missing %REL_DIR%\%%B
        exit /b 1
    )
)
if not exist "%HERDR_ZIP%" (
    echo Error: missing %HERDR_ZIP%
    exit /b 1
)

echo [2/5] Preparing portable directory: %OUT_DIR%
if exist "%OUT_DIR%" rmdir /s /q "%OUT_DIR%" >nul 2>&1
if not exist "%OUT_DIR%" (
    mkdir "%OUT_DIR%"
    if errorlevel 1 exit /b 1
)

echo [3/5] Copying sheprd binaries and compatibility aliases...
for %%B in (%BINS%) do (
    copy /y "%REL_DIR%\%%B" "%OUT_DIR%\%%B" >nul
    if errorlevel 1 exit /b 1
)
copy /y "%REL_DIR%\sheprd.exe" "%OUT_DIR%\herdr-gpui.exe" >nul
if errorlevel 1 exit /b 1
copy /y "%REL_DIR%\sheprd-launcher.exe" "%OUT_DIR%\herdr-gpui-launcher.exe" >nul
if errorlevel 1 exit /b 1
copy /y "%REL_DIR%\sheprd-acp.exe" "%OUT_DIR%\herdr-acp.exe" >nul
if errorlevel 1 exit /b 1
copy /y "%REL_DIR%\sheprd-automate.exe" "%OUT_DIR%\herdr-automate.exe" >nul
if errorlevel 1 exit /b 1
copy /y "%REL_DIR%\sheprd-nearby.exe" "%OUT_DIR%\herdr-nearby.exe" >nul
if errorlevel 1 exit /b 1
copy /y "%REL_DIR%\sheprd-remote-control.exe" "%OUT_DIR%\herdr-remote-control.exe" >nul
if errorlevel 1 exit /b 1

echo [4/5] Copying herdr-core.exe and herdr-dist.zip...
if exist "%REL_DIR%\herdr-core.exe" (
    copy /y "%REL_DIR%\herdr-core.exe" "%OUT_DIR%\herdr-core.exe" >nul
    if errorlevel 1 exit /b 1
) else if exist "%INSTALLED_CORE%" (
    copy /y "%INSTALLED_CORE%" "%OUT_DIR%\herdr-core.exe" >nul
    if errorlevel 1 exit /b 1
) else (
    where python >nul 2>&1
    if not errorlevel 1 (
        python "%ROOT%\scripts\release\fetch-herdr-core.py" windows-x86_64 "%OUT_DIR%"
        if errorlevel 1 echo Warning: failed to fetch herdr-core.exe; skipping.
    ) else (
        echo Warning: herdr-core.exe not found locally and python unavailable; skipping.
    )
)
copy /y "%HERDR_ZIP%" "%OUT_DIR%\herdr-dist.zip" >nul
if errorlevel 1 exit /b 1

echo [5/5] Extracting custom herdr.exe ^(and conpty runtime^) into %OUT_DIR%...
set "STAGE_DIR=%OUT_DIR%\_stage"
mkdir "%STAGE_DIR%" >nul 2>&1
tar -xf "%HERDR_ZIP%" -C "%STAGE_DIR%" herdr-windows-x86_64.zip >nul 2>&1
if exist "%STAGE_DIR%\herdr-windows-x86_64.zip" (
    tar -xf "%STAGE_DIR%\herdr-windows-x86_64.zip" -C "%OUT_DIR%" >nul 2>&1
) else if exist "%HERDR_WIN_DIR%\herdr.exe" (
    xcopy "%HERDR_WIN_DIR%\*" "%OUT_DIR%\" /e /i /y >nul
)
if exist "%STAGE_DIR%" rmdir /s /q "%STAGE_DIR%"

if not exist "%OUT_DIR%\herdr.exe" (
    echo Error: herdr.exe missing in %OUT_DIR%
    exit /b 1
)

echo.
echo Portable bundle created at: %OUT_DIR%
dir /b "%OUT_DIR%"
endlocal
