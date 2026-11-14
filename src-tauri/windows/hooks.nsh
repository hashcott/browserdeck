; Uninstall cleanup for what Browserdeck creates outside its own files (the template already removes the per-user
; Run value). Only on a real uninstall: not when the updater passes /UPDATE ($UpdateMode = 1), and not when a newer
; installer runs this uninstaller in place to replace the old version ($EXEDIR = $INSTDIR; an uninstall from
; Settings > Apps runs a copy from %TEMP%). So an upgrade keeps the administrator-mode pairing code and sign-in task.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
  ${AndIf} $EXEDIR != $INSTDIR
    ; Start with Windows in administrator mode: the elevated sign-in task.
    nsExec::Exec '"$SYSDIR\schtasks.exe" /Delete /TN browserdeck-autostart /F'
    Pop $0
    ; Administrator mode: the pairing codes (64-bit registry view, where the app writes them) and the elevated
    ; copy's WebView2 data next to the exe.
    SetRegView 64
    DeleteRegKey HKLM "SOFTWARE\Browserdeck"
    SetRegView lastused
    RMDir /r "$INSTDIR\WebView2Admin"
    RMDir "$INSTDIR"
  ${EndIf}
!macroend
