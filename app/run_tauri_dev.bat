@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
rem Remove MinGW from PATH (interferes with CMake)
set PATH=%PATH:C:\Users\bigdata\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;=%
rem Override auto-detected clang-cl with MSVC cl.exe for CMake builds
set CMAKE_C_COMPILER=cl.exe
set CMAKE_CXX_COMPILER=cl.exe
set OPENHUMAN_APP_ENV=staging
cargo tauri dev
