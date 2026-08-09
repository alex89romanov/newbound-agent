#!/usr/bin/env python3
"""Smoke-test the P0 agent write API against a DISPOSABLE Newbound instance.

Exercises read_control_facet / patch_control_facet / list_control_patches /
set_library_meta / set_control_meta / set_command_meta (newbound branch
claude/agent-api-p0) end to end on a scratch library it creates itself
(`apitest`), and prints a PASS/FAIL line per check. See CONTRACT §6 for the
contract each check asserts.

This script MUTATES the instance it points at (creates a library, writes
facets and metadata, touches one desc in the agent library). Per the repo's
hard rule it refuses to run without --disposable, and you must only ever
point it at a disposable instance (tools/scratch-instance.md).

The final section exercises the deletion commands and ends by deleting the
scratch library itself, so a completed run leaves the instance clean and the
same --lib name works again. A run that DIES midway leaves the lib half-used —
the setup guard will then ask for a fresh name.

Usage:
  python3 tools/validate-agent-api.py --base http://localhost:33182 \
      --user admin --password <pw> --disposable
  # or with an existing session:
  python3 tools/validate-agent-api.py --base http://localhost:33182 \
      --sessionid <sid> --disposable
"""
import argparse
import json
import sys
import urllib.parse
import urllib.request
import uuid

FAILS = []


def get(base, sid, path, params):
    # CONTRACT §1.2: percent-encode only (%XX); the server does not decode '+'.
    qs = "&".join(
        "{}={}".format(k, urllib.parse.quote(v, safe="")) for k, v in params.items()
    )
    req = urllib.request.Request(base + path + "?" + qs)
    req.add_header("Cookie", "sessionid=" + sid)  # CONTRACT §1.1: cookie, not query
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read().decode("utf-8"))


def get_raw(base, sid, path):
    req = urllib.request.Request(base + path)
    req.add_header("Cookie", "sessionid=" + sid)
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.read().decode("utf-8", "replace")
    except Exception as e:  # noqa: BLE001 — a failed fetch is a failed check
        return "FETCH ERROR: {}".format(e)


def check(name, cond, detail=""):
    tag = "PASS" if cond else "FAIL"
    print("  [{}] {}{}".format(tag, name, (" — " + detail) if (detail and not cond) else ""))
    if not cond:
        FAILS.append(name)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True, help="e.g. http://localhost:33182")
    ap.add_argument("--sessionid", help="existing authenticated (admin) session id")
    ap.add_argument("--user", help="login instead of --sessionid")
    ap.add_argument("--password")
    ap.add_argument("--lib", default="apitest", help="scratch library name to create/use")
    ap.add_argument("--disposable", action="store_true",
                    help="required: confirms the target is a disposable instance")
    a = ap.parse_args()

    if not a.disposable:
        sys.exit("Refusing to run: this script mutates the instance. "
                 "Point it at a disposable instance and pass --disposable.")

    base = a.base.rstrip("/")
    sid = a.sessionid
    if not sid:
        if not (a.user and a.password):
            sys.exit("Need --sessionid, or --user and --password.")
        sid = uuid.uuid4().hex
        r = get(base, sid, "/app/login", {"user": a.user, "pass": a.password})
        if r.get("status") != "ok":
            sys.exit("Login failed: {}".format(r))

    # Discover the agent.code control and its command ids (CONTRACT §2.3).
    controls = get(base, sid, "/app/read", {"lib": "agent", "id": "controls"})
    if controls.get("status") != "ok":
        sys.exit("Cannot read agent controls (is the session admin?): {}".format(controls))
    code_id = next(c["id"] for c in controls["data"]["list"] if c["name"] == "code")
    code = get(base, sid, "/app/read", {"lib": "agent", "id": code_id})
    ids = {c["name"]: c["id"] for c in code["data"]["cmd"]}

    needed = ["add_library", "add_control", "read_command",
              "read_control_facet", "patch_control_facet", "list_control_patches",
              "set_library_meta", "set_control_meta", "set_command_meta",
              "list_assets", "write_asset", "rename_asset", "delete_asset",
              "read_flow_body", "write_flow_body", "invoke_command",
              "set_timer", "remove_timer", "set_event_handler", "remove_event_handler",
              "read_control_scene", "write_control_scene",
              "delete_command", "delete_control", "delete_library"]
    missing = [n for n in needed if n not in ids]
    if missing:
        sys.exit("agent.code is missing {} — is branch claude/agent-api-p0 "
                 "checked out and rebuilt?".format(missing))

    def call(cmd_name, /, **args):
        # positional-only: command params named `name`/`cmd` pass as kwargs
        return get(base, sid, "/app/exec",
                   {"lib": "agent", "id": ids[cmd_name], "args": json.dumps(args)})

    L = a.lib
    print("target: {}  scratch lib: {}".format(base, L))

    print("setup")
    r = call("add_library", lib=L)
    check("add_library", r.get("status") == "ok", str(r))
    r = call("add_control", lib=L, ctl="probe")
    check("add_control", r.get("status") == "ok", str(r))
    r = call("read_control_facet", lib=L, ctl="probe", facet="html")
    if r.get("exists") is True:
        sys.exit("Scratch lib '{}' was already used on this instance — "
                 "re-run with --lib <fresh-name> (e.g. {}2) for a clean run.".format(L, L))

    print("read_control_facet")
    r = call("read_control_facet", lib=L, ctl="probe", facet="html")
    check("absent facet reads ok", r.get("status") == "ok", str(r))
    check("absent facet: exists=false, empty source",
          r.get("exists") is False and r.get("source") == "", str(r))
    r = call("read_control_facet", lib=L, ctl="probe", facet="data")
    check("unsupported facet errors", r.get("status") == "err", str(r))
    r = call("read_control_facet", lib=L, ctl="nope", facet="html")
    check("missing control errors", r.get("status") == "err", str(r))

    print("patch_control_facet")
    html1 = '<div class="probe">hello world</div>'
    r = call("patch_control_facet", lib=L, ctl="probe", facet="html",
             old_snippet="", new_snippet=html1, base="",
             label="initial html", author="validator")
    check("whole-facet create", r.get("status") == "ok" and r.get("patch_id") == "p1", str(r))
    r = call("read_control_facet", lib=L, ctl="probe", facet="html")
    check("created facet reads back",
          r.get("exists") is True and r.get("source") == html1, str(r))
    h = r.get("hash", "")

    r = call("patch_control_facet", lib=L, ctl="probe", facet="html",
             old_snippet="hello world", new_snippet="hello, bench", base=h,
             label="greeting", author="validator")
    check("snippet patch with fresh base", r.get("status") == "ok", str(r))
    r2 = call("read_control_facet", lib=L, ctl="probe", facet="html")
    check("patched source correct", "hello, bench" in r2.get("source", ""), str(r2))

    r = call("patch_control_facet", lib=L, ctl="probe", facet="html",
             old_snippet="bench", new_snippet="x", base=h,
             label="", author="validator")
    check("stale base rejected",
          r.get("status") == "err" and r.get("msg") == "stale_base" and "current_hash" in r,
          str(r))
    r = call("patch_control_facet", lib=L, ctl="probe", facet="html",
             old_snippet="zzz-not-here", new_snippet="x", base="",
             label="", author="validator")
    check("snippet not found rejected",
          r.get("status") == "err" and "not found" in r.get("msg", "").lower(), str(r))

    css = ".a{color:red}\n.b{color:red}\n"
    r = call("patch_control_facet", lib=L, ctl="probe", facet="css",
             old_snippet="", new_snippet=css, base="",
             label="initial css", author="validator")
    check("second facet create", r.get("status") == "ok", str(r))
    r = call("patch_control_facet", lib=L, ctl="probe", facet="css",
             old_snippet="color:red", new_snippet="color:blue", base="",
             label="", author="validator")
    check("ambiguous snippet rejected, no write",
          r.get("status") == "err" and "ambiguous" in r.get("msg", "").lower(), str(r))
    r = call("read_control_facet", lib=L, ctl="probe", facet="css")
    check("ambiguous attempt left source untouched", r.get("source") == css, str(r))

    print("list_control_patches")
    r = call("list_control_patches", lib=L, ctl="probe", limit=0)
    ps = r.get("patches", [])
    check("journal has the 3 applied patches", len(ps) == 3, str(r))
    check("newest first", bool(ps) and ps[0].get("patch_id") == "p3", str(ps[:1]))
    check("entry shape", bool(ps) and all(
        k in ps[0] for k in ("patch_id", "author", "facet", "old", "new", "time", "label")),
        str(ps[:1]))
    r = call("list_control_patches", lib=L, ctl="probe", limit=2)
    check("limit respected", len(r.get("patches", [])) == 2, str(r))

    print("meta setters")
    r = call("set_control_meta", lib=L, ctl="probe",
             desc="A probe control for validating the agent write API.", groups="update")
    check("set_control_meta", r.get("status") == "ok" and r.get("changed") is True, str(r))
    probe_id = next(c["id"] for c in
                    get(base, sid, "/app/read", {"lib": L, "id": "controls"})["data"]["list"]
                    if c["name"] == "probe")
    rec = get(base, sid, "/app/read", {"lib": L, "id": probe_id})
    check("control desc+groups persisted",
          rec["data"].get("desc", "").startswith("A probe control")
          and rec["data"].get("groups") == "update", str({k: rec["data"].get(k) for k in ("desc", "groups")}))

    r = call("set_library_meta", lib=L,
             desc="Scratch library created by tools/validate-agent-api.py.", groups="")
    check("set_library_meta", r.get("status") == "ok" and r.get("changed") is True, str(r))
    # meta.json is written on disk; the in-memory snapshot refreshes on restart.

    print("set_command_meta (target: agent.code.read_control_facet)")
    # Verify against the store records directly (app/read), independent of
    # read_command's desc surfacing — this isolates where a desc goes missing.
    cmdrec = get(base, sid, "/app/read", {"lib": "agent", "id": ids["read_control_facet"]})
    impl_id = cmdrec.get("data", {}).get("rust", "")
    check("command record resolves its impl record", bool(impl_id),
          str(cmdrec.get("data", cmdrec)))
    implrec = get(base, sid, "/app/read", {"lib": "agent", "id": impl_id}) if impl_id else {}
    orig_desc = implrec.get("data", {}).get("desc", "")
    check("impl record carries the authored desc", orig_desc != "",
          "data keys: " + str(sorted(implrec.get("data", {}).keys())))

    marker = "Validation marker desc written by tools/validate-agent-api.py."
    r = call("set_command_meta", lib="agent", ctl="code", cmd="read_control_facet",
             desc=marker, groups="")
    check("set_command_meta writes a marker desc",
          r.get("status") == "ok" and r.get("changed") is True, str(r))
    implrec2 = get(base, sid, "/app/read", {"lib": "agent", "id": impl_id}) if impl_id else {}
    check("marker desc persisted on the impl record",
          implrec2.get("data", {}).get("desc") == marker,
          repr(implrec2.get("data", {}).get("desc", ""))[:160])
    cur = call("read_command", lib="agent", ctl="code", cmd="read_control_facet")
    # read_command's return type is Object, not FLAT — its payload sits under `data`.
    payload = cur.get("data") if isinstance(cur.get("data"), dict) else cur
    check("read_command surfaces the marker desc", payload.get("desc") == marker,
          "status={} msg={} keys={}".format(
              cur.get("status"), cur.get("msg", ""), sorted(cur.keys())[:14]))
    if orig_desc:
        r = call("set_command_meta", lib="agent", ctl="code", cmd="read_control_facet",
                 desc=orig_desc, groups="")
        check("original desc restored",
              r.get("status") == "ok" and r.get("changed") is True, str(r))

    r = call("set_control_meta", lib=L, ctl="probe", desc="", groups="")
    check("empty strings leave fields untouched (changed=false)",
          r.get("status") == "ok" and r.get("changed") is False, str(r))

    print("assets (P1)")
    r = call("list_assets", lib=L)
    check("empty asset list", r.get("status") == "ok" and r.get("assets") == [], str(r))
    css = ".probe{color:red}"
    r = call("write_asset", lib=L, name="probe.css", content=css, tempfile="")
    check("write inline asset",
          r.get("status") == "ok" and r.get("replaced") is False
          and r.get("size") == len(css), str(r))
    r = call("write_asset", lib=L, name="sub/nested.txt", content="nested", tempfile="")
    check("write nested asset", r.get("status") == "ok", str(r))
    r = call("list_assets", lib=L)
    entries = r.get("assets", [])
    names = sorted(a.get("name") for a in entries)
    check("list shows both, relative paths", names == ["probe.css", "sub/nested.txt"], str(names))
    check("entry shape (name/size/time)", bool(entries) and all(
        k in entries[0] for k in ("name", "size", "time")), str(entries[:1]))
    r = call("write_asset", lib=L, name="probe.css", content=".probe{color:blue}", tempfile="")
    check("replace reports replaced=true", r.get("status") == "ok" and r.get("replaced") is True, str(r))

    raw = get_raw(base, sid, "/app/asset/{}/probe.css".format(L))
    check("asset served over HTTP (app/asset)", raw == ".probe{color:blue}",
          repr(raw)[:80])

    r = call("write_asset", lib=L, name="../escape.txt", content="x", tempfile="")
    check("traversal rejected", r.get("status") == "err", str(r))
    r = call("rename_asset", lib=L, **{"from": "probe.css", "to": "styles/probe.css"})
    check("rename into subdir", r.get("status") == "ok", str(r))
    r = call("rename_asset", lib=L, **{"from": "styles/probe.css", "to": "sub/nested.txt"})
    check("rename refuses to clobber", r.get("status") == "err", str(r))
    r = call("delete_asset", lib=L, name="sub/nested.txt")
    check("delete", r.get("status") == "ok" and r.get("removed") is True, str(r))
    r = call("delete_asset", lib=L, name="sub/nested.txt")
    check("delete absent errors", r.get("status") == "err", str(r))
    r = call("list_assets", lib=L)
    names = sorted(a.get("name") for a in r.get("assets", []))
    check("final asset list", names == ["styles/probe.css"], str(names))

    print("flow body (read_flow_body / write_flow_body)")
    r = call("read_flow_body", lib=L, ctl="probe", cmd="passthru")
    check("unknown flow command errs", r.get("status") == "err", str(r))
    # Minimal valid Case: one passthrough wire, every Node carrying the
    # parser-required mode/type/x (flow3d-design R-1). Terminal types are
    # `integer` because the derived signature is ENFORCED: write_flow_body
    # maps the bars to the command's params/returntype, and cast_params
    # really coerces — invoking with a=41 against an `object` param panics
    # the cast (caught by the first run of this section).
    body1 = {"input": {"a": {"mode": "regular", "type": "integer", "x": 0}},
             "output": {"a": {"mode": "regular", "type": "integer", "x": 0}},
             "cmds": [], "cons": [{"src": [-1, "a"], "dest": [-2, "a"]}]}
    r = call("write_flow_body", lib=L, ctl="probe", cmd="passthru", body=body1,
             base="", label="create passthru", author="validator")
    check("write creates the flow command",
          r.get("status") == "ok" and r.get("created") is True
          and r.get("patch_id"), str(r))
    r = call("read_flow_body", lib=L, ctl="probe", cmd="passthru")
    check("body reads back",
          r.get("status") == "ok" and r.get("exists") is True
          and r.get("body", {}).get("cons") == [{"src": [-1, "a"], "dest": [-2, "a"]}],
          str(r)[:200])
    h = r.get("hash", "")
    r = call("write_flow_body", lib=L, ctl="probe", cmd="passthru", body=body1,
             base="deadbeef", label="", author="validator")
    check("stale base rejected",
          r.get("status") == "err" and r.get("msg") == "stale_base"
          and "current_hash" in r, str(r))
    body2 = json.loads(json.dumps(body1))
    body2["input"]["a"]["x"] = 0.25
    r = call("write_flow_body", lib=L, ctl="probe", cmd="passthru", body=body2,
             base=h, label="nudge a", author="validator")
    check("write with fresh base", r.get("status") == "ok" and r.get("created") is False, str(r))
    r = call("invoke_command", lib=L, ctl="probe", cmd="passthru", args={"a": 41})
    check("the written flow EXECUTES (passthrough returns 41)",
          r.get("status") == "ok" and "41" in json.dumps(r), str(r)[:200])
    r = call("list_control_patches", lib=L, ctl="probe", limit=0)
    flows = [p for p in r.get("patches", []) if p.get("facet") == "flow"]
    check("journal carries the flow entries (facet 'flow' + cmd name)",
          len(flows) == 2 and flows[0].get("cmd") == "passthru", str(flows[:1])[:200])
    r = call("write_flow_body", lib="agent", ctl="code", cmd="read_command",
             body=body1, base="", label="", author="validator")
    check("non-flow command refused, no write", r.get("status") == "err", str(r))

    print("timers/events (P2)")
    # start = 1 hour out so the disposable instance never actually fires it
    r = call("set_timer", lib=L, ctl="probe", name="tick", cmd="passthru",
             start=1, startunit="hours", interval=1, intervalunit="hours",
             repeat=True, author="validator")
    check("set_timer creates", r.get("status") == "ok" and r.get("created") is True, str(r))
    probe_id2 = next(c["id"] for c in
                     get(base, sid, "/app/read", {"lib": L, "id": "controls"})["data"]["list"]
                     if c["name"] == "probe")
    ctl_rec = get(base, sid, "/app/read", {"lib": L, "id": probe_id2})
    tarr = ctl_rec["data"].get("timer", [])
    check("control record links the timer", len(tarr) == 1 and tarr[0].get("name") == "tick", str(tarr))
    comp_id = tarr[0]["id"] if tarr else ""
    comp = get(base, sid, "/app/read", {"lib": L, "id": comp_id})
    check("component record carries the dev-lib shape",
          comp.get("status") == "ok" and comp["data"].get("cmddb") == L
          and comp["data"].get("start") == 1 and comp["data"].get("startunit") == "hours"
          and comp["data"].get("repeat") is True, str(comp.get("data"))[:200])
    r = call("set_timer", lib=L, ctl="probe", name="tick", cmd="passthru",
             start=1, startunit="hours", interval=2, intervalunit="hours",
             repeat=True, author="validator")
    check("replace-only: same name overwrites, keeps id",
          r.get("status") == "ok" and r.get("created") is False
          and r.get("component_id") == comp_id, str(r))
    comp = get(base, sid, "/app/read", {"lib": L, "id": comp_id})
    check("replacement persisted (interval 2)", comp["data"].get("interval") == 2,
          str(comp.get("data"))[:160])
    r = call("set_timer", lib=L, ctl="probe", name="bad", cmd="no_such_cmd",
             start=1, startunit="hours", interval=1, intervalunit="hours",
             repeat=False, author="validator")
    check("unknown command refused", r.get("status") == "err", str(r))
    r = call("set_timer", lib=L, ctl="probe", name="bad", cmd="passthru",
             start=1, startunit="fortnights", interval=1, intervalunit="hours",
             repeat=False, author="validator")
    check("invalid unit refused", r.get("status") == "err", str(r))

    r = call("set_event_handler", lib=L, ctl="probe", name="onlogin",
             bot="security", event="LOGIN", cmd="passthru", author="validator")
    check("set_event_handler creates", r.get("status") == "ok" and r.get("created") is True, str(r))
    r = call("set_event_handler", lib=L, ctl="probe", name="onlogin",
             bot="security", event="LOGOUT", cmd="passthru", author="validator")
    check("handler replace-only", r.get("status") == "ok" and r.get("created") is False, str(r))

    r = call("list_control_patches", lib=L, ctl="probe", limit=0)
    kinds = [p.get("facet") for p in r.get("patches", [])]
    check("journal carries timer+event entries",
          kinds.count("timer") >= 2 and kinds.count("event") >= 2, str(kinds))

    r = call("remove_timer", lib=L, ctl="probe", name="tick", author="validator")
    check("remove_timer", r.get("status") == "ok" and r.get("removed") is True, str(r))
    ctl_rec = get(base, sid, "/app/read", {"lib": L, "id": probe_id2})
    check("timer unlinked", ctl_rec["data"].get("timer", []) == [], str(ctl_rec["data"].get("timer")))
    comp = get(base, sid, "/app/read", {"lib": L, "id": comp_id})
    check("component record deleted", comp.get("status") == "err", str(comp)[:120])
    r = call("remove_timer", lib=L, ctl="probe", name="tick", author="validator")
    check("remove absent timer errs", r.get("status") == "err", str(r))
    r = call("remove_event_handler", lib=L, ctl="probe", name="onlogin", author="validator")
    check("remove_event_handler", r.get("status") == "ok", str(r))
    r = call("remove_event_handler", lib=L, ctl="probe", name="onlogin", author="validator")
    check("remove absent handler errs", r.get("status") == "err", str(r))

    # ── the scene facet pair (scene-facet-design Part VI) ────────────────────
    print("\nscene facet pair:")
    r = call("read_control_scene", lib=L, ctl="probe")
    check("read on facet-absent control: ok + exists false",
          r.get("status") == "ok" and r.get("exists") is False and r.get("hash"), str(r)[:160])
    h0 = r.get("hash", "")

    before = get(base, sid, "/app/read", {"lib": L, "id": probe_id2})["data"]
    before_keys = set(before.keys())
    before_html = before.get("html")

    scene1 = {"v": 1,
              "state": [{"name": "n", "type": "number", "value": 1}],
              "nodes": [{"id": "a", "kind": "box", "pos": {"x": 0, "y": 0.5, "z": 0}}],
              "bindings": [{"target": "a.scale.x", "expr": "n"}]}
    r = call("write_control_scene", lib=L, ctl="probe", scene=scene1,
             base=h0, label="scene v1", author="validator")
    check("write creates the facet (created true, hash-guarded vs the absent hash)",
          r.get("status") == "ok" and r.get("created") is True and r.get("hash"), str(r)[:160])
    h1 = r.get("hash", "")

    r = call("read_control_scene", lib=L, ctl="probe")
    check("read round-trips the object + the write's hash",
          r.get("status") == "ok" and r.get("exists") is True and r.get("hash") == h1
          and r.get("scene", {}).get("nodes", [{}])[0].get("id") == "a", str(r)[:200])

    scene2 = json.loads(json.dumps(scene1))
    scene2["nodes"][0]["pos"]["x"] = 2
    r = call("write_control_scene", lib=L, ctl="probe", scene=scene2,
             base=h1, label="scene v2", author="validator")
    check("guarded update (created false)",
          r.get("status") == "ok" and r.get("created") is False, str(r)[:160])
    h2 = r.get("hash", "")

    r = call("write_control_scene", lib=L, ctl="probe", scene=scene1,
             base=h1, label="stale", author="validator")
    check("stale base refused with current_hash",
          r.get("status") == "err" and r.get("msg") == "stale_base"
          and r.get("current_hash") == h2, str(r)[:160])

    r = call("write_control_scene", lib=L, ctl="probe", scene=scene1,
             base="", label="unguarded", author="validator")
    check("empty base = unguarded write", r.get("status") == "ok", str(r)[:160])

    after = get(base, sid, "/app/read", {"lib": L, "id": probe_id2})["data"]
    check("read-modify-write touches ONLY the scene key (html + every other key intact)",
          after.get("html") == before_html
          and before_keys - set(after.keys()) == set()
          and set(after.keys()) - before_keys <= {"scene"},
          "lost: {} gained: {}".format(before_keys - set(after.keys()),
                                       set(after.keys()) - before_keys))

    r = call("list_control_patches", lib=L, ctl="probe", limit=0)
    # NEWEST FIRST (CONTRACT §6) — scenes[0] is the unguarded write, whose
    # old/new are both full bodies; the oldest entry's old is "" (it created
    # the facet), which is correct, not a defect.
    scenes = [p for p in r.get("patches", []) if p.get("facet") == "scene"]
    check("journal carries the scene entries (old/new JSON)",
          len(scenes) >= 3 and scenes[0].get("old") and scenes[0].get("new"),
          str([p.get("label") for p in scenes]))

    # Revert = write the newest entry's `old` (that entry's old is scene2 —
    # the unguarded write replaced it with scene1).
    old_body = json.loads(scenes[0]["old"]) if scenes and scenes[0].get("old") else {}
    r = call("write_control_scene", lib=L, ctl="probe", scene=old_body,
             base="", label="revert", author="validator")
    rr = call("read_control_scene", lib=L, ctl="probe")
    check("revert-from-journal restores the prior body",
          r.get("status") == "ok"
          and rr.get("scene", {}).get("nodes", [{}])[0].get("pos", {}).get("x") == 2,
          str(rr)[:200])

    r = call("read_control_scene", lib=L, ctl="no_such_control")
    check("unknown control errs", r.get("status") == "err", str(r)[:120])

    # ── deletion (CONTRACT §6.1) ─────────────────────────────────────────────
    # Everything here is verified through the API only (this script may point
    # at a remote disposable, so no disk checks — the in-repo probe covered
    # attachment-file removal). The published-app refusal needs a published
    # app to exist and is exercised by hand, not here. This section ends by
    # deleting the scratch library itself, so re-runs no longer need a fresh
    # --lib name.
    print("deletion (delete_command / delete_control / delete_library)")
    r = call("write_flow_body", lib=L, ctl="probe", cmd="doomed",
             body={"input": {"a": {"mode": "regular", "type": "integer", "x": 0}},
                   "output": {"a": {"mode": "regular", "type": "integer", "x": 0}},
                   "cmds": [], "cons": [{"src": [-1, "a"], "dest": [-2, "a"]}]},
             base="", label="deletion fixture", author="validator")
    check("fixture command created", r.get("status") == "ok", str(r)[:160])
    r = call("delete_command", lib=L, ctl="probe", cmd="doomed", author="validator")
    check("delete_command ok + patch_id",
          r.get("status") == "ok" and r.get("patch_id"), str(r)[:160])
    r = call("read_command", lib=L, ctl="probe", cmd="doomed")
    # read_command returns Object, not FLAT — its payload rides in `data`
    check("deleted command unreadable",
          (r.get("data") or r).get("status") == "err", str(r)[:120])
    r = call("list_control_patches", lib=L, ctl="probe", limit=1)
    p0 = (r.get("patches") or [{}])[0]
    old = json.loads(p0.get("old") or "{}")
    check("deletion journaled (facet command, old = both records)",
          p0.get("facet") == "command" and p0.get("cmd") == "doomed"
          and p0.get("label") == "delete command doomed"
          and "command" in old and "impl" in old, str(p0)[:200])
    r = call("delete_command", lib=L, ctl="probe", cmd="doomed", author="validator")
    check("unknown command refused", r.get("status") == "err", str(r)[:120])

    r = call("add_control", lib=L, ctl="victim")
    r = call("write_flow_body", lib=L, ctl="victim", cmd="tick",
             body={"input": {"a": {"mode": "regular", "type": "integer", "x": 0}},
                   "output": {"a": {"mode": "regular", "type": "integer", "x": 0}},
                   "cmds": [], "cons": [{"src": [-1, "a"], "dest": [-2, "a"]}]},
             base="", label="deletion fixture", author="validator")
    r = call("set_timer", lib=L, ctl="victim", name="t1", cmd="tick",
             start=1, startunit="hours", interval=1, intervalunit="hours",
             repeat=True, author="validator")
    check("victim fixture (command + timer)", r.get("status") == "ok", str(r)[:160])
    r = call("delete_control", lib=L, ctl="victim", author="validator")
    check("delete_control ok, counts commands + components",
          r.get("status") == "ok" and r.get("deleted_commands") == 1
          and r.get("deleted_components") == 1, str(r)[:160])
    vidx = get(base, sid, "/app/read", {"lib": L, "id": "controls"})
    check("control unlinked from the library index",
          all(c.get("name") != "victim" for c in vidx["data"]["list"]), str(vidx)[:160])
    r = call("delete_control", lib=L, ctl="victim", author="validator")
    check("unknown control refused", r.get("status") == "err", str(r)[:120])

    r = call("delete_library", lib=L, author="validator")
    check("library with controls refused (rmdir semantics)",
          r.get("status") == "err" and "control" in r.get("msg", ""), str(r)[:160])
    r = call("delete_library", lib="no_such_lib_xyz", author="validator")
    check("unknown library refused", r.get("status") == "err", str(r)[:120])
    for c in list(vidx["data"]["list"]):
        call("delete_control", lib=L, ctl=c["name"], author="validator")
    r = call("delete_library", lib=L, author="validator")
    check("emptied library deletes", r.get("status") == "ok", str(r)[:160])
    # a deleted library's index answers HTTP 500, not a JSON err envelope
    try:
        r = get(base, sid, "/app/read", {"lib": L, "id": "controls"})
        gone = r.get("status") != "ok"
    except Exception:
        gone = True
    check("library gone (controls index unreadable)", gone)

    print()
    if FAILS:
        print("RESULT: {} FAILED — {}".format(len(FAILS), ", ".join(FAILS)))
        sys.exit(1)
    print("RESULT: all checks passed — promote CONTRACT §6 to [live]")


if __name__ == "__main__":
    main()
