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

REM Get old version.
for /f "tokens=2 delims=#" %%v in ('cargo pkgid') do set "OLD_VER=%%v"

REM Bump version, update Cargo.toml and Cargo.lock.
cargo install cargo-edit
cargo set-version --bump %1

REM Get new version.
for /f "tokens=2 delims=#" %%v in ('cargo pkgid') do set "NEW_VER=%%v"

echo Previous version was: %OLD_VER%
echo New version is: %NEW_VER%

REM Generate changelog using commit messages.
set "COMMIT_MSG_FILE=%TEMP%\msg.txt"
(
REM The ">" is escaped by the "^".
echo v%OLD_VER% -^> v%NEW_VER%
echo.
REM The percent sign needs to be escaped because this is a batch script; it's not for git.
git log --format="* %%s" --reverse "v%OLD_VER%.."
) > "%COMMIT_MSG_FILE%"

REM Commit and tag.
git add -u
git commit -e -F "%COMMIT_MSG_FILE%"

REM Reset on abort.
if %ERRORLEVEL% NEQ 0 (
    echo Aborting!
    git reset --hard HEAD
    exit /b 1
)

REM Commit was successful; proceed to tag.
git tag "v%NEW_VER%"
echo Created tag: v%NEW_VER%
