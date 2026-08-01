@echo off

REM First argument is required.

if "%1"=="patch" goto valid
if "%1"=="minor" goto valid
if "%1"=="major" goto valid

echo Please specify one of the following: patch, minor, major
exit /b 1

:valid

REM Git tree must be clean.

for /f %%i in ('git status --porcelain') do (
    echo ERROR: Git working tree is not clean. Commit or stash changes first.
    exit /b 1
)
echo Git tree is clean.

REM Update Cargo.toml and Cargo.lock.

cargo install cargo-edit
cargo set-version --bump %1

REM Get new version.

for /f "tokens=2 delims=#" %%v in ('cargo pkgid') do set "PKG_VER=%%v"
echo Version is: %PKG_VER%

REM Commit and tag.

git commit -am "v%PKG_VER%"
git tag "v%PKG_VER%"

echo Done!
