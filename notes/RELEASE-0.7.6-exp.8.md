# QuotaVPN 0.7.6-exp.8

Windows experimental build. The phone stays at 0.7.5.

## Spaces

- Every space is now its own private OpenRad network. The engine joins only
  the selected space, so peers and broadcast traffic never mix.
- The baseline space is private. Its network is created on the first connect
  and its password is sealed with DPAPI. The old public `QuotaVPN` network has
  been deleted from the server and the service no longer creates it.
- Old layer-3 spaces are removed from the profile. The old space adapter is
  disabled and has no `10.77.x` address.
- Create and join save the space before switching, so a failed switch cannot
  orphan a network or lose its invite. Delete only removes the local record
  after the server confirms.
- Switching verifies that only the selected network remains joined. A failed
  leave stops the engine instead of mixing two spaces.
- The invite input is capped before decoding.

## Server

The self-hosted server keeps a private network after its last member leaves
and promotes the oldest remaining member when the admin switches away. The
patch is in `notes/OPENRAD-SERVER-PATCH.md`.

The public `QuotaVPN` network is gone. Existing peers join by pasting the new
private invite from Spaces.

## Fixes

- The card list can be restored from the DPAPI store when a profile rewrite
  leaves it empty. Main Connect and the Valorant plate work again.
- The main Connect and Valorant paths were rechecked end to end.
- Space passwords use DPAPI.
- Peer RTT checks run in small parallel waves.
- Engine logs rotate at 5 MB.

## Signing

`QuotaVPN.exe` and `QuotaSpace.exe` carry `CompanyName = YousefMohiey` and are
signed with the local `CN=YousefMohiey` certificate. The PFX and password are
restricted to Administrators and SYSTEM.

The certificate is self-signed. UAC and the firewall prompt show the name only
on a PC that has `yousef-codesign.cer` in Trusted Root and Trusted Publishers.
Smart App Control still blocks the installer on a PC with SAC enforced unless
SAC is turned off or the supplemental policy is deployed.
