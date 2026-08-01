@echo off
for /f %%i in ('git status --porcelain') do (
    echo ERROR: Git working tree is not clean. Commit or stash changes first.
    exit /b 1
)
echo Git tree is clean.

cargo install cargo-edit
cargo set-version --bump patch
for /f "tokens=2 delims=#" %%v in ('cargo pkgid') do set "PKG_VER=%%v"
echo Version is: %PKG_VER%
git commit -am "v%PKG_VER%"
git tag "v%PKG_VER%"
echo Done!
