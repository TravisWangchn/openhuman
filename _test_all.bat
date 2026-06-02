@echo off
set OPENHUMAN_CORE_TOKEN=de1d7994d60575e4c805d0ff2fad7b40ee9904cd3f0d494b226afd14acbe30be

echo ===== _test_rpc.py =====
python C:\Users\bigdata\openhuman\_test_rpc.py
echo.

echo ===== _check_config.py =====
python C:\Users\bigdata\openhuman\_check_config.py
echo.

echo ===== _test_rpc.ps1 =====
powershell -ExecutionPolicy Bypass -File C:\Users\bigdata\openhuman\_test_rpc.ps1
echo.

echo ===== DONE =====
