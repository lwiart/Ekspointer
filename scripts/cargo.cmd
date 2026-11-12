@echo off
rem Lance cargo dans l'environnement MSVC (editeur de liens et SDK Windows).
setlocal
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
for /f "usebackq delims=" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSI=%%i"
if not defined VSI (
  echo Build Tools Visual Studio introuvables. 1>&2
  exit /b 1
)
call "%VSI%\VC\Auxiliary\Build\vcvars64.bat" >nul
"%USERPROFILE%\.cargo\bin\cargo.exe" %*
exit /b %ERRORLEVEL%
