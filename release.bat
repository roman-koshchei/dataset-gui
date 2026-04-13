@echo off
if not exist ".\src-tauri\target\release\dataset-gui.exe" (
    echo Release executable not found. Building...
    call npm install
    call npm run tauri build
)
start "" ".\src-tauri\target\release\dataset-gui.exe"
