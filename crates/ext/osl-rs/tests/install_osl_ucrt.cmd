@echo off
setlocal

set "ROOT=C:\Temp\ucrt"
set "MIRROR=https://mirror.msys2.org/mingw/ucrt64"

if not exist "%ROOT%" mkdir "%ROOT%"

set PKGS=^
mingw-w64-ucrt-x86_64-openshadinglanguage-1.14.8.0-3-any.pkg.tar.zst ^
mingw-w64-ucrt-x86_64-openimageio-3.1.9.0-5-any.pkg.tar.zst ^
mingw-w64-ucrt-x86_64-zstd-1.5.7-1-any.pkg.tar.zst ^
mingw-w64-ucrt-x86_64-zlib-1.3.1-1-any.pkg.tar.zst ^
mingw-w64-ucrt-x86_64-tbb-2022.3.0-1-any.pkg.tar.zst ^
mingw-w64-ucrt-x86_64-gcc-libs-15.2.0-11-any.pkg.tar.zst ^
mingw-w64-ucrt-x86_64-libwinpthread-13.0.0.r488.g3fedac280-2-any.pkg.tar.zst

for %%P in (%PKGS%) do (
  echo Downloading %%P
  powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-WebRequest -Uri '%MIRROR%/%%P' -OutFile '%ROOT%\%%P'"
  if errorlevel 1 goto :err
  echo Extracting %%P
  tar -xf "%ROOT%\%%P" -C "%ROOT%"
  if errorlevel 1 goto :err
)

>"%ROOT%\oslc.cmd" (
  echo @echo off
  echo setlocal
  echo set "UCRT_ROOT=%ROOT%\ucrt64"
  echo set "PATH=%%UCRT_ROOT%%\bin;%%PATH%%"
  echo "%%UCRT_ROOT%%\bin\oslc.exe" %%*
  echo endlocal
)

echo Done. Run: %ROOT%\oslc.cmd --help
exit /b 0

:err
echo Failed while processing packages.
exit /b 1
