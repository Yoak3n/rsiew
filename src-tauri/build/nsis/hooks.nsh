!macro NSIS_HOOK_POSTINSTALL
  Rename "$INSTDIR\rsiew-cli.exe" "$INSTDIR\rsiew.com"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\rsiew.exe" uninstall-cleanup'
  Delete "$INSTDIR\rsiew.com"
!macroend