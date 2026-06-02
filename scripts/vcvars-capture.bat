@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" > nul
echo export PATH="%PATH:"=%" > C:\Users\bigdata\openhuman\scripts\.msvc-env.sh
echo export INCLUDE="%INCLUDE:"=%" >> C:\Users\bigdata\openhuman\scripts\.msvc-env.sh
echo export LIB="%LIB:"=%" >> C:\Users\bigdata\openhuman\scripts\.msvc-env.sh
echo export LIBPATH="%LIBPATH:"=%" >> C:\Users\bigdata\openhuman\scripts\.msvc-env.sh
echo export WindowsSdkDir="%WindowsSdkDir:"=%" >> C:\Users\bigdata\openhuman\scripts\.msvc-env.sh
echo export WindowsSDKVersion="%WindowsSDKVersion:"=%" >> C:\Users\bigdata\openhuman\scripts\.msvc-env.sh
echo export VCToolsInstallDir="%VCToolsInstallDir:"=%" >> C:\Users\bigdata\openhuman\scripts\.msvc-env.sh
echo MSVC env captured
