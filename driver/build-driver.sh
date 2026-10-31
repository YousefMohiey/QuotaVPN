#!/usr/bin/env bash
# Builds the QuotaVPN packet driver.
#
# The WDK's Visual Studio integration is not needed for a driver this small:
# cl and link with the kernel headers and libraries produce the same .sys, and
# a plain script keeps the build reproducible on any machine with the WDK.
set -euo pipefail

MSVC="${MSVC:-C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC/14.44.35207}"
KITS="${KITS:-C:/Program Files (x86)/Windows Kits/10}"
SDK="${SDK:-10.0.26100.0}"

CL="$MSVC/bin/Hostx64/x64/cl.exe"
LINK="$MSVC/bin/Hostx64/x64/link.exe"

cd "$(dirname "$0")/QuotaVpnPkt"
mkdir -p obj

DEFS=(/D_AMD64_=1 /D_WIN64 /DUNICODE /D_UNICODE /DNDIS_WDM=1 /DNDISLWF=1 /DNDIS630=1 /DPOOL_NX_OPTIN=1)
INCS=(
  /I.
  "/I$KITS/Include/$SDK/km"
  "/I$KITS/Include/$SDK/km/crt"
  "/I$KITS/Include/$SDK/shared"
  "/I$KITS/Include/$SDK/ucrt"
  "/I$MSVC/include"
)

for f in device filter flt_dbg tap; do
  if [ -f "$f.c" ]; then
    echo "  cl $f.c"
    "$CL" /nologo /c "${DEFS[@]}" "${INCS[@]}" "/Foobj/$f.obj" "/Tc$f.c"
  fi
done

echo "  link QuotaVpnPkt.sys"
"$LINK" /nologo /DRIVER /SUBSYSTEM:NATIVE /ENTRY:DriverEntry \
  "/OUT:QuotaVpnPkt.sys" /NODEFAULTLIB \
  "/LIBPATH:$KITS/Lib/$SDK/km/x64" \
  obj/*.obj ndis.lib ntoskrnl.lib wdmsec.lib BufferOverflowK.lib libcntpr.lib

ls -la QuotaVpnPkt.sys
