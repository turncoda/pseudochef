@echo off

set "ARCH=generic"
if /i "%PROCESSOR_ARCHITECTURE%"=="AMD64" (
    set "ARCH=x86_64"
)

cd %~dp0
cd ..
cd ..

for /f "tokens=2 delims=#" %%v in ('cargo pkgid') do set "PKG_VER=%%v"
set "ZIP_NAME=pseudochef-win64-%ARCH%-%PKG_VER%.zip"

cargo build --release
echo.
echo ### Version is: %PKG_VER%
echo.


copy "target\release\pseudochef.exe" "dist\"
copy "target\release\dump_assets.exe" "dist\"
cd "dist\"
7z u "..\%ZIP_NAME%" *
cd ..
7z l "%ZIP_NAME%"
