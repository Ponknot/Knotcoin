@echo off
REM Knotcoin Seed Node — Windows Task Scheduler installer
REM Run as Administrator

SET BINARY=%~dp0..\knotcoind.exe
IF "%1" NEQ "" SET BINARY=%1

IF NOT EXIST "%BINARY%" (
    echo ERROR: knotcoind.exe not found at %BINARY%
    echo Usage: install-seed-node-windows.bat C:\path\to\knotcoind.exe
    exit /b 1
)

SET TASK_NAME=KnotcoinSeedNode
SET DATA_DIR=%USERPROFILE%\.knotcoin\mainnet

if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"

REM Remove existing task if present
schtasks /delete /tn "%TASK_NAME%" /f 2>nul

REM Create task: run at login, restart on failure, run even when not logged in
schtasks /create /tn "%TASK_NAME%" ^
  /tr "\"%BINARY%\" --rpc-port=9001 --p2p-port=9000" ^
  /sc ONLOGON ^
  /ru "%USERNAME%" ^
  /rl HIGHEST ^
  /f

REM Start it now
schtasks /run /tn "%TASK_NAME%"

echo Seed node installed as Windows scheduled task: %TASK_NAME%
echo   Status:  schtasks /query /tn %TASK_NAME%
echo   Stop:    schtasks /end /tn %TASK_NAME%
echo   Remove:  schtasks /delete /tn %TASK_NAME% /f
