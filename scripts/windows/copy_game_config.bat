@echo off
if /i "%1 %2"=="to repo" (
    set "direction=in"
    goto valid
)
if /i "%1 %2"=="from repo" (
    set "direction=out"
    goto valid
)
if /i "%1 %2"=="to tb" (
    set "direction=out"
    goto valid
)
if /i "%1 %2"=="from tb" (
    set "direction=in"
    goto valid
)
echo Please specify one of the following: to repo, from repo, to tb, from tb
exit /b 1

:valid

set "src=%appdata%\TrenchBroom\games\Pseudoregalia"
set "dst=%~dp0\..\..\static\Pseudoregalia"

REM Normalize paths.
for %%p in ("%src%") do set "src=%%~fp"
for %%p in ("%dst%") do set "dst=%%~fp"

REM Maybe swap paths.
REM Note: this might look strange, but it works as intended.
REM We don't need a temp variable because batch scripts perform all
REM substitutions in a block upfront.
if "%direction%"=="out" (
    set src=%dst%
    set dst=%src%
)

echo.
echo Copying from: %src%
echo Copying   to: %dst%
echo.
echo WARNING: Everything in the following folder will be overwritten!
echo.
echo     %dst%
echo.
set /p "answer=Are you sure? (y/n) "
if /i "%answer%" NEQ "y" (
    exit /b 1
)

robocopy "%src%" "%dst%" /mir
