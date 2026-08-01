set "SRC=%~dp0\..\static\Pseudoregalia"
set "DST=%appdata%\TrenchBroom\games\Pseudoregalia"
rmdir /S /Q "%DST%"
robocopy "%SRC%" "%DST%" *.* /S
