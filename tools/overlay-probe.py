#!/usr/bin/env python3
"""Probe `newbound mcp` on the symlink-overlaid checkout.

Usage: overlay-probe.py [path-to-newbound-checkout]   (default: cwd)

Checks: initialize; tools/list contains dev-code-*, agent-*, kb tools
(libraries discovered through symlinks); a real read call
(dev-code-search_commands) answers; a dylib-dispatched call
(agent-plugin-list_tools) answers — proving FFI crates load through the
overlay.
"""
import json, os, subprocess, sys, tempfile

DIR = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else ".")
if not os.path.isfile(os.path.join(DIR, "target/release/newbound")):
    sys.exit("error: no target/release/newbound under %s — build first (tools/setup.sh)" % DIR)
ERRLOG = tempfile.NamedTemporaryFile(prefix="overlay-probe-", suffix=".log",
                                     delete=False, mode="w")
p = subprocess.Popen(["./target/release/newbound", "mcp"], cwd=DIR,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=ERRLOG, text=True, bufsize=1)
_id = 0
def rpc(method, params):
    global _id
    _id += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method,
                              "params": params, "id": _id}) + "\n")
    p.stdin.flush()
    while True:
        line = p.stdout.readline()
        if not line:
            err = open(ERRLOG.name).read()
            raise RuntimeError("server exited: " + err[-2000:])
        line = line.strip()
        if line.startswith("{"):
            return json.loads(line)

ok = 0
def check(name, cond, detail=""):
    global ok
    print(("PASS " if cond else "FAIL ") + name + (" — " + detail if detail else ""))
    ok += (0 if cond else 1)

r = rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                       "clientInfo": {"name": "overlay-probe", "version": "0"}})
check("initialize", "result" in r, r.get("result", {}).get("serverInfo", {}).get("name", ""))

r = rpc("tools/list", {})
tools = [t["name"] for t in r["result"]["tools"]]
check("tools/list answers", len(tools) > 0, f"{len(tools)} tools")
for prefix in ["dev-code-", "agent-plugin-", "agent-llm-"]:
    got = [t for t in tools if t.startswith(prefix)]
    check(f"{prefix}* present", len(got) > 0, f"{len(got)}")

r = rpc("tools/call", {"name": "dev-code-search_commands",
                       "arguments": {"lib": "", "ctl": "", "query": "delete"}})
txt = json.dumps(r.get("result", ""))
check("search_commands executes", "delete_control" in txt or "delete_command" in txt,
      txt[:120].replace("\n", " "))

r = rpc("tools/call", {"name": "agent-plugin-list_tools", "arguments": {}})
txt = json.dumps(r.get("result", ""))
check("agent dylib dispatch (plugin.list_tools)", "isError" not in json.dumps(r.get("result", {})) and "tools" in txt,
      txt[:120].replace("\n", " "))

p.stdin.close(); p.wait(timeout=10)
print("---")
sys.exit(1 if ok else 0)
