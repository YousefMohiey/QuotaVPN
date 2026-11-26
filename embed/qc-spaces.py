#!/usr/bin/env python3
"""QuotaVPN Spaces coordinator.

Standard library only. It owns membership, addresses and invites, and it keeps
the kernel WireGuard hub in step with the database. It is not a data path: no
packet ever reaches this process.

Production run:
    qc-spaces --db /var/lib/qc-spaces/spaces.db \
              --tls-cert /etc/letsencrypt/live/quotavpn.duckdns.org/fullchain.pem \
              --tls-key  /etc/letsencrypt/live/quotavpn.duckdns.org/privkey.pem

Development run (no wg, no firewall, plain HTTP on loopback):
    qc-spaces --dev --port 8445 --db spaces.db
"""

import argparse
import base64
import hashlib
import hmac
import ipaddress
import json
import os
import secrets
import socket
import sqlite3
import ssl
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

SPACE_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
SUBNET_BASE = "10.77"
MAX_SUBNET = 254
MAX_MEMBERS = 254
HUB_PORT = 51821
ONLINE_WINDOW = 90
PBKDF2_ROUNDS = 200_000
BODY_LIMIT = 64 * 1024
DEV_HUB_KEY = base64.b64encode(b"qc-spaces-dev-hub-public-key-32b").decode()

SCHEMA = """
CREATE TABLE IF NOT EXISTS spaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    subnet INTEGER NOT NULL UNIQUE,
    owner TEXT NOT NULL,
    invite_hash TEXT NOT NULL,
    pass_salt TEXT NOT NULL DEFAULT '',
    pass_hash TEXT NOT NULL DEFAULT '',
    mode TEXT NOT NULL DEFAULT 'l3',
    created INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS members (
    id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL,
    name TEXT NOT NULL,
    public_key TEXT NOT NULL,
    address TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    created INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    FOREIGN KEY (space_id) REFERENCES spaces(id)
);
CREATE INDEX IF NOT EXISTS members_space ON members(space_id);
CREATE INDEX IF NOT EXISTS members_token ON members(token_hash);
CREATE UNIQUE INDEX IF NOT EXISTS members_address ON members(space_id, address);
CREATE TABLE IF NOT EXISTS events (
    at INTEGER NOT NULL,
    space_id TEXT NOT NULL,
    member_id TEXT NOT NULL,
    action TEXT NOT NULL
);
"""


def now():
    return int(time.time())


def sha256_hex(text):
    return hashlib.sha256(text.encode()).hexdigest()


def invite_hash(space_id, secret):
    return sha256_hex(f"{space_id}:{secret}")


def pass_hash(password, salt_hex):
    if not password:
        return ""
    salt = bytes.fromhex(salt_hex)
    return hashlib.pbkdf2_hmac(
        "sha256", password.encode(), salt, PBKDF2_ROUNDS
    ).hex()


def new_space_id():
    return "".join(secrets.choice(SPACE_ALPHABET) for _ in range(8))


def new_token():
    return secrets.token_hex(32)


def member_mac(member_id):
    """Must match member_mac in src/spaces.rs: 02:51:56 plus 3 hash bytes."""
    digest = hashlib.sha256(b"quota-spaces-mac-v1" + member_id.encode()).digest()
    return "02:51:56:%02x:%02x:%02x" % (digest[0], digest[1], digest[2])


def valid_public_key(text):
    try:
        raw = base64.b64decode(text.strip(), validate=True)
    except Exception:
        return False
    return len(raw) == 32


def valid_name(text, limit=40):
    text = (text or "").strip()
    return 1 <= len(text) <= limit


class Store:
    def __init__(self, path, dev=False, hub_key=DEV_HUB_KEY, iface="qcspaces"):
        self.dev = dev
        self.hub_key = hub_key
        self.iface = iface
        self.lock = threading.Lock()
        self.db = sqlite3.connect(path, check_same_thread=False)
        self.db.row_factory = sqlite3.Row
        self.db.executescript(SCHEMA)
        # Older databases predate the layer-2 mode column.
        columns = {row["name"] for row in self.db.execute("PRAGMA table_info(spaces)")}
        if "mode" not in columns:
            self.db.execute(
                "ALTER TABLE spaces ADD COLUMN mode TEXT NOT NULL DEFAULT 'l3'"
            )
        self.db.commit()
        self.create_hits = {}
        self.join_fails = {}
        # Direct-path probe state. The HTTP API issues a probe id; the UDP
        # listener records the source address the coordinator sees. The client
        # reads its own endpoint and its peers' endpoints back over HTTP, then
        # punches UDP to the peer directly.
        self.probes = {}
        self.endpoints = {}
        self.tcp_endpoints = {}
        self.probe_lock = threading.Lock()
        self.udp_port = 0

    # ---------- rate limits ----------

    def _allow(self, table, key, limit, window=3600):
        cut = time.monotonic() - window
        with self.lock:
            hits = [t for t in table.get(key, []) if t > cut]
            if len(hits) >= limit:
                table[key] = hits
                return False
            hits.append(time.monotonic())
            table[key] = hits
            return True

    def _fail(self, key):
        with self.lock:
            self.join_fails.setdefault(key, []).append(time.monotonic())

    def join_blocked(self, key):
        cut = time.monotonic() - 3600
        with self.lock:
            hits = [t for t in self.join_fails.get(key, []) if t > cut]
            self.join_fails[key] = hits
            return len(hits) >= 10

    # ---------- WireGuard hub ----------

    def _wg(self, args):
        if self.dev:
            return True
        try:
            subprocess.run(
                ["wg"] + args,
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=5,
            )
        except Exception:
            return False
        return True

    def add_peer(self, public_key, address):
        return self._wg(
            ["set", self.iface, "peer", public_key, "allowed-ips", address + "/32"]
        )

    def del_peer(self, public_key):
        return self._wg(["set", self.iface, "peer", public_key, "remove"])

    def ensure_address(self, subnet):
        """Give the hub an address in the space's own /24.

        Without this the kernel sources hub traffic from 10.77.0.1, which is
        outside the client's AllowedIPs (the space /24) and is dropped. The /24
        address makes the hub 10.77.<n>.1, which is inside it.
        """
        if self.dev:
            return
        subprocess.run(
            [
                "ip",
                "addr",
                "add",
                f"{SUBNET_BASE}.{subnet}.1/24",
                "dev",
                self.iface,
            ],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=5,
        )

    def remove_address(self, subnet):
        if self.dev:
            return
        subprocess.run(
            [
                "ip",
                "addr",
                "del",
                f"{SUBNET_BASE}.{subnet}.1/24",
                "dev",
                self.iface,
            ],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=5,
        )

    def ensure_addresses(self):
        for row in self.db.execute("SELECT subnet FROM spaces").fetchall():
            self.ensure_address(row["subnet"])

    def handshakes(self):
        """Public key -> unix time of the latest handshake, when wg is available."""
        if self.dev:
            return {}
        try:
            out = subprocess.run(
                ["wg", "show", self.iface, "latest-handshakes"],
                check=False,
                capture_output=True,
                text=True,
                timeout=5,
            ).stdout
        except Exception:
            return {}
        result = {}
        for line in out.splitlines():
            parts = line.split()
            if len(parts) == 2 and parts[1].isdigit():
                result[parts[0]] = int(parts[1])
        return result

    def rebuild_firewall(self):
        if self.dev:
            return
        self.ensure_addresses()
        subprocess.run(
            ["iptables", "-N", "QCSPACES"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        subprocess.run(
            ["iptables", "-F", "QCSPACES"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        check = subprocess.run(
            [
                "iptables",
                "-C",
                "FORWARD",
                "-i",
                self.iface,
                "-o",
                self.iface,
                "-j",
                "QCSPACES",
            ],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if check.returncode != 0:
            subprocess.run(
                [
                    "iptables",
                    "-I",
                    "FORWARD",
                    "1",
                    "-i",
                    self.iface,
                    "-o",
                    self.iface,
                    "-j",
                    "QCSPACES",
                ],
                check=False,
            )
        rows = self.db.execute("SELECT subnet FROM spaces ORDER BY subnet").fetchall()
        for row in rows:
            n = row["subnet"]
            subprocess.run(
                [
                    "iptables",
                    "-A",
                    "QCSPACES",
                    "-s",
                    f"{SUBNET_BASE}.{n}.0/24",
                    "-d",
                    f"{SUBNET_BASE}.{n}.0/24",
                    "-j",
                    "ACCEPT",
                ],
                check=False,
            )
        subprocess.run(["iptables", "-A", "QCSPACES", "-j", "DROP"], check=False)

    # ---------- queries ----------

    def _space(self, space_id):
        return self.db.execute(
            "SELECT * FROM spaces WHERE id = ?", (space_id,)
        ).fetchone()

    def _member(self, member_id):
        return self.db.execute(
            "SELECT * FROM members WHERE id = ?", (member_id,)
        ).fetchone()

    def _member_by_token(self, token):
        if not token:
            return None
        row = self.db.execute(
            "SELECT * FROM members WHERE token_hash = ?", (sha256_hex(token),)
        ).fetchone()
        return row

    def _free_subnet(self):
        used = {
            row["subnet"]
            for row in self.db.execute("SELECT subnet FROM spaces").fetchall()
        }
        for n in range(1, MAX_SUBNET + 1):
            if n not in used:
                return n
        return None

    def _free_host(self, space_id, subnet):
        used = {
            int(row["address"].split(".")[-1])
            for row in self.db.execute(
                "SELECT address FROM members WHERE space_id = ?", (space_id,)
            ).fetchall()
        }
        for host in range(2, MAX_MEMBERS + 1):
            if host not in used:
                return host
        return None

    def _member_view(self, row, caller_id, owner_id, shakes):
        pub = row["public_key"]
        if pub in shakes:
            online = (now() - shakes[pub]) <= ONLINE_WINDOW
        else:
            online = (now() - row["last_seen"]) <= ONLINE_WINDOW
        return {
            "id": row["id"],
            "name": row["name"],
            "address": row["address"],
            # The peer's static X25519 key. Not secret; it is what the direct
            # encrypted channel authenticates against.
            "public_key": row["public_key"],
            "online": bool(online),
            "self_pc": row["id"] == caller_id,
            "owner": row["id"] == owner_id,
            "mac": member_mac(row["id"]),
            "last_seen": row["last_seen"],
        }

    def _hub(self, subnet):
        # The hub's own address, not loopback. A proxy transport (VLESS,
        # Hysteria2) reaches it as a local destination on the VPS; the main
        # WireGuard transport reaches it through wg0, where the kernel sees it
        # as local too. Loopback would only work for the proxy transports.
        return {
            "public_key": self.hub_key,
            "endpoint": f"{SUBNET_BASE}.0.1",
            "port": HUB_PORT,
            "subnet": f"{SUBNET_BASE}.{subnet}.0/24",
        }

    def _space_view(self, row):
        return {
            "id": row["id"],
            "name": row["name"],
            "subnet": f"{SUBNET_BASE}.{row['subnet']}.0/24",
            "mode": row["mode"] if "mode" in row.keys() else "l3",
        }

    def _audit(self, space_id, member_id, action):
        self.db.execute(
            "INSERT INTO events (at, space_id, member_id, action) VALUES (?, ?, ?, ?)",
            (now(), space_id, member_id, action),
        )

    # ---------- actions ----------

    def create(self, name, password, device_name, public_key, source, mode="l3"):
        name = (name or "").strip()
        device_name = (device_name or "").strip()
        mode = (mode or "l3").strip().lower()
        if mode not in ("l3", "l2"):
            return 400, {"error": "the space mode must be l2 or l3"}
        if not valid_name(name):
            return 400, {"error": "give the space a name of 1 to 40 characters"}
        if not valid_name(device_name):
            return 400, {"error": "give this device a name of 1 to 40 characters"}
        if len(password or "") > 64:
            return 400, {"error": "the password is too long"}
        if not valid_public_key(public_key):
            return 400, {"error": "the WireGuard public key is malformed"}
        if not self._allow(self.create_hits, source, 5):
            return 429, {"error": "too many spaces from this address, try later"}

        with self.lock:
            subnet = self._free_subnet()
            if subnet is None:
                return 503, {"error": "this server has no free space subnets"}
            space_id = new_space_id()
            while self._space(space_id) is not None:
                space_id = new_space_id()
            secret = secrets.token_hex(16)
            token = new_token()
            member_id = secrets.token_hex(16)
            salt = secrets.token_hex(16)
            created = now()
            address = f"{SUBNET_BASE}.{subnet}.2"
            try:
                self.db.execute(
                    "INSERT INTO spaces (id, name, subnet, owner, invite_hash, "
                    "pass_salt, pass_hash, mode, created) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    (
                        space_id,
                        name,
                        subnet,
                        member_id,
                        invite_hash(space_id, secret),
                        salt,
                        pass_hash(password, salt),
                        mode,
                        created,
                    ),
                )
                self.db.execute(
                    "INSERT INTO members (id, space_id, name, public_key, address, "
                    "token_hash, created, last_seen) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    (
                        member_id,
                        space_id,
                        device_name,
                        public_key.strip(),
                        address,
                        sha256_hex(token),
                        created,
                        created,
                    ),
                )
                self._audit(space_id, member_id, "create")
                self.db.commit()
            except sqlite3.Error:
                self.db.rollback()
                return 500, {"error": "the space could not be saved"}

        self.add_peer(public_key.strip(), address)
        self.rebuild_firewall()
        return 201, {
            "space": {
                "id": space_id,
                "name": name,
                "subnet": f"{SUBNET_BASE}.{subnet}.0/24",
                "mode": mode,
            },
            "member": {
                "id": member_id,
                "name": device_name,
                "address": address + "/32",
                "token": token,
            },
            "hub": self._hub(subnet),
            "invite": f"qvs1:{space_id}:{secret}",
        }

    def join(self, code, password, device_name, public_key, source):
        device_name = (device_name or "").strip()
        if not valid_name(device_name):
            return 400, {"error": "give this device a name of 1 to 40 characters"}
        if not valid_public_key(public_key):
            return 400, {"error": "the WireGuard public key is malformed"}
        if self.join_blocked(source):
            return 429, {"error": "too many join attempts, try again later"}
        try:
            prefix, space_id, secret = (code or "").strip().split(":", 2)
        except ValueError:
            return self._bad_join(source)
        space_id = space_id.strip().upper()
        secret = secret.strip().lower()
        if prefix != "qvs1" or len(space_id) != 8 or len(secret) != 32:
            return self._bad_join(source)
        row = self._space(space_id)
        if row is None:
            return self._bad_join(source)
        if not hmac.compare_digest(row["invite_hash"], invite_hash(space_id, secret)):
            return self._bad_join(source)
        if row["pass_hash"]:
            if not password:
                return self._bad_join(source)
            if not hmac.compare_digest(
                row["pass_hash"], pass_hash(password, row["pass_salt"])
            ):
                return self._bad_join(source)

        with self.lock:
            existing = self.db.execute(
                "SELECT * FROM members WHERE space_id = ? AND public_key = ?",
                (space_id, public_key.strip()),
            ).fetchone()
            token = new_token()
            if existing is not None:
                # A reinstall that kept the key reclaims its old address. A new
                # token is issued so a leaked old one stops working.
                self.db.execute(
                    "UPDATE members SET name = ?, token_hash = ?, last_seen = ? "
                    "WHERE id = ?",
                    (device_name, sha256_hex(token), now(), existing["id"]),
                )
                self._audit(space_id, existing["id"], "rejoin")
                self.db.commit()
                member = dict(existing)
                member["name"] = device_name
                member["last_seen"] = now()
                token_for_answer = token
            else:
                host = self._free_host(space_id, row["subnet"])
                if host is None:
                    return 409, {"error": "this space is full"}
                member_id = secrets.token_hex(16)
                address = f"{SUBNET_BASE}.{row['subnet']}.{host}"
                try:
                    self.db.execute(
                        "INSERT INTO members (id, space_id, name, public_key, "
                        "address, token_hash, created, last_seen) "
                        "VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        (
                            member_id,
                            space_id,
                            device_name,
                            public_key.strip(),
                            address,
                            sha256_hex(token),
                            now(),
                            now(),
                        ),
                    )
                    self._audit(space_id, member_id, "join")
                    self.db.commit()
                except sqlite3.Error:
                    self.db.rollback()
                    return 500, {"error": "the member could not be saved"}
                member = {
                    "id": member_id,
                    "name": device_name,
                    "address": address,
                    "last_seen": now(),
                }
                token_for_answer = token

        self.add_peer(public_key.strip(), member["address"])
        self.rebuild_firewall()
        return 200, {
            "space": self._space_view(row),
            "member": {
                "id": member["id"],
                "name": member["name"],
                "address": member["address"] + "/32",
                "token": token_for_answer,
            },
            "hub": self._hub(row["subnet"]),
        }

    def _bad_join(self, source):
        self._fail(source)
        # Same answer for a bad id, a bad secret, a wrong password and a
        # deleted space. Nothing here confirms that a space exists.
        return 404, {"error": "that invite is not valid"}

    def auth(self, token, space_id):
        member = self._member_by_token(token)
        if member is None or member["space_id"] != space_id:
            return None, None
        space = self._space(space_id)
        if space is None:
            return None, None
        return member, space

    # ---------- direct-path probe ----------

    def probe_create(self, member, space):
        if self.udp_port == 0:
            return 503, {"error": "the direct-path listener is not running"}
        probe_id = secrets.token_hex(16)
        with self.probe_lock:
            self.probes[probe_id] = (space["id"], member["id"], now())
        return 200, {"probe_id": probe_id, "udp_port": self.udp_port}

    def note_probe(self, probe_id, addr):
        with self.probe_lock:
            row = self.probes.get(probe_id)
            if row is None:
                return
            space_id, member_id, _ = row
            self.endpoints[(space_id, member_id)] = (
                f"{addr[0]}:{addr[1]}",
                now(),
            )

    def probe_endpoint(self, member, space, probe_id, endpoint, tcp_endpoint=""):
        """Store the public UDP mapping the client learned from STUN.

        The VPS does not need an open UDP port for this. The client asks a STUN
        server from the same socket it will use for the Noise handshake, then
        reports the mapping over the existing HTTPS channel. The optional TCP
        endpoint is the stable listener port behind the client's NAT.
        """
        try:
            ip_text, port_text = str(endpoint).rsplit(":", 1)
            ipaddress.IPv4Address(ip_text)
            port = int(port_text)
            if not 1 <= port <= 65535:
                raise ValueError("port")
        except Exception:
            return 400, {"error": "the endpoint is not an IPv4 host and port"}
        tcp_text = ""
        if tcp_endpoint:
            try:
                tcp_ip, tcp_port_text = str(tcp_endpoint).rsplit(":", 1)
                ipaddress.IPv4Address(tcp_ip)
                tcp_port = int(tcp_port_text)
                if not 1 <= tcp_port <= 65535:
                    raise ValueError("port")
                tcp_text = f"{tcp_ip}:{tcp_port}"
            except Exception:
                return 400, {"error": "the tcp endpoint is not an IPv4 host and port"}
        with self.probe_lock:
            row = self.probes.get(probe_id)
            if row is None or row[0] != space["id"] or row[1] != member["id"]:
                return 404, {"error": "not found"}
            self.endpoints[(space["id"], member["id"])] = (
                f"{ip_text}:{port}",
                now(),
            )
            if tcp_text:
                self.tcp_endpoints[(space["id"], member["id"])] = (
                    tcp_text,
                    now(),
                )
        return 200, {"ok": True}

    def probe_status(self, member, space, probe_id):
        # Read the roster under the DB lock, then the probe state under its own
        # lock. The UDP listener never touches SQLite, so the two cannot wait
        # on each other.
        with self.lock:
            members = self.db.execute(
                "SELECT id, name, public_key FROM members WHERE space_id = ? ORDER BY created",
                (space["id"],),
            ).fetchall()
        cut = now() - 120
        with self.probe_lock:
            row = self.probes.get(probe_id)
            if row is None or row[0] != space["id"] or row[1] != member["id"]:
                return 404, {"error": "not found"}
            self.endpoints = {
                key: value for key, value in self.endpoints.items() if value[1] > cut
            }
            self.tcp_endpoints = {
                key: value for key, value in self.tcp_endpoints.items() if value[1] > cut
            }
            self.probes = {
                key: value
                for key, value in self.probes.items()
                if value[2] > now() - 300
            }
            self_addr = self.endpoints.get((space["id"], member["id"]))
            peers = []
            for row in members:
                endpoint = self.endpoints.get((space["id"], row["id"]))
                tcp_endpoint = self.tcp_endpoints.get((space["id"], row["id"]))
                peers.append(
                    {
                        "member_id": row["id"],
                        "name": row["name"],
                        "public_key": row["public_key"],
                        "endpoint": endpoint[0] if endpoint else None,
                        "tcp_endpoint": tcp_endpoint[0] if tcp_endpoint else None,
                    }
                )
            return 200, {
                "self": self_addr[0] if self_addr else None,
                "peers": peers,
                "udp_port": self.udp_port,
            }

    def list_space(self, member, space):
        self.db.execute(
            "UPDATE members SET last_seen = ? WHERE id = ?", (now(), member["id"])
        )
        self.db.commit()
        rows = self.db.execute(
            "SELECT * FROM members WHERE space_id = ? ORDER BY created",
            (space["id"],),
        ).fetchall()
        shakes = self.handshakes()
        return 200, {
            "space": self._space_view(space),
            "members": [
                self._member_view(r, member["id"], space["owner"], shakes)
                for r in rows
            ],
            "hub": self._hub(space["subnet"]),
        }

    def leave(self, member, space):
        pub = member["public_key"]
        with self.lock:
            self.db.execute("DELETE FROM members WHERE id = ?", (member["id"],))
            self._audit(space["id"], member["id"], "leave")
            if space["owner"] == member["id"]:
                nxt = self.db.execute(
                    "SELECT id FROM members WHERE space_id = ? ORDER BY created LIMIT 1",
                    (space["id"],),
                ).fetchone()
                if nxt is None:
                    self.db.execute("DELETE FROM spaces WHERE id = ?", (space["id"],))
                    self.remove_address(space["subnet"])
                else:
                    self.db.execute(
                        "UPDATE spaces SET owner = ? WHERE id = ?",
                        (nxt["id"], space["id"]),
                    )
            self.db.commit()
        self.del_peer(pub)
        self.rebuild_firewall()
        return 200, {"ok": True}

    def kick(self, member, space, member_id):
        if space["owner"] != member["id"]:
            return 403, {"error": "only the owner can remove a member"}
        if member_id == member["id"]:
            return 400, {"error": "leave the space instead of removing yourself"}
        row = self._member(member_id)
        if row is None or row["space_id"] != space["id"]:
            return 404, {"error": "that member is not in this space"}
        with self.lock:
            self.db.execute("DELETE FROM members WHERE id = ?", (member_id,))
            self._audit(space["id"], member_id, "kick")
            self.db.commit()
        self.del_peer(row["public_key"])
        self.rebuild_firewall()
        return 200, {"ok": True}

    def rotate_invite(self, member, space):
        if space["owner"] != member["id"]:
            return 403, {"error": "only the owner can change the invite"}
        secret = secrets.token_hex(16)
        with self.lock:
            self.db.execute(
                "UPDATE spaces SET invite_hash = ? WHERE id = ?",
                (invite_hash(space["id"], secret), space["id"]),
            )
            self._audit(space["id"], member["id"], "rotate")
            self.db.commit()
        return 200, {"invite": f"qvs1:{space['id']}:{secret}"}

    def set_mode(self, member, space, mode):
        if space["owner"] != member["id"]:
            return 403, {"error": "only the owner can change the space mode"}
        mode = (mode or "").strip().lower()
        if mode not in ("l3", "l2"):
            return 400, {"error": "the space mode must be l2 or l3"}
        with self.lock:
            self.db.execute(
                "UPDATE spaces SET mode = ? WHERE id = ?", (mode, space["id"])
            )
            self._audit(space["id"], member["id"], "mode")
            self.db.commit()
        return 200, {"mode": mode}

    def delete(self, member, space):
        if space["owner"] != member["id"]:
            return 403, {"error": "only the owner can delete the space"}
        rows = self.db.execute(
            "SELECT public_key FROM members WHERE space_id = ?", (space["id"],)
        ).fetchall()
        with self.lock:
            self.db.execute("DELETE FROM members WHERE space_id = ?", (space["id"],))
            self.db.execute("DELETE FROM spaces WHERE id = ?", (space["id"],))
            self._audit(space["id"], member["id"], "delete")
            self.db.commit()
            self.remove_address(space["subnet"])
        for row in rows:
            self.del_peer(row["public_key"])
        self.rebuild_firewall()
        return 200, {"ok": True}


class Handler(BaseHTTPRequestHandler):
    server_version = "qc-spaces/1"
    store = None

    def log_message(self, fmt, *args):
        # One short line, no bodies and no tokens.
        sys.stderr.write("%s %s\n" % (self.address_string(), fmt % args))

    def _send(self, code, payload):
        body = json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _body(self):
        length = int(self.headers.get("content-length") or 0)
        if length > BODY_LIMIT:
            return None
        raw = self.rfile.read(length) if length else b"{}"
        try:
            data = json.loads(raw.decode() or "{}")
        except Exception:
            return None
        return data if isinstance(data, dict) else None

    def _token(self):
        head = self.headers.get("authorization") or ""
        if not head.lower().startswith("bearer "):
            return ""
        return head[7:].strip()

    def _source(self):
        return self.client_address[0] if self.client_address else "?"

    def _route(self):
        parsed = urlparse(self.path)
        parts = [p for p in parsed.path.split("/") if p]
        return parts

    def do_GET(self):
        parts = self._route()
        if parts == ["health"]:
            return self._send(200, {"ok": True})
        if (
            len(parts) == 5
            and parts[:2] == ["v1", "spaces"]
            and parts[3] == "probe"
        ):
            member, space = self.store.auth(self._token(), parts[2].upper())
            if member is None:
                return self._send(404, {"error": "not found"})
            code, payload = self.store.probe_status(member, space, parts[4])
            return self._send(code, payload)
        if len(parts) == 3 and parts[:2] == ["v1", "spaces"]:
            token = self._token()
            member, space = self.store.auth(token, parts[2].upper())
            if member is None:
                return self._send(404, {"error": "not found"})
            code, payload = self.store.list_space(member, space)
            return self._send(code, payload)
        return self._send(404, {"error": "not found"})

    def do_POST(self):
        parts = self._route()
        body = self._body()
        if body is None:
            return self._send(400, {"error": "that request is not valid JSON"})
        if parts == ["v1", "spaces"]:
            code, payload = self.store.create(
                body.get("name", ""),
                body.get("password", "") or "",
                body.get("device_name", ""),
                body.get("public_key", ""),
                self._source(),
                body.get("mode", "l3"),
            )
            return self._send(code, payload)
        if parts == ["v1", "spaces", "join"]:
            code, payload = self.store.join(
                body.get("code", ""),
                body.get("password", "") or "",
                body.get("device_name", ""),
                body.get("public_key", ""),
                self._source(),
            )
            return self._send(code, payload)
        if (
            len(parts) == 6
            and parts[:2] == ["v1", "spaces"]
            and parts[3] == "probe"
            and parts[5] == "endpoint"
        ):
            member, space = self.store.auth(self._token(), parts[2].upper())
            if member is None:
                return self._send(404, {"error": "not found"})
            code, payload = self.store.probe_endpoint(
                member, space, parts[4], body.get("endpoint", ""), body.get("tcp_endpoint", "")
            )
            return self._send(code, payload)
        if len(parts) == 4 and parts[:2] == ["v1", "spaces"]:
            space_id = parts[2].upper()
            action = parts[3]
            member, space = self.store.auth(self._token(), space_id)
            if member is None:
                return self._send(404, {"error": "not found"})
            if action == "heartbeat":
                code, payload = self.store.list_space(member, space)
                return self._send(code, payload)
            if action == "probe":
                code, payload = self.store.probe_create(member, space)
                return self._send(code, payload)
            if action == "leave":
                code, payload = self.store.leave(member, space)
                return self._send(code, payload)
            if action == "kick":
                code, payload = self.store.kick(
                    member, space, body.get("member_id", "")
                )
                return self._send(code, payload)
            if action == "invite":
                code, payload = self.store.rotate_invite(member, space)
                return self._send(code, payload)
            if action == "mode":
                code, payload = self.store.set_mode(
                    member, space, body.get("mode", "")
                )
                return self._send(code, payload)
        return self._send(404, {"error": "not found"})

    def do_DELETE(self):
        parts = self._route()
        if len(parts) == 3 and parts[:2] == ["v1", "spaces"]:
            member, space = self.store.auth(self._token(), parts[2].upper())
            if member is None:
                return self._send(404, {"error": "not found"})
            code, payload = self.store.delete(member, space)
            return self._send(code, payload)
        return self._send(404, {"error": "not found"})


def hub_key(path, explicit):
    if explicit:
        return explicit
    if path and os.path.exists(path):
        with open(path, "r", encoding="utf-8") as f:
            return f.read().strip()
    return DEV_HUB_KEY


def udp_probe_loop(sock, store):
    """Record the source address of each probe hello.

    The client keeps this same socket for the peer punch, so the mapping it
    learns here is the mapping its peer will try.
    """
    while True:
        try:
            data, addr = sock.recvfrom(2048)
        except OSError:
            return
        if not data.startswith(b"QVP1 "):
            continue
        probe_id = data[5:].strip().decode("ascii", "ignore")
        if probe_id:
            store.note_probe(probe_id, addr)


def selftest():
    import tempfile

    fd, path = tempfile.mkstemp(suffix=".db")
    os.close(fd)
    os.unlink(path)
    store = Store(path, dev=True)
    udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    udp.bind(("127.0.0.1", 0))
    store.udp_port = udp.getsockname()[1]
    threading.Thread(
        target=udp_probe_loop, args=(udp, store), daemon=True
    ).start()
    Handler.store = store
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base = f"http://127.0.0.1:{port}"
    key_a = base64.b64encode(secrets.token_bytes(32)).decode()
    key_b = base64.b64encode(secrets.token_bytes(32)).decode()

    def call(method, route, body=None, token=""):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(base + route, data=data, method=method)
        req.add_header("content-type", "application/json")
        if token:
            req.add_header("authorization", "Bearer " + token)
        try:
            with urllib.request.urlopen(req, timeout=5) as res:
                return res.status, json.loads(res.read())
        except urllib.error.HTTPError as e:
            return e.code, json.loads(e.read())

    try:
        code, created = call(
            "POST",
            "/v1/spaces",
            {
                "name": "Test space",
                "password": "secret",
                "device_name": "PC A",
                "public_key": key_a,
            },
        )
        assert code == 201, (code, created)
        assert created["invite"].startswith("qvs1:")
        assert created["member"]["address"].endswith(".2/32")
        space_id = created["space"]["id"]
        token_a = created["member"]["token"]

        code, joined = call(
            "POST",
            "/v1/spaces/join",
            {
                "code": created["invite"],
                "password": "secret",
                "device_name": "PC B",
                "public_key": key_b,
            },
        )
        assert code == 200, (code, joined)
        assert joined["member"]["address"].endswith(".3/32")
        token_b = joined["member"]["token"]

        code, listed = call("GET", f"/v1/spaces/{space_id}", token=token_a)
        assert code == 200 and len(listed["members"]) == 2, (code, listed)
        me = [m for m in listed["members"] if m["self_pc"]]
        assert len(me) == 1 and me[0]["name"] == "PC A"

        code, probe_a = call(
            "POST", f"/v1/spaces/{space_id}/probe", token=token_a
        )
        assert code == 200, (code, probe_a)
        code, probe_b = call(
            "POST", f"/v1/spaces/{space_id}/probe", token=token_b
        )
        assert code == 200, (code, probe_b)
        udp_a = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        udp_a.bind(("127.0.0.1", 0))
        udp_b = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        udp_b.bind(("127.0.0.1", 0))
        udp_a.sendto(
            ("QVP1 " + probe_a["probe_id"]).encode(),
            ("127.0.0.1", probe_a["udp_port"]),
        )
        udp_b.sendto(
            ("QVP1 " + probe_b["probe_id"]).encode(),
            ("127.0.0.1", probe_b["udp_port"]),
        )
        code, reported = call(
            "POST",
            f"/v1/spaces/{space_id}/probe/{probe_a['probe_id']}/endpoint",
            {"endpoint": "198.51.100.7:52000"},
            token=token_a,
        )
        assert code == 200, (code, reported)
        code, reported = call(
            "POST",
            f"/v1/spaces/{space_id}/probe/{probe_b['probe_id']}/endpoint",
            {"endpoint": "198.51.100.8:52001"},
            token=token_b,
        )
        assert code == 200, (code, reported)
        status = None
        for _ in range(40):
            code, status = call(
                "GET",
                f"/v1/spaces/{space_id}/probe/{probe_a['probe_id']}",
                token=token_a,
            )
            if (
                code == 200
                and status.get("self")
                and any(
                    p["endpoint"] for p in status["peers"] if p["member_id"] != me[0]["id"]
                )
            ):
                break
            time.sleep(0.05)
        assert code == 200 and status["self"], (code, status)
        assert any(
            p["endpoint"] for p in status["peers"] if p["member_id"] != me[0]["id"]
        ), status
        peer = [p for p in status["peers"] if p["member_id"] != me[0]["id"]][0]
        assert peer["endpoint"] == "198.51.100.8:52001", peer
        udp_a.close()
        udp_b.close()

        code, bad = call(
            "POST",
            "/v1/spaces/join",
            {
                "code": created["invite"],
                "password": "wrong",
                "device_name": "PC C",
                "public_key": base64.b64encode(secrets.token_bytes(32)).decode(),
            },
        )
        assert code == 404, (code, bad)

        code, left = call("POST", f"/v1/spaces/{space_id}/leave", token=token_b)
        assert code == 200, (code, left)
        code, listed = call("GET", f"/v1/spaces/{space_id}", token=token_a)
        assert code == 200 and len(listed["members"]) == 1, (code, listed)

        code, second = call(
            "POST",
            "/v1/spaces",
            {
                "name": "Second",
                "password": "",
                "device_name": "PC D",
                "public_key": base64.b64encode(secrets.token_bytes(32)).decode(),
            },
        )
        assert code == 201
        assert second["space"]["subnet"] != created["space"]["subnet"]

        code, gone = call("DELETE", f"/v1/spaces/{space_id}", token=token_a)
        assert code == 200, (code, gone)
        code, gone = call("GET", f"/v1/spaces/{space_id}", token=token_a)
        assert code == 404, (code, gone)
        print("SPACES SELFTEST OK")
    finally:
        server.shutdown()
        server.server_close()
        udp.close()
        store.db.close()
        try:
            os.unlink(path)
        except OSError:
            pass


def main():
    ap = argparse.ArgumentParser(description="QuotaVPN Spaces coordinator")
    ap.add_argument("--host", default="")
    ap.add_argument("--port", type=int, default=8445)
    ap.add_argument("--udp-port", type=int, default=8446)
    ap.add_argument("--udp-host", default="")
    ap.add_argument("--db", default="/var/lib/qc-spaces/spaces.db")
    ap.add_argument("--tls-cert", default="")
    ap.add_argument("--tls-key", default="")
    ap.add_argument("--wg-interface", default="qcspaces")
    ap.add_argument("--hub-key-file", default="/etc/wireguard/qcspaces.pub")
    ap.add_argument("--hub-key", default="")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()

    if args.selftest:
        selftest()
        return

    if args.dev:
        # Loopback by default, but an explicit --host is honoured so two PCs
        # on one LAN can use the dev coordinator without any public deployment.
        if not args.host:
            args.host = "127.0.0.1"
        if args.db == "/var/lib/qc-spaces/spaces.db":
            args.db = "spaces-dev.db"
    elif not args.host:
        args.host = "0.0.0.0"
    os.makedirs(os.path.dirname(os.path.abspath(args.db)), exist_ok=True)
    key = DEV_HUB_KEY if args.dev else hub_key(args.hub_key_file, args.hub_key)
    store = Store(args.db, dev=args.dev, hub_key=key, iface=args.wg_interface)
    store.rebuild_firewall()
    udp_host = args.udp_host or ("127.0.0.1" if args.dev else "0.0.0.0")
    udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    udp.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        udp.bind((udp_host, args.udp_port))
        store.udp_port = udp.getsockname()[1]
        threading.Thread(
            target=udp_probe_loop, args=(udp, store), daemon=True
        ).start()
    except OSError as error:
        print(f"qc-spaces: direct-path UDP listener failed: {error}", file=sys.stderr)
    Handler.store = store
    server = ThreadingHTTPServer((args.host, args.port), Handler)
    if args.tls_cert and args.tls_key and not args.dev:
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.load_cert_chain(args.tls_cert, args.tls_key)
        server.socket = ctx.wrap_socket(server.socket, server_side=True)
    scheme = "https" if args.tls_cert and not args.dev else "http"
    print(
        f"qc-spaces listening on {scheme}://{args.host}:{args.port}, "
        f"probe udp/{store.udp_port}"
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        udp.close()
        store.db.close()


if __name__ == "__main__":
    main()
