#!/usr/bin/env python3
"""Call a newbound store command via `newbound mcp` (stdin JSON-RPC).

The native path is the harness attaching `newbound mcp` through the
checkout's .mcp.json — every store command is then a first-class tool.
This driver is the fallback for sessions where that attachment failed
(e.g. the binary didn't exist yet when the session started): the same
tool surface, driven by hand.

Usage:
  nb-call.py [-C <checkout>] <tool-name> ['<json-arguments>']
  nb-call.py [-C <checkout>] --list [prefix]

Checkout resolution: -C argument, else the current directory, else a
`newbound` directory beside this repo. Every declared param must be
present in the arguments JSON — there are no optional parameters.
"""
import json, os, subprocess, sys, tempfile

args = sys.argv[1:]
DIR = None
if args and args[0] == "-C":
    DIR = os.path.abspath(args[1])
    args = args[2:]
if DIR is None:
    here = os.path.dirname(os.path.abspath(__file__))
    for cand in [os.getcwd(), os.path.join(here, "..", "..", "newbound")]:
        if os.path.isfile(os.path.join(cand, "target/release/newbound")):
            DIR = os.path.abspath(cand)
            break
if DIR is None or not os.path.isfile(os.path.join(DIR, "target/release/newbound")):
    sys.exit("error: no built newbound checkout found (use -C, or build via tools/setup.sh)")
if not args:
    sys.exit(__doc__.strip())

ERRLOG = tempfile.NamedTemporaryFile(prefix="nb-call-", suffix=".log",
                                     delete=False, mode="w")
p = subprocess.Popen(["./target/release/newbound", "mcp"], cwd=DIR,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=ERRLOG, text=True, bufsize=1)
_id = [0]

def rpc(method, params):
    _id[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method,
                              "params": params, "id": _id[0]}) + "\n")
    p.stdin.flush()
    while True:
        line = p.stdout.readline()
        if not line:
            err = open(ERRLOG.name).read()
            raise RuntimeError("server exited: " + err[-2000:])
        line = line.strip()
        if line.startswith("{"):
            return json.loads(line)

rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                   "clientInfo": {"name": "nb-call", "version": "0"}})

status = 0
if args[0] == "--list":
    r = rpc("tools/list", {})
    prefix = args[1] if len(args) > 1 else ""
    for t in r["result"]["tools"]:
        if t["name"].startswith(prefix):
            print(t["name"], "—", (t.get("description") or "")[:120])
else:
    name = args[0]
    arguments = json.loads(args[1]) if len(args) > 1 else {}
    r = rpc("tools/call", {"name": name, "arguments": arguments})
    if "error" in r:
        print(json.dumps(r["error"], indent=1))
        status = 1
    else:
        res = r["result"]
        for c in res.get("content", []):
            text = c.get("text", "")
            try:
                print(json.dumps(json.loads(text), indent=1))
            except Exception:
                print(text)
        if res.get("isError"):
            status = 1

p.stdin.close()
p.terminate()
sys.exit(status)
