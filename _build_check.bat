@echo off
REM Initialize MSVC build environment for OpenHuman cargo check
REM Strategy: avoid clang-cl entirely and use MSVC's cl.exe
call "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat"
if %ERRORLEVEL% neq 0 (
    echo vcvars64.bat failed with exit code %ERRORLEVEL%
    exit /b %ERRORLEVEL%
)
cd /d C:\Users\bigdata\openhuman

REM Force cc-rs + cmake to use MSVC cl.exe, not LLVM clang-cl
set "CC_x86_64_pc_windows_msvc=cl.exe"
set "CXX_x86_64_pc_windows_msvc=cl.exe"

cargo check --manifest-path Cargo.toml
