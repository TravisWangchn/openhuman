@echo off
set OPENHUMAN_APP_ENV=staging
set OPENHUMAN_CORE_TOKEN=de1d7994d60575e4c805d0ff2fad7b40ee9904cd3f0d494b226afd14acbe30be
echo === Env vars ===
echo OPENHUMAN_APP_ENV=%OPENHUMAN_APP_ENV%
echo OPENHUMAN_CORE_TOKEN=%OPENHUMAN_CORE_TOKEN%
echo === Starting core ===
C:\Users\bigdata\openhuman\target\debug\openhuman-core.exe serve
