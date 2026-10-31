# QuotaVPN packet driver

A network filter driver of our own, so the app does not need Npcap and does not
inherit its licence.

Built from Microsoft's NDIS 6 lightweight filter sample, which is published
under the MS-PL and may be modified and redistributed.

## What it is

- `QuotaVpnPkt.sys`, an NDIS 6 lightweight filter bound to the physical adapter
- device name `\Device\QuotaVpnPkt`, service `QuotaVpnPkt`, filter GUID in `filter.h`

## Building

Needs the Windows Driver Kit and the MSVC build tools. No Visual Studio driver
integration is required:

    bash driver/build-driver.sh

## Loading it on a development machine

An unsigned driver will not load on 64-bit Windows. For testing only:

    bcdedit /set testsigning on      (then reboot)
    makecert / svcert a self signed certificate and sign QuotaVpnPkt.sys

For release the driver must be signed through the Microsoft Hardware Dev Center
with an EV certificate, which is the one step that cannot be automated here.
