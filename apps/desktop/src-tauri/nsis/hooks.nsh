!macro BERTH_USER_PATH ACTION
  nsExec::Exec `powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$dir = '$INSTDIR'; $$entries = @([string][Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $$_ -and $$_ -ne $$dir }); if ('${ACTION}' -eq 'add') { $$entries += $$dir }; [Environment]::SetEnvironmentVariable('Path', ($$entries -join ';'), 'User')"`
  Pop $0
!macroend

!macro NSIS_HOOK_PREINSTALL
  Delete "$INSTDIR\berth.old.exe"
  IfFileExists "$INSTDIR\berth.exe" 0 +2
    Rename "$INSTDIR\berth.exe" "$INSTDIR\berth.old.exe"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Delete "$INSTDIR\berth.old.exe"
  !insertmacro BERTH_USER_PATH add
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$INSTDIR\berth.old.exe"
  !insertmacro BERTH_USER_PATH remove
!macroend
