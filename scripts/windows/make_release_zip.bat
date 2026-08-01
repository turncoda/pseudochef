set "ROOT=%~dp0\..\.."
echo "Root directory set to: %ROOT%"

cargo build --verbose --release

for /f "tokens=2 delims=#" %%v in ('cargo pkgid') do set "PKG_VER=%%v"
echo "Version is: %PKG_VER%"

7z a "%ROOT%\pseudochef-win64-%PKG_VER%.zip" "%ROOT%\static\*" "%ROOT%\target\release\pseudochef.exe"
