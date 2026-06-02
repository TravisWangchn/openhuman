@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
bash -c "cd /c/Users/bigdata/openhuman && bash scripts/run-dev-win.sh"
