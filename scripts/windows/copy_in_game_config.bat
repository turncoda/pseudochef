set "SRC=%appdata%\TrenchBroom\games\Pseudoregalia"
set "DST=%~dp0\..\..\static\Pseudoregalia"
rmdir /S /Q "%DST%"
robocopy "%SRC%" "%DST%" *.* /S
