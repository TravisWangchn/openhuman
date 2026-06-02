#!/bin/bash
# Build Tauri app from bash — sets up MSVC environment manually
set -e

MSVC_VER="14.44.35207"
SDK_VER="10.0.26100.0"

MSVC="C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC/${MSVC_VER}"
SDK="C:/Program Files (x86)/Windows Kits/10"

export VCINSTALLDIR="C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\"
export VCToolsInstallDir="${MSVC}\\"
export VCToolsVersion="${MSVC_VER}"
export WindowsSdkDir="${SDK}\\"
export WindowsSDKVersion="${SDK_VER}\\"

export INCLUDE="${MSVC}\\include;${SDK}\\Include\\${SDK_VER}\\ucrt;${SDK}\\Include\\${SDK_VER}\\shared;${SDK}\\Include\\${SDK_VER}\\um;${SDK}\\Include\\${SDK_VER}\\winrt;${SDK}\\Include\\${SDK_VER}\\cppwinrt"
export LIB="${MSVC}\\lib\\x64;${SDK}\\Lib\\${SDK_VER}\\ucrt\\x64;${SDK}\\Lib\\${SDK_VER}\\um\\x64"
export PATH="${MSVC}\\bin\\Hostx64\\x64:${PATH}"

export CC_x86_64_pc_windows_msvc="${MSVC}/bin/Hostx64/x64/cl.exe"
export CXX_x86_64_pc_windows_msvc="${MSVC}/bin/Hostx64/x64/cl.exe"
export CMAKE_GENERATOR="Ninja"

export CEF_PATH="${USERPROFILE}/Library/Caches/tauri-cef"
export PATH="${CEF_PATH}/146.0.9/cef_windows_x86_64:${CEF_PATH}/146.0.9/cef_windows_x86_64/locales:${PATH}"
export OPENHUMAN_CORE_PORT="7788"
export OPENHUMAN_DEV_JWT_TOKEN="dev-bypass-local-zn"
export RUST_LOG="info"
export RUST_BACKTRACE="1"
export OPENHUMAN_DEV_PORT="1421"

echo "[build-tauri] MSVC=${MSVC_VER} SDK=${SDK_VER}"
cd /c/Users/bigdata/openhuman/app
cargo tauri dev "$@"
