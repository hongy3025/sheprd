@echo off
setlocal
set "PATH=D:\AI\herdr\dist\windows;%PATH%"
set "SESSION=%~1"
if "%SESSION%"=="" set "SESSION=dev-test"
start "" "D:\AI\sheprd\target\debug\sheprd.exe" --session "%SESSION%"
endlocal
