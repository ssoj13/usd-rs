@echo off
setlocal

set "UCRT_ROOT=C:\Temp\ucrt\ucrt64"
set "PATH=%UCRT_ROOT%\bin;%PATH%"

"%UCRT_ROOT%\bin\oslc.exe" %*

endlocal
