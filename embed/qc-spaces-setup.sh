#!/bin/bash
# QuotaVPN Spaces: hub interface and coordinator service.
# Run as root on the QuotaVPN VPS after copying qc-spaces.py beside this file.
# Safe to re-run: it recreates the interface and restarts the coordinator.
set -e

HERE="$(cd "$(dirname "$0")" && pwd)"
IFACE="${QC_SPACES_IFACE:-qcspaces}"
PORT="${QC_SPACES_HUB_PORT:-51821}"
DB="/var/lib/qc-spaces/spaces.db"
ENV_FILE="/etc/qc-spaces/env"

if [ "$(id -u)" != "0" ]; then
  echo "run this as root" >&2
  exit 1
fi

if ! command -v wg >/dev/null 2>&1; then
  apt-get update -qq
  apt-get install -y -qq wireguard-tools iptables >/dev/null
fi

mkdir -p /etc/wireguard /etc/qc-spaces /var/lib/qc-spaces
chmod 700 /etc/wireguard /etc/qc-spaces
chmod 750 /var/lib/qc-spaces

if [ ! -f /etc/wireguard/qcspaces.key ]; then
  umask 077
  wg genkey > /etc/wireguard/qcspaces.key
  wg pubkey < /etc/wireguard/qcspaces.key > /etc/wireguard/qcspaces.pub
fi

if [ -f "$HERE/qc-spaces.py" ]; then
  install -m 755 "$HERE/qc-spaces.py" /usr/local/bin/qc-spaces
elif [ ! -x /usr/local/bin/qc-spaces ]; then
  echo "qc-spaces.py was not found beside this script and /usr/local/bin/qc-spaces is missing" >&2
  exit 1
fi

cat > /etc/systemd/system/qc-spaces-net.service <<UNIT
[Unit]
Description=QuotaVPN Spaces WireGuard hub
After=network.target
Before=qc-spaces.service

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/local/bin/qc-spaces-net
ExecStop=/usr/local/bin/qc-spaces-net down

[Install]
WantedBy=multi-user.target
UNIT

cat > /usr/local/bin/qc-spaces-net <<'SH'
#!/bin/bash
set -e
IFACE="${QC_SPACES_IFACE:-qcspaces}"
PORT="${QC_SPACES_HUB_PORT:-51821}"
KEY=/etc/wireguard/qcspaces.key

if [ "$1" = "down" ]; then
  ip link del "$IFACE" 2>/dev/null || true
  exit 0
fi

ip link del "$IFACE" 2>/dev/null || true
ip link add "$IFACE" type wireguard
ip addr add 10.77.0.1/16 dev "$IFACE"
wg set "$IFACE" listen-port "$PORT" private-key "$KEY"
ip link set "$IFACE" up

cat > /etc/sysctl.d/99-qc-spaces.conf <<EOF
net.ipv4.ip_forward=1
EOF
sysctl -q -p /etc/sysctl.d/99-qc-spaces.conf

# The hub is reached through the main QuotaVPN tunnel. A proxy transport
# (VLESS, Hysteria2) dials it from loopback; the main WireGuard transport
# reaches it through wg0. Both are allowed, nothing else is.
iptables -C INPUT -p udp --dport "$PORT" -j DROP 2>/dev/null || \
  iptables -I INPUT 1 -p udp --dport "$PORT" -j DROP
iptables -C INPUT -p udp --dport "$PORT" -s 10.8.0.0/24 -j ACCEPT 2>/dev/null || \
  iptables -I INPUT 1 -p udp --dport "$PORT" -s 10.8.0.0/24 -j ACCEPT
iptables -C INPUT -p udp --dport "$PORT" -s 127.0.0.0/8 -j ACCEPT 2>/dev/null || \
  iptables -I INPUT 1 -p udp --dport "$PORT" -s 127.0.0.0/8 -j ACCEPT

# One chain decides which space may reach which. The coordinator flushes and
# rebuilds the ACCEPT lines; the DROP at the end is the default.
iptables -N QCSPACES 2>/dev/null || true
iptables -F QCSPACES
iptables -A QCSPACES -j DROP
iptables -C FORWARD -i "$IFACE" -o "$IFACE" -j QCSPACES 2>/dev/null || \
  iptables -I FORWARD 1 -i "$IFACE" -o "$IFACE" -j QCSPACES

# Nested MTU leaves 1280 on the space adapter; clamp the MSS so a large SMB
# read does not stall waiting for a segment that will never fit.
iptables -t mangle -C FORWARD -i "$IFACE" -o "$IFACE" -p tcp \
  --tcp-flags SYN,RST SYN -j TCPMSS --set-mss 1200 2>/dev/null || \
  iptables -t mangle -I FORWARD 1 -i "$IFACE" -o "$IFACE" -p tcp \
  --tcp-flags SYN,RST SYN -j TCPMSS --set-mss 1200
SH
chmod 755 /usr/local/bin/qc-spaces-net

TLS_ARGS=""
HOST="127.0.0.1"
if [ -f "$ENV_FILE" ]; then
  # shellcheck disable=SC1090
  . "$ENV_FILE"
  if [ -n "${QC_SPACES_CERT:-}" ] && [ -n "${QC_SPACES_KEY:-}" ]; then
    TLS_ARGS="--tls-cert $QC_SPACES_CERT --tls-key $QC_SPACES_KEY"
    HOST="${QC_SPACES_HOST:-0.0.0.0}"
  fi
fi

cat > /etc/systemd/system/qc-spaces.service <<UNIT
[Unit]
Description=QuotaVPN Spaces coordinator
After=network.target qc-spaces-net.service
Requires=qc-spaces-net.service

[Service]
ExecStart=/usr/local/bin/qc-spaces --db $DB --host $HOST --port ${QC_SPACES_API_PORT:-8445} $TLS_ARGS
Restart=on-failure
RestartSec=3
User=root
UMask=0077

[Install]
WantedBy=multi-user.target
UNIT

systemctl daemon-reload
systemctl enable qc-spaces-net >/dev/null 2>&1 || true
systemctl enable qc-spaces >/dev/null 2>&1 || true
systemctl restart qc-spaces-net
systemctl restart qc-spaces

echo "hub public key:"
cat /etc/wireguard/qcspaces.pub
echo
echo "coordinator: $(systemctl is-active qc-spaces) on $HOST:${QC_SPACES_API_PORT:-8445}"
echo "If TLS was not configured, the coordinator is loopback only. Put a valid"
echo "certificate in $ENV_FILE and re-run this script before opening the port."
