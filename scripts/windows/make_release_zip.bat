@echo off

cargo build --release
echo.

set "ROOT=%~dp0\..\.."
for %%p in ("%ROOT%") do set "ROOT=%%~fp"
echo ### Root directory set to: %ROOT%


for /f "tokens=2 delims=#" %%v in ('cargo pkgid') do set "PKG_VER=%%v"
echo ### Version is: %PKG_VER%
echo.

7z a "%ROOT%\pseudochef-win64-%PKG_VER%.zip" "%ROOT%\static\*" "%ROOT%\target\release\pseudochef.exe"
