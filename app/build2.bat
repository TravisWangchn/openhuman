@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
rem Remove MinGW from PATH (interferes with CMake)
set PATH=%PATH:C:\Users\bigdata\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;=%
rem Clear CEF and whisper build caches
for /d %%i in ("%CD%\src-tauri\target\debug\build\cef-dll-sys-*") do rmdir /s /q "%%i" 2>nul
for /d %%i in ("%CD%\src-tauri\target\debug\build\whisper-rs-sys-*") do rmdir /s /q "%%i" 2>nul
rem CC/CXX override cmake crate's compiler detection (uses cc crate internally)
set CC=cl.exe
set CXX=cl.exe
set OPENHUMAN_APP_ENV=staging
cargo tauri dev
