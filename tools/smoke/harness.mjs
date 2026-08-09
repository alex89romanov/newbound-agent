// harness.mjs — shared plumbing for the browser smokes.
//
// These used to live in a session scratchpad and died with it, so every
// session paid to rewrite them. Moving them here meant fixing the three
// things that made them un-portable:
//
//   · hardcoded ports and absolute playwright/chromium paths → resolved
//     from flags or the environment, with a readable error when missing;
//   · a hardcoded session id → each run logs in for itself (sessions expire
//     in 15 minutes and a disposable instance restarts often);
//   · hardcoded record IDS — the killer. Command and control ids are
//     generated PER INSTANCE, so the scratchpad copies only ever worked on
//     the machine that produced them. Everything here resolves by NAME.
//
// Usage from a smoke:
//   import { smoke } from "./harness.mjs";
//   await smoke("my check", async (t) => { ... });
//
// Flags (all optional): --base http://host:port   running instance
//                       --dir  /path/to/instance  to read admin.properties
//                       --user / --password       instead of --dir
//                       --chromium /path/to/chrome
//                       --keep                    leave the browser open
import { readFileSync, existsSync } from "node:fs";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

export const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

export function args() {
  const a = { _: [] };
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i++) {
    if (argv[i].startsWith("--")) {
      const k = argv[i].slice(2);
      const v = argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[++i] : true;
      a[k] = v;
    } else a._.push(argv[i]);
  }
  return a;
}

/** Playwright lives wherever the host put it; try the package first, then
    common locations, then an explicit --playwright path. */
export async function playwright(opts = {}) {
  const tries = [
    opts.playwright || process.env.PLAYWRIGHT_MODULE,
    "playwright",
    "/opt/node22/lib/node_modules/playwright/index.mjs",
    "/usr/lib/node_modules/playwright/index.mjs",
  ].filter(Boolean);
  for (const t of tries) {
    try { return await import(t); } catch { /* next */ }
  }
  throw new Error("playwright not found — install it, or pass --playwright " +
    "/path/to/playwright/index.mjs (tried: " + tries.join(", ") + ")");
}

/** A chromium that can do WebGL headlessly. The sandbox ships one; elsewhere
    playwright's own download is used by leaving executablePath unset. */
export function chromiumOpts(opts = {}) {
  const explicit = opts.chromium || process.env.SMOKE_CHROMIUM;
  const known = "/opt/pw-browsers/chromium-1194/chrome-linux/chrome";
  const exe = explicit || (existsSync(known) ? known : null);
  return {
    ...(exe ? { executablePath: exe } : {}),
    args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
  };
}

// ── talking to an instance ──────────────────────────────────────────────────

export function adminPassword(dir) {
  const p = resolve(dir, "users/admin.properties");
  if (!existsSync(p)) {
    throw new Error(`no ${p} — point --dir at an instance directory (the file ` +
      `is generated on that instance's first boot), or pass --user/--password`);
  }
  return readFileSync(p, "utf8").match(/^password=(.*)$/m)[1].trim();
}

/** Log in and return a session id. Each run gets its own. */
export async function login(base, { dir, user = "admin", password } = {}) {
  const pass = password || adminPassword(dir);
  const sid = "smoke" + process.pid + Math.floor(Date.now() % 100000);
  const r = await fetch(`${base}/app/login?user=${encodeURIComponent(user)}` +
    `&pass=${encodeURIComponent(pass)}`, { headers: { Cookie: `sessionid=${sid}` } })
    .then((x) => x.text());
  if (!/now logged in/.test(r)) throw new Error("login failed: " + r.slice(0, 160));
  return sid;
}

/** Resolve agent.code's command ids BY NAME — ids are per-instance, so this
    is the difference between a portable smoke and one that only ran here. */
export async function agentApi(base, sid) {
  const read = async (lib, id) => (await fetch(
    `${base}/app/read?lib=${lib}&id=${id}`,
    { headers: { Cookie: `sessionid=${sid}` } })).json();
  const ctls = await read("agent", "controls");
  if (ctls.status !== "ok") throw new Error("cannot read agent controls: " + JSON.stringify(ctls).slice(0, 160));
  const codeId = ctls.data.list.find((c) => c.name === "code")?.id;
  if (!codeId) throw new Error("no agent.code control on this instance");
  const code = await read("agent", codeId);
  const ids = Object.fromEntries(code.data.cmd.map((c) => [c.name, c.id]));
  const call = async (name, a = {}) => {
    if (!ids[name]) throw new Error(`agent.code.${name} missing on this instance`);
    const r = await fetch(`${base}/app/exec?lib=agent&id=${ids[name]}&args=` +
      encodeURIComponent(JSON.stringify(a)), { headers: { Cookie: `sessionid=${sid}` } });
    const text = await r.text();
    // a command that panics server-side answers with an HTML 500, not JSON —
    // surface that as an error value instead of exploding the whole run
    try { return JSON.parse(text); }
    catch { return { status: "err", http: r.status, msg: text.slice(0, 200) }; }
  };
  return { ids, call, read };
}

/** Create (or reuse) a library + control + FLOW command for the editor
    smokes, and return the ids the #/flow route needs. write_flow_body is the
    only API path that creates a flow command, so the body doubles as the
    fixture. Everything is addressed by name; ids come back from the store. */
export async function flowFixture(api, { lib = "smoketest", ctl = "probe",
                                         cmd = "passthru", body, label = "smoke fixture" }) {
  await api.call("add_library", { lib });
  await api.call("add_control", { lib, ctl });
  const w = await api.call("write_flow_body", { lib, ctl, cmd, body, base: "", label, author: "smoke" });
  if (w.status !== "ok") throw new Error("fixture write failed: " + JSON.stringify(w).slice(0, 200));
  const idx = await api.read(lib, "controls");
  const ctlId = idx.data.list.find((c) => c.name === ctl)?.id;
  if (!ctlId) throw new Error(`fixture control ${lib}.${ctl} not found after creation`);
  const rec = await api.read(lib, ctlId);
  const cmdId = (rec.data.cmd || []).find((c) => c.name === cmd)?.id;
  if (!cmdId) throw new Error(`fixture command ${cmd} not found after creation`);
  return { lib, ctl, cmd, ctlId, cmdId, hash: w.hash };
}

/** A minimal valid flow body: params a → return a, plus whatever ops given. */
export function flowBody({ cmds = [], cons = null } = {}) {
  return {
    input: { a: { mode: "regular", type: "integer", x: 0.25 } },
    output: { a: { mode: "regular", type: "integer", x: 0 } },
    cmds,
    cons: cons ?? [{ src: [-1, "a"], dest: [-2, "a"] }],
  };
}

export const op = (name, x, y, extra = {}) => ({
  name, type: "primitive", width: 1.5, pos: { x, y, z: 0 },
  in: { a: { mode: "regular", type: "integer", x: 0 } },
  out: { a: { mode: "regular", type: "integer", x: 0 } }, ...extra,
});

// ── a static file server, for the mock-mode and pure-stage smokes ───────────

export async function staticServer(port = 0) {
  const py = spawn("python3", ["-m", "http.server", String(port || 8471),
                               "--directory", REPO],
                   { stdio: "ignore", detached: true });
  const url = `http://127.0.0.1:${port || 8471}`;
  for (let i = 0; i < 40; i++) {
    try {
      const r = await fetch(url + "/index.html");
      if (r.ok) return { url, stop: () => { try { process.kill(-py.pid); } catch { py.kill(); } } };
    } catch { /* not up yet */ }
    await new Promise((r) => setTimeout(r, 150));
  }
  throw new Error("static server did not come up on " + url);
}

// ── the smoke wrapper: checks, error capture, exit code ─────────────────────

export async function smoke(title, body) {
  const a = args();
  const pw = await playwright(a);
  const errors = [];
  const results = [];
  const t = {
    args: a,
    errors,
    check(name, cond, extra = "") {
      results.push({ name, ok: !!cond, extra });
      console.log(`${cond ? "PASS" : "FAIL"}  ${name}${extra ? "  — " + extra : ""}`);
      if (!cond) errors.push(name);
    },
    note(msg) { console.log("      " + msg); },
    /** A page wired the way the bench expects: session cookie, saves-ON
        config, and console/page errors collected as failures. */
    async page(browser, base, sid, { writable = true, viewport, live } = {}) {
      const ctx = await browser.newContext({ viewport: viewport || { width: 1440, height: 900 } });
      if (sid) await ctx.addCookies([{ name: "sessionid", value: sid, url: base }]);
      const page = await ctx.newPage();
      page.on("pageerror", (e) => { errors.push(`pageerror: ${e.message}`); console.log("  !! " + e.message); });
      // The browser asks for /favicon.ico unprompted and the repo has none.
      // Answer it here rather than filtering the resulting console error:
      // that message carries no url, so text-matching it would also swallow
      // real 404s — which are exactly what these smokes exist to catch.
      await page.route("**/favicon.ico", (r) => r.fulfill({ status: 204, body: "" }));
      page.on("console", (m) => {
        if (m.type() !== "error") return;
        const text = m.text();
        const url = m.location()?.url || "";
        errors.push(`console: ${text}${url ? ` (${url})` : ""}`);
        console.log("  !! " + text + (url ? ` (${url})` : ""));
      });
      // Only seed a LIVE connection when one is wanted (default: whenever a
      // session was supplied). A pure-stage smoke loads index.html off a
      // static file server and imports a module directly — seeding live mode
      // there makes the bench boot and 404 on /app/libs.
      if (live ?? Boolean(sid)) {
        await page.addInitScript(([b, w]) => localStorage.setItem("bench.connection",
          JSON.stringify({ mode: "live", baseUrl: b, sessionid: "", writable: w })), [base, writable]);
      }
      return page;
    },
  };
  console.log(`\n── ${title} ──`);
  const browser = await pw.chromium.launch(chromiumOpts(a));
  let thrown = null;
  try {
    await body(t, browser, pw);
  } catch (e) {
    thrown = e;
    errors.push(`threw: ${e.message}`);
    console.log("FAIL  " + e.message);
  } finally {
    if (!a.keep) await browser.close();
  }
  const passed = results.filter((r) => r.ok).length;
  console.log(errors.length
    ? `\n${title}: ${passed}/${results.length} checks, ${errors.length} problem(s)`
    : `\n${title}: ${passed}/${results.length} checks — NO ERRORS`);
  if (thrown && process.env.SMOKE_TRACE) console.error(thrown);
  process.exitCode = errors.length ? 1 : 0;
  return !errors.length;
}
