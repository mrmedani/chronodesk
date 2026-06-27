@echo off
echo Building CHRONODESK for Windows...

:: Build Rust core
cargo build --release
if %errorlevel% neq 0 exit /b %errorlevel%

:: Build Flutter UI
cd chronodesk_app
flutter build windows --release
if %errorlevel% neq 0 exit /b %errorlevel%

echo Build complete.
