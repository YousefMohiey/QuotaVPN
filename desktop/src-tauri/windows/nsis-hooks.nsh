; QuotaVPN NSIS installer hooks (bundle.windows.nsis.installerHooks).
;
; Two jobs:
;
; 1) Shortcut hygiene. The app used to be called QuotaCards, and an update
;    skips the template's shortcut creation ($UpdateMode), so old-name
;    shortcuts could survive forever. Old installs may also have used the
;    per-machine layout, which puts shortcuts in the shared (Public) desktop
;    and the shared start menu, where a per-user cleanup never looks. A
;    short-lived 0.3.2 test build briefly named the shortcut "Quota"; that
;    name is legacy too. On every install and update: remove every legacy
;    shortcut name from every location Windows can show, then make sure the
;    single current shortcut (named after the app, QuotaVPN) exists.
;
; 2) Shell icon cache refresh, so nobody sees a stale icon after an update.
;    That repair used to be a manual script the user had to find and run;
;    the installer now does it on every install and every in-app update.

!macro QCVPN_PURGE_LEGACY_SHORTCUTS
  ; Shared locations first (per-machine era). A per-user installer may not
  ; have rights here; the reboot fallback covers that without failing.
  SetShellVarContext all
  Delete /REBOOTOK "$DESKTOP\QuotaCards.lnk"
  Delete /REBOOTOK "$DESKTOP\Quota.lnk"
  Delete /REBOOTOK "$SMPROGRAMS\QuotaCards.lnk"
  Delete /REBOOTOK "$SMPROGRAMS\Quota.lnk"
  RMDir /r "$SMPROGRAMS\QuotaCards"
  ; Older installers shipped two command line harnesses next to the app.
  ; They are developer tools and never belonged to the product.
  Delete "$INSTDIR\clitest.exe"
  Delete "$INSTDIR\spdtest.exe"
  RMDir /r "$SMPROGRAMS\Quota"
  SetShellVarContext current

  ; Per-user locations. The extra profile paths cover a desktop that was
  ; moved by OneDrive redirection after the old shortcut was created.
  Delete "$DESKTOP\QuotaCards.lnk"
  Delete "$DESKTOP\Quota.lnk"
  Delete "$PROFILE\Desktop\QuotaCards.lnk"
  Delete "$PROFILE\Desktop\Quota.lnk"
  Delete "$PROFILE\OneDrive\Desktop\QuotaCards.lnk"
  Delete "$PROFILE\OneDrive\Desktop\Quota.lnk"
  Delete "$SMPROGRAMS\QuotaCards.lnk"
  Delete "$SMPROGRAMS\Quota.lnk"
  RMDir /r "$SMPROGRAMS\QuotaCards"
  RMDir /r "$SMPROGRAMS\Quota"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; The installer deliberately asks for nothing and registers nothing.
  ;
  ; It used to spawn `--setup-helper` hidden and elevated from here, and that
  ; pass registers scheduled tasks. Defender's behaviour model read exactly that
  ; and called the installer Behavior:Win32/Persistence.A!ml: an unsigned
  ; installer that launches a hidden elevated process which then installs
  ; persistence. It quarantined the download on sight, twice.
  ;
  ; The registration moved to the app, on the first connect, where a person
  ; presses a button and Windows shows them the prompt. Same outcome, and the
  ; elevation happens in front of them instead of behind an installer.
  ; `setup_helper` in the app calls `--setup-helper` and the connect button
  ; offers it whenever the helper is missing, so nothing is lost but the
  ; silent part.

  ; This installer registers nothing, schedules nothing, and starts nothing.
  ;
  ; It tried twice. First it spawned a hidden elevated `--setup-helper`, and
  ; Defender called that Behavior:Win32/Persistence.A!ml and quarantined the
  ; download. Then, once the install was per-machine, it ran the setup pass
  ; directly - same verdict, same outcome, because the behaviour model is reading
  ; "an installer created a scheduled task that launches this program", and it
  ; does not care whose token was used.
  ;
  ; The tasks are gone from the product entirely now. The app already runs with
  ; an administrator token, so it starts its engine as a detached child and
  ; registers nothing. Nothing needs doing here.

  !insertmacro QCVPN_PURGE_LEGACY_SHORTCUTS

  ; Exactly one shortcut, named after the app, always refreshed here. The
  ; template skips its own creation on updates, so this is what keeps the
  ; shortcut correct on every install and every in-app update.
  CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
  CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"

  DetailPrint "Refreshing the shell icon cache"
  nsExec::ExecToLog 'cmd /c del /f /q "%LOCALAPPDATA%\IconCache.db" 2>nul & del /f /q "%LOCALAPPDATA%\Microsoft\Windows\Explorer\iconcache_*.db" 2>nul'
  FindFirst $0 $1 "$LOCALAPPDATA\Microsoft\Windows\Explorer\iconcache_*.db"
  iconcache_loop:
    StrCmp $1 "" iconcache_done
    Delete /REBOOTOK "$LOCALAPPDATA\Microsoft\Windows\Explorer\$1"
    FindNext $0 $1
    Goto iconcache_loop
  iconcache_done:
  FindClose $0
  nsExec::ExecToLog '"$SYSDIR\ie4uinit.exe" -ClearIconCache'
  nsExec::ExecToLog '"$SYSDIR\ie4uinit.exe" -show'
  ; An in-app update ends the running app before the files are replaced and the
  ; app never gets to start itself again. It leaves a marker saying this
  ; install came from inside it, so the window comes back on the new build
  ; instead of the user having to hunt for the shortcut. A fresh install
  ; writes no marker and starts nothing.
  ; IfFileExists jumps to its second argument when the file EXISTS, so the
  ; label has to sit on the not-exists branch: 0 means fall through and do
  ; the relaunch, anything else skips it.
  IfFileExists "$LOCALAPPDATA\quotacards\updating" 0 qvp_no_relaunch
    Delete "$LOCALAPPDATA\quotacards\updating"
    ExecShell "" "$INSTDIR\${MAINBINARYNAME}.exe"
  qvp_no_relaunch:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; The template removes "$DESKTOP\${PRODUCTNAME}.lnk" itself; clean the
  ; legacy names here too. Restore the shell var context at the end so the
  ; rest of the uninstaller is unaffected.
  Delete "$DESKTOP\QuotaCards.lnk"
  Delete "$DESKTOP\Quota.lnk"
  Delete "$SMPROGRAMS\QuotaCards.lnk"
  Delete "$SMPROGRAMS\Quota.lnk"
  SetShellVarContext all
  Delete /REBOOTOK "$DESKTOP\QuotaCards.lnk"
  Delete /REBOOTOK "$DESKTOP\Quota.lnk"
  Delete /REBOOTOK "$SMPROGRAMS\QuotaCards.lnk"
  Delete /REBOOTOK "$SMPROGRAMS\Quota.lnk"
  SetShellVarContext current
  ; The helper task belongs to this install: take it with the app. This runs
  ; before the files go, because afterwards the exe is gone and the task
  ; survives an uninstall for ever.
  ; An in-app update runs this uninstaller too, and the helper has to survive
  ; that: the task is per account and re-registering it means another Windows
  ; prompt. The app drops a marker before it hands over to the updater, so that
  ; marker is the test. A real uninstall has no marker and takes the task.
  ; The helper tasks are removed on every uninstall, with no exception.
  ;
  ; This used to keep them when `%LOCALAPPDATA%\quotacards\updating` existed, so
  ; that an in-app update did not have to ask Windows for administrator again. The
  ; marker is a file in the user's own profile, so anyone could create it, and the
  ; tasks would survive an uninstall and still name this executable. "Uninstall the
  ; app" has to mean the privileged tasks are gone; an update paying one prompt is
  ; the cheaper problem.
  nsExec::ExecToLog '"$INSTDIR\${MAINBINARYNAME}.exe" --remove-helper'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Same refresh after uninstall, so the removed shortcuts leave the shell
  ; cleanly instead of showing broken icons until the next rebuild.
  nsExec::ExecToLog '"$SYSDIR\ie4uinit.exe" -ClearIconCache'
!macroend
