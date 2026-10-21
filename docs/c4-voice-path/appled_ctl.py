#!/usr/bin/env python3
"""Drive appled's :9200 JSON control plane one command at a time.

Lets us run the voice bring-up deliberately and read each reply, rather than
firing a blind script at a live controller. See PROTOCOL.md §0/§8.

SAFETY: do NOT send `provision` or `bridge` to a controller whose appled is
already set up and paired to an Apple TV — those are first-time-setup commands
and can disturb the existing HAP identity/pairing. On a live, working appled use
only: status, remote, activate, button, and the cleanup inverse.

Examples:
  appled_ctl.py 192.168.1.139 status
  appled_ctl.py 192.168.1.139 remote   --id 7 --data '{"active":true,"available":true,"bridged":true,"voice":true}'
  appled_ctl.py 192.168.1.139 activate --identifier 7 --timeout 15000
  appled_ctl.py 192.168.1.139 button   --button 1 --state down --timestamp now
  appled_ctl.py 192.168.1.139 button   --button 1 --state up   --timestamp now
  appled_ctl.py 192.168.1.139 activate --identifier 7 --timeout 0      # deactivate
  appled_ctl.py 192.168.1.139 remote   --id 7 --data '{"active":false,"available":false,"bridged":false,"voice":false}'  # cleanup
  appled_ctl.py 192.168.1.139 raw --json '{"method":"status","args":{}}'
"""
import argparse
import json
import socket
import sys
import time

PORT = 9200


def send(host, port, obj, wait):
    line = json.dumps(obj) + "\n"
    with socket.create_connection((host, port), timeout=8) as s:
        s.settimeout(wait)
        s.sendall(line.encode())
        buf = b""
        deadline = time.time() + wait
        try:
            while time.time() < deadline:
                c = s.recv(2048)
                if not c:
                    break
                buf += c
                if b"\n" in buf:
                    break
        except socket.timeout:
            pass
    sent = json.dumps(obj)
    if not buf:
        print(f">>> {sent}\n<<< (no reply within {wait}s)")
        return None
    text = buf.decode("utf-8", "replace").strip()
    print(f">>> {sent}\n<<< {text}")
    try:
        return json.loads(text.splitlines()[0])
    except Exception:
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("host")
    ap.add_argument("--port", type=int, default=PORT)
    ap.add_argument("--wait", type=float, default=4.0)
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("status")

    r = sub.add_parser("remote")
    r.add_argument("--id", type=int, required=True)
    r.add_argument("--data", default='{"active":true,"available":true,"bridged":true,"voice":true}')

    a = sub.add_parser("activate")
    a.add_argument("--identifier", type=int, required=True)
    a.add_argument("--timeout", type=int, default=15000)

    b = sub.add_parser("button")
    b.add_argument("--button", type=int, required=True)
    b.add_argument("--state", choices=["up", "down", "press"], required=True)
    b.add_argument("--timestamp", default="now", help="ms, or 'now'")
    b.add_argument("--duration", type=int, default=None)

    rw = sub.add_parser("raw")
    rw.add_argument("--json", required=True)

    # first-time setup — guarded; never on a live paired appled
    pr = sub.add_parser("provision")
    pr.add_argument("--uuid", required=True)
    pr.add_argument("--token", required=True)
    pr.add_argument("--force", action="store_true")
    pr.add_argument("--i-understand-this-can-break-pairing", action="store_true")
    br = sub.add_parser("bridge")
    br.add_argument("--force", action="store_true")
    br.add_argument("--i-understand-this-can-break-pairing", action="store_true")

    args = ap.parse_args()
    now_ms = int(time.time() * 1000)

    if args.cmd == "status":
        obj = {"method": "status", "args": {}}
    elif args.cmd == "remote":
        obj = {"method": "remote", "args": {"id": args.id, "data": json.loads(args.data)}}
    elif args.cmd == "activate":
        obj = {"method": "activate", "args": {"identifier": args.identifier, "timeout": args.timeout}}
    elif args.cmd == "button":
        ts = now_ms if args.timestamp == "now" else int(args.timestamp)
        a2 = {"button": args.button, "state": args.state, "timestamp": ts}
        if args.duration is not None:
            a2["duration"] = args.duration
        obj = {"method": "button", "args": a2}
    elif args.cmd == "raw":
        obj = json.loads(args.json)
    elif args.cmd in ("provision", "bridge"):
        if not getattr(args, "i_understand_this_can_break_pairing"):
            sys.exit(f"REFUSED: `{args.cmd}` is first-time setup and can disturb an "
                     f"already-paired appled. Re-run with "
                     f"--i-understand-this-can-break-pairing if this appled is unprovisioned.")
        if args.cmd == "provision":
            obj = {"method": "provision", "args": {"uuid": args.uuid, "token": args.token, "force": args.force}}
        else:
            obj = {"method": "bridge", "args": {"force": args.force}}
    else:
        ap.error("unknown command")

    send(args.host, args.port, obj, args.wait)


if __name__ == "__main__":
    main()
