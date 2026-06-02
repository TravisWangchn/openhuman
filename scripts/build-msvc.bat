@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64

rem Use Windows-native cmake (not MinGW's)
set PATH=C:\Program Files\CMake\bin;%PATH%

rem Override ALL cc crate env vars to force MSVC cl.exe over clang-cl
set MSVC_CL=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\cl.exe
set CC=%MSVC_CL%
set CXX=%MSVC_CL%
set CC_x86_64_pc_windows_msvc=%MSVC_CL%
set CXX_x86_64_pc_windows_msvc=%MSVC_CL%

set CEF_PATH=%USERPROFILE%\Library\Caches\tauri-cef
rem Add CEF DLLs to PATH so Windows loader can find libcef.dll at runtime
set PATH=%CEF_PATH%\146.0.9\cef_windows_x86_64;%CEF_PATH%\146.0.9\cef_windows_x86_64\locales;%PATH%
set OPENHUMAN_CORE_PORT=7788
set OPENHUMAN_DEV_JWT_TOKEN=dev-bypass-local-zn
set RUST_LOG=info
set RUST_BACKTRACE=1
set CMAKE_GENERATOR=Ninja
set OPENHUMAN_DEV_PORT=1421
cd /d C:\Users\bigdata\openhuman\app
cargo tauri dev
