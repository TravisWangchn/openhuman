@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
set PATH=%PATH:C:\Users\bigdata\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;=%
for /d %%i in ("%CD%\src-tauri\target\debug\build\whisper-rs-sys-*") do rmdir /s /q "%%i" 2>nul
set CMAKE_GENERATOR=Ninja
set CMAKE_GENERATOR_PLATFORM=
set GGML_NATIVE=OFF
set OPENHUMAN_APP_ENV=staging
cargo tauri dev
