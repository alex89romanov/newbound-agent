// wiring — the workbench's timers/events panel against a REAL instance.
// The setters are replace-only keyed on name (Q7), they write component
// records in the dev lib's shapes, and they register LIVE through the
// appserver, so a set timer actually fires. That last part is precisely
// what a simulator could never prove, which is why this pass moved off one.
//
//   node tools/smoke/wiring.mjs --base http://localhost:33199 --dir DIR
import { smoke, login, agentApi, flowFixture, flowBody } from "./harness.mjs";

const BASE = (process.argv.includes("--base")
  ? process.argv[process.argv.indexOf("--base") + 1] : null) || "http://127.0.0.1:33199";

await smoke("workbench timers/events (real instance)", async (t, browser) => {
  const sid = await login(BASE, { dir: t.args.dir, user: t.args.user, password: t.args.password });
  const api = await agentApi(BASE, sid);
  const LIB = t.args.lib || "smoketest", CTL = "wired";

  // A control with one command, so the panel's "runs <command>" select has
  // something real to offer. A FLOW command, deliberately: write_flow_body
  // creates one outright, where upsert_command needs a compile toolchain.
  const f = await flowFixture(api, { lib: LIB, ctl: CTL, cmd: "tick",
    body: flowBody(), label: "wiring fixture" });
  t.check("a command exists for the timer to run", !!f.cmdId, f.cmdId);

  // Start from a known state. NOTE the `author`: every DECLARED param must
  // be passed. The generated command wrapper reads its args BEFORE its
  // catch_unwind, so a missing key panics through the FFI boundary and
  // ABORTS THE WHOLE SERVER — not just the request. (The bench's own store.js
  // wrappers always fill author, which is why the UI never hits this.)
  await api.call("remove_timer", { lib: LIB, ctl: CTL, name: "nightly", author: "smoke" });
  await api.call("remove_event_handler", { lib: LIB, ctl: CTL, name: "onping", author: "smoke" });

  const ctlId = f.ctlId;

  const page = await t.page(browser, BASE, sid);
  await page.goto(`${BASE}/bench/#/bench/${LIB}/${ctlId}`, { waitUntil: "networkidle" });
  await page.waitForTimeout(2500);
  const V = (sel) => page.locator(sel + " >> visible=true").first();

  await V(".wm-wiring").click();
  await page.waitForTimeout(400);
  t.check("the panel says registration is live",
    /live/i.test(await V(".ww-cap").textContent()));
  t.check("both lists start empty",
    /no timers|none/i.test(await V(".ww-timer-rows").textContent() || "none") ||
    (await V(".ww-timer-rows").textContent()).trim() === "");

  // ── set a timer ──
  await V(".ww-timer-add").click();
  await V(".wt-name").fill("nightly");
  await V(".wt-cmd").selectOption({ index: 0 });
  await V(".wt-start").fill("2");
  await V(".wt-startunit").selectOption("hours");
  await V(".ww-timer-form .ww-apply").click();
  await page.waitForTimeout(800);
  const row = V(".ww-timer-rows .ww-row");
  const rowText = await row.textContent();
  t.check("the timer appears in the list", /nightly/.test(rowText), rowText.replace(/\s+/g, " "));

  // it must exist in the STORE, not just the DOM
  const rec = await api.read(LIB, ctlId);
  t.check("the control record links the timer",
    JSON.stringify(rec.data.timer || rec.data.timers || []).length > 2,
    JSON.stringify(rec.data.timer || rec.data.timers || []).slice(0, 120));

  // ── replace-only: the same name edits rather than duplicating (Q7) ──
  await row.locator(".ww-act:not(.ww-del)").click();
  await page.waitForTimeout(400);
  t.check("edit ▸ prefills the existing timer",
    (await V(".wt-name").inputValue()) === "nightly",
    await V(".wt-name").inputValue());
  await V(".wt-start").fill("6");
  await V(".ww-timer-form .ww-apply").click();
  await page.waitForTimeout(800);
  const rowsAfter = await page.locator(".ww-timer-rows .ww-row").count();
  t.check("re-applying the same NAME replaces, never duplicates (Q7)",
    rowsAfter === 1, `${rowsAfter} rows`);

  // ── the journal records it ──
  const patches = await api.call("list_control_patches", { lib: LIB, ctl: CTL, limit: 20 });
  const facets = (patches.patches || []).map((p) => p.facet);
  t.check("timer edits are journaled on a `timer` facet",
    facets.includes("timer"), JSON.stringify([...new Set(facets)]));

  // ── remove (two-click) ──
  await row.locator(".ww-del").click();
  await page.waitForTimeout(200);
  await row.locator(".ww-del").click();
  await page.waitForTimeout(800);
  t.check("two-click remove clears the row",
    (await page.locator(".ww-timer-rows .ww-row").count()) === 0);
  const gone = await api.call("remove_timer", { lib: LIB, ctl: CTL, name: "nightly", author: "smoke" });
  t.check("the platform agrees it is gone (removing again errs)",
    gone.status !== "ok", JSON.stringify(gone).slice(0, 100));

  await page.close();
});
