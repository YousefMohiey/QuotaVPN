# Capturing the app window

The agent shell runs in **session 0**. Anything it starts runs there too, where
no window can be shown, and anything it tries to enumerate or paint belongs to
another session's desktop: `Get-Process` reports `MainWindowHandle = 0`,
`EnumWindows` returns nothing, and `PrintWindow` gives a black bitmap. That is
not a broken app, it is the wrong session.

## What works

Run the click and the capture **inside the interactive session** through a
scheduled task:

```powershell
schtasks /Create /TN "qc-shot" /TR "powershell -NoProfile -ExecutionPolicy Bypass -Command \"& 'C:\Tools\qc-ship\shot-settings.ps1' -Out 'C:\Tools\qc-ship\evidence\shot.png'\"" /SC ONCE /ST 23:59 /IT /RU Administrator /RL HIGHEST /F
schtasks /Run /TN "qc-shot"
# wait for the script to finish, then:
schtasks /Delete /TN "qc-shot" /F
```

`C:\Tools\qc-ship\shot-settings.ps1` finds the visible QuotaVPN window by PID,
clicks Settings at client (165, 391), and captures with
`PrintWindow(hwnd, dc, 3)` (`PW_CLIENTONLY | PW_RENDERFULLCONTENT`).

## Launchers

- Start the app through `schtasks /Run` with `/IT /RU Administrator /RL HIGHEST`
  so it lands in session 1. A plain `Start-Process` from the agent shell puts it
  in session 0 where it never appears.
- `%LOCALAPPDATA%\QuotaVPN\QuotaVPN.exe` is the installed binary; version check
  with `(Get-Item ...).VersionInfo.FileVersion`.

## Traps

- A client rect of `0 x 0` or `6 x 6` means the window is collapsed, not closed.
  `SetWindowPos` it back to 1600x1000 before capturing.
- A capture that comes back a few KB and uniformly black is the session problem,
  not a render bug. Check `tasklist /FO TABLE` for the Session# column first.
- Never grab the window rectangle instead of the client rect: that drags the
  desktop, the taskbar and whatever is behind the frame into the image.
