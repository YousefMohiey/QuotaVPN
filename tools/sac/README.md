# Smart App Control helper

Smart App Control (SAC) blocks QuotaVPN because the installer and the app are
unsigned and SAC trusts only certificates from Microsoft's Trusted Root Program.
There is no allow-list and no "run anyway" in enforcement mode. The only free
fix is to change SAC's mode, and since the April 2026 update (KB5083769) that is
reversible without reinstalling Windows.

## On a PC that blocked the installer

1. Double-click `sac-off.reg` and accept the UAC prompt.
2. Reboot, or run `citool.exe -r` in an elevated prompt.
3. Install and run QuotaVPN.

## Restore SAC afterwards

Double-click `sac-on.reg` and reboot. SAC comes back in enforcement mode, and it
will block QuotaVPN again on its next launch, so leave it off if the PC needs the
app. `sac-eval.reg` is the middle setting: the policies load and log, but
unsigned code is usually let through.

## Keep SAC on and let QuotaVPN through

`quotavpn-supplemental.xml` is an App Control supplement aimed at the SAC base
policy. It deploys and signs, but SAC does not authorize it, so on its own it
currently allows nothing. See `notes/SIGNING-AND-SAC.md` for the measured detail.

```powershell
powershell -ExecutionPolicy Bypass -File deploy-supplemental.ps1
powershell -ExecutionPolicy Bypass -File deploy-supplemental.ps1 -Remove
```

## Check the current mode

```powershell
powershell -ExecutionPolicy Bypass -File sac-state.ps1
```

0 is off, 1 is enforce, 2 is evaluation.
