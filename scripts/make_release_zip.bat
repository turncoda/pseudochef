@echo off
cargo build --release

for /f "tokens=2 delims=#" %%v in ('cargo pkgid') do set "PKG_VER=%%v"
echo Version is: %PKG_VER%

"C:\Program Files\7-Zip\7z.exe" a "%~dp0\..\pseudochef-win64-%PKG_VER%.zip" "%~dp0\..\static\Pseudoregalia" "%~dp0\..\static\INSTRUCTIONS.txt" "%~dp0\..\target\release\pseudochef.exe"
