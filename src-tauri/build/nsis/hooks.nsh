!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\rsiew.exe" uninstall-cleanup'
!macroend