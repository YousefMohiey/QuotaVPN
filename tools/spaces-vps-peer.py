#!/usr/bin/env python3
"""A Spaces layer-2 peer for testing from a Linux box.

Joins a space with an invite, publishes a TCP endpoint, completes the Noise
handshake, and then answers ARP and ICMP for its own space address. The
Windows side sees a normal Ethernet neighbour and a ping that works, which is
what proves the layer-2 path across the real punch without a second PC.

Run on the VPS (it stops qvreg to free port 8444 and restores it on exit):

    sudo systemctl stop qvreg        # the script does this itself
    python3 spaces-vps-peer.py 'qvs1:SPACE:SECRET'

The machine needs the `cryptography` and `noise` Python packages.
"""

import base64
import hashlib
import hmac
import json
import queue
import socket
import struct
import subprocess
import sys
import threading
import time
import urllib.request

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
from noise.connection import Keypair, NoiseConnection

API = "https://quotavpn.duckdns.org/spaces"
VPS_IP = "80.225.89.162"
PORT = 8444
DEVICE = "vps-l2-peer"


def call(method, path, token=None, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(API + path, data=data, method=method)
    req.add_header("content-type", "application/json")
    if token:
        req.add_header("authorization", "Bearer " + token)
    with urllib.request.urlopen(req, timeout=15) as answer:
        raw = answer.read()
    return json.loads(raw or b"{}")


def recv_exact(sock, size):
    data = b""
    while len(data) < size:
        chunk = sock.recv(size - len(data))
        if not chunk:
            raise RuntimeError("peer closed")
        data += chunk
    return data


def send_frame(sock, frame):
    sock.sendall(len(frame).to_bytes(2, "big") + frame)


def read_frame(sock):
    size = int.from_bytes(recv_exact(sock, 2), "big")
    return recv_exact(sock, size)


def checksum(data):
    if len(data) % 2:
        data += b"\x00"
    total = 0
    for i in range(0, len(data), 2):
        total += (data[i] << 8) | data[i + 1]
    while total >> 16:
        total = (total & 0xFFFF) + (total >> 16)
    return (~total) & 0xFFFF


def icmp_reply(packet):
    if len(packet) < 20 or packet[0] >> 4 != 4:
        return None
    ihl = (packet[0] & 0x0F) * 4
    if len(packet) < ihl + 8 or packet[9] != 1:
        return None
    icmp = packet[ihl:]
    if icmp[0] != 8:
        return None
    payload = icmp[8:]
    reply_icmp = b"\x00\x00\x00\x00" + icmp[4:8] + payload
    reply_icmp = reply_icmp[:2] + struct.pack("!H", checksum(reply_icmp)) + reply_icmp[4:]
    ip = bytearray(packet[:ihl])
    ip[12:16], ip[16:20] = packet[16:20], packet[12:16]
    ip[10:12] = b"\x00\x00"
    ip[2:4] = struct.pack("!H", ihl + len(reply_icmp))
    ip[10:12] = struct.pack("!H", checksum(bytes(ip)))
    return bytes(ip) + reply_icmp


def ethernet_reply(frame, mac, ip_bytes):
    """Answer ARP and ICMP. Everything else is ignored."""
    if len(frame) < 14:
        return None
    dst, src = frame[0:6], frame[6:12]
    ethertype = int.from_bytes(frame[12:14], "big")

    if ethertype == 0x0806 and len(frame) >= 42:
        oper = int.from_bytes(frame[20:22], "big")
        if oper != 1 or frame[38:42] != ip_bytes:
            return None
        reply = bytearray()
        reply += b"\xff" * 6 + mac + b"\x08\x06"
        reply += b"\x00\x01\x08\x00\x06\x04\x00\x02"
        reply += mac + ip_bytes
        reply += src + frame[28:32]
        print("ARP", ".".join(str(b) for b in frame[28:32]), flush=True)
        return bytes(reply)

    if ethertype == 0x0800:
        inner = icmp_reply(frame[14:])
        if inner is None:
            return None
        print("ECHO", len(inner), flush=True)
        return src + mac + b"\x08\x00" + inner

    return None


def main():
    if len(sys.argv) < 2:
        raise SystemExit("usage: spaces-vps-peer.py 'qvs1:SPACE:SECRET'")
    invite = sys.argv[1]

    private = X25519PrivateKey.generate()
    private_raw = private.private_bytes(
        serialization.Encoding.Raw,
        serialization.PrivateFormat.Raw,
        serialization.NoEncryption(),
    )
    public_raw = private.public_key().public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw
    )
    public_b64 = base64.b64encode(public_raw).decode()

    joined = call(
        "POST",
        "/v1/spaces/join",
        body={
            "code": invite,
            "password": "",
            "device_name": DEVICE,
            "public_key": public_b64,
        },
    )
    space_id = joined["space"]["id"]
    mode = joined["space"].get("mode", "l3")
    token = joined["member"]["token"]
    member_id = joined["member"]["id"]
    address = joined["member"]["address"].split("/")[0]
    ip_bytes = socket.inet_aton(address)
    digest = hashlib.sha256(b"quota-spaces-mac-v1" + member_id.encode()).digest()
    mac = bytes([0x02, 0x51, 0x56, digest[0], digest[1], digest[2]])
    print("SPACE", space_id, "MODE", mode, flush=True)
    print("MEMBER", member_id, address, ":".join("%02x" % b for b in mac), flush=True)
    if mode != "l2":
        print("WARNING the space is still layer 3; set it to l2 first", flush=True)

    probe = call("POST", "/v1/spaces/" + space_id + "/probe", token, {})
    probe_id = probe["probe_id"]
    endpoint_body = {
        "endpoint": VPS_IP + ":" + str(PORT),
        "tcp_endpoint": VPS_IP + ":" + str(PORT),
    }
    call(
        "POST",
        "/v1/spaces/" + space_id + "/probe/" + probe_id + "/endpoint",
        token,
        endpoint_body,
    )
    print("ENDPOINT", VPS_IP + ":" + str(PORT), flush=True)

    stop = threading.Event()

    def keepalive():
        while not stop.is_set():
            try:
                call("POST", "/v1/spaces/" + space_id + "/heartbeat", token, {})
                call(
                    "POST",
                    "/v1/spaces/" + space_id + "/probe/" + probe_id + "/endpoint",
                    token,
                    endpoint_body,
                )
            except Exception:
                pass
            time.sleep(20)

    threading.Thread(target=keepalive, daemon=True).start()

    subprocess.run(["sudo", "systemctl", "stop", "qvreg"], check=False)
    time.sleep(0.5)
    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind(("0.0.0.0", PORT))
    listener.listen(8)
    listener.settimeout(1.0)
    accepted = queue.Queue()

    def accept_loop():
        while not stop.is_set():
            try:
                conn, addr = listener.accept()
                accepted.put((conn, addr))
                return
            except socket.timeout:
                continue
            except OSError:
                return

    threading.Thread(target=accept_loop, daemon=True).start()
    print("LISTENING", flush=True)

    peer = None
    deadline = time.time() + 120
    while time.time() < deadline:
        status = call("GET", "/v1/spaces/" + space_id + "/probe/" + probe_id, token)
        for p in status.get("peers", []):
            if (
                p.get("member_id") != member_id
                and p.get("name") != DEVICE
                and p.get("public_key")
                and p.get("tcp_endpoint")
            ):
                peer = p
                break
        if peer:
            break
        time.sleep(0.3)
    if not peer:
        raise RuntimeError("the Windows peer never published an endpoint")

    print("PEER", peer["member_id"], peer.get("name"), peer["tcp_endpoint"], flush=True)
    print("SELFKEY", public_b64, flush=True)
    print("PEERKEY", peer.get("public_key", ""), flush=True)
    # The Windows member owns this space, so its engine connects out to the
    # endpoint above. This side only has to be listening when that happens.
    try:
        conn, addr = accepted.get(timeout=90.0)
        print("USED punched", addr, flush=True)
    except queue.Empty:
        raise RuntimeError("the Windows member never connected to the listener")

    conn.settimeout(60)
    peer_public = base64.b64decode(peer["public_key"])
    initiator = member_id < peer["member_id"]
    noise = NoiseConnection.from_name(b"Noise_IK_25519_ChaChaPoly_BLAKE2s")
    noise.set_prologue(b"quota-spaces-direct-v1")
    noise.set_keypair_from_private_bytes(Keypair.STATIC, private_raw)
    if initiator:
        noise.set_as_initiator()
        noise.set_keypair_from_public_bytes(Keypair.REMOTE_STATIC, peer_public)
    else:
        noise.set_as_responder()
    noise.start_handshake()

    if initiator:
        send_frame(conn, b"\x01" + bytes(noise.write_message()))
        frame = read_frame(conn)
        noise.read_message(frame[1:])
    else:
        frame = read_frame(conn)
        noise.read_message(frame[1:])
        send_frame(conn, b"\x01" + bytes(noise.write_message()))
    print("HANDSHAKE", noise.handshake_finished, flush=True)

    h = noise.get_handshake_hash()
    prk = hmac.new(b"quota-spaces-datagram-v1", h, hashlib.sha256).digest()
    i2r = hmac.new(prk, b"i2r\x01", hashlib.sha256).digest()
    r2i = hmac.new(prk, b"r2i\x01", hashlib.sha256).digest()
    send_key, recv_key = (i2r, r2i) if initiator else (r2i, i2r)
    send_aead = ChaCha20Poly1305(send_key)
    recv_aead = ChaCha20Poly1305(recv_key)
    send_counter = 0

    try:
        while True:
            frame = read_frame(conn)
            if not frame or frame[0] != 2 or len(frame) < 25:
                continue
            counter = frame[1:9]
            plain = recv_aead.decrypt(b"\x00\x00\x00\x00" + counter, frame[9:], counter)
            if len(plain) >= 14 and plain[12:14] == b"\x08\x00" and len(plain) > 23:
                print("IP", "proto", plain[23], "len", len(plain), flush=True)
                if plain[23] == 1 and len(plain) >= 42:
                    print("ICMPHEX", plain[14:42].hex(), flush=True)
            reply = ethernet_reply(plain, mac, ip_bytes)
            if reply is None:
                continue
            print("REPLYHEX", reply.hex(), flush=True)
            counter = send_counter.to_bytes(8, "big")
            send_counter += 1
            packet = b"\x02" + counter + send_aead.encrypt(
                b"\x00\x00\x00\x00" + counter, reply, counter
            )
            send_frame(conn, packet)
    except Exception as e:
        print("PEER_END", repr(e), flush=True)
    finally:
        stop.set()
        try:
            listener.close()
        except Exception:
            pass
        try:
            conn.close()
        except Exception:
            pass
        subprocess.run(["sudo", "systemctl", "start", "qvreg"], check=False)
        print("QCREG_RESTORED", flush=True)


if __name__ == "__main__":
    main()
