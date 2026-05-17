!macro NSIS_HOOK_POSTINSTALL
  Delete "$INSTDIR\rsiew.com"
  Rename "$INSTDIR\rsiew-entry.exe" "$INSTDIR\rsiew.com"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\rsiew.exe" uninstall-cleanup'
  Delete "$INSTDIR\rsiew.com"
!macroend