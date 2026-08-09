// flow-editor — the 3D flow editor against a REAL instance, end to end.
// Four passes that used to be four scratchpad scripts:
//
//   deep loop  open a flow → edit its signature → ⌘S (journaled, hash-guarded)
//              → journal strip → run ▸ → invoke_command → result inspected
//   untangle   a non-planar tangle → untangle ▸ previews → applies as ONE
//              mutation → ONE undo reverts it → the save unfolds z
//   keyboard   W → arrows → Enter, pointer-free, offering only legal targets
//   journal    entries expand into a structural diff, and revert still works
//
// It builds its OWN fixture library through the API, so nothing here depends
// on ids from some other machine.
//
//   node tools/smoke/flow-editor.mjs --base http://localhost:33199 --dir /path/to/instance
import { smoke, login, agentApi, flowFixture, flowBody, op } from "./harness.mjs";

const BASE = (process.argv.includes("--base")
  ? process.argv[process.argv.indexOf("--base") + 1] : null) || "http://127.0.0.1:33199";

await smoke("flow editor (real instance)", async (t, browser) => {
  const sid = await login(BASE, { dir: t.args.dir, user: t.args.user, password: t.args.password });
  const api = await agentApi(BASE, sid);
  const readBody = async (f) => (await api.call("read_flow_body",
    { lib: f.lib, ctl: f.ctl, cmd: f.cmd })).body;
  const put = (f, body, label) => api.call("write_flow_body",
    { lib: f.lib, ctl: f.ctl, cmd: f.cmd, body, base: "", label, author: "smoke" });

  const open = async (f) => {
    const page = await t.page(browser, BASE, sid);
    await page.goto(`${BASE}/bench/#/flow/${f.lib}/${f.ctlId}/${f.cmdId}`, { waitUntil: "networkidle" });
    await page.waitForTimeout(3000);
    return page;
  };
  // The editor's keybinds are host-scoped: after a popover commit re-renders
  // the button that held focus, focus must go back to the pane before ⌘S.
  // A user clicks the canvas naturally; automation has to do it on purpose.
  const focusPane = async (page) => {
    const b = await page.locator(".nb-floweditor3d canvas").first().boundingBox();
    await page.mouse.click(b.x + b.width * 0.12, b.y + b.height * 0.88);
    await page.waitForTimeout(150);
  };
  const saveNow = async (page) => {
    await focusPane(page);
    await page.keyboard.press("Control+s");
    await page.waitForFunction(() => /saved ·|save failed/.test(
      document.querySelector(".fx-status")?.textContent || ""), { timeout: 10000 });
    await page.waitForTimeout(200);
  };

  // ── 1. the deep loop ─────────────────────────────────────────────────────
  {
    const f = await flowFixture(api, { cmd: "passthru", body: flowBody(), label: "deep-pass reset" });
    const page = await open(f);
    t.check("the 3D editor mounts", await page.locator(".nb-floweditor3d canvas").count() === 1);
    t.check("editable — run ▸ shows on a writable connection",
      await page.locator(".fx-run").isVisible());

    const bars = page.locator(".fx-label.bar");
    let paramsBar = bars.first();
    for (let i = 0; i < await bars.count(); i++) {
      if (/param/i.test(await bars.nth(i).textContent())) paramsBar = bars.nth(i);
    }
    const box = await paramsBar.boundingBox();
    let selected = false;
    for (const dy of [0, 10, -10, 20]) {
      await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2 + dy);
      await page.waitForTimeout(150);
      if ((await page.locator(".fi-kind").first().textContent().catch(() => "")) === "params") { selected = true; break; }
    }
    t.check("the params bar selects", selected);
    await page.keyboard.press("Enter");
    await page.waitForTimeout(200);
    t.check("Enter opens the bar popover", await page.locator(".fx-pop").isVisible());
    const row = page.locator(".fx-pop .fx-pop-row").last();
    await row.locator("input").nth(0).fill("b");
    await row.locator("input").nth(1).fill("integer");
    await row.locator("button", { hasText: "add" }).click();
    await page.waitForTimeout(300);
    t.check("the edit marks the doc dirty", await page.locator(".fx-save").textContent() === "save");

    await saveNow(page);
    t.check("⌘S saves through write_flow_body",
      /saved ·/.test(await page.locator(".fx-status").textContent()));
    t.check("no conflict banner", !(await page.locator(".fx-conflict").isVisible()));
    t.check("the journal strip lists this flow", await page.locator(".fx-journal").isVisible());

    await page.locator(".fx-run").click();
    await page.waitForTimeout(300);
    t.check("run ▸ opens an args picker built from the params bar",
      await page.locator(".fx-picker").isVisible());
    const rows = page.locator(".fx-picker .fx-pop-row");
    t.check("both params are offered", await rows.count() === 2);
    await rows.nth(0).locator("input").fill("5");
    await rows.nth(1).locator("input").fill("7");
    await page.locator(".fx-picker-foot button", { hasText: "run" }).click();
    await page.waitForFunction(() => /ran ·|run failed/.test(
      document.querySelector(".fx-status")?.textContent || ""), { timeout: 20000 });
    const status = await page.locator(".fx-status").textContent();
    t.check("the flow EXECUTES on the platform", !/failed/.test(status), status);
    const inspect = (await page.locator(".fx-inspect").textContent()).replace(/\s+/g, " ");
    t.check("the result lands in the inspector", inspect.includes("5"), inspect.slice(0, 90));
    await page.close();
  }

  // ── 2. untangle ──────────────────────────────────────────────────────────
  {
    // K3,3: no x ordering clears it, one component, so only depth can
    const cmds = [op("s0", -3, 3), op("s1", 0, 3), op("s2", 3, 3),
                  op("t0", -3, 0), op("t1", 0, 0), op("t2", 3, 0)];
    const cons = [{ src: [-1, "a"], dest: [0, "a"] }];
    for (let s = 0; s < 3; s++) for (let d = 3; d < 6; d++) cons.push({ src: [s, "a"], dest: [d, "a"] });
    cons.push({ src: [3, "a"], dest: [-2, "a"] });
    const f = await flowFixture(api, { cmd: "tangled", body: flowBody({ cmds, cons }), label: "untangle fixture" });
    const page = await open(f);
    t.check("auto-layout controls show when writable",
      await page.locator(".fx-layout-btns").isVisible());

    await page.locator(".fx-layout-btns button", { hasText: "untangle" }).click();
    await page.waitForTimeout(600);
    const bar = page.locator(".fx-layout-bar").first();
    t.check("untangle ▸ previews with an apply/cancel bar", await bar.isVisible());
    t.check("a proposal mutates nothing until applied",
      await page.locator(".fx-save").textContent() === "saved");
    await bar.locator(".fx-layout-apply").click();
    await page.waitForTimeout(600);
    t.check("applying marks it dirty", await page.locator(".fx-save").textContent() === "save");

    // ONE undo must take the WHOLE layout back — proven through the store,
    // not the dirty flag (undo() deliberately marks dirty; it IS a change)
    await focusPane(page);
    await page.keyboard.press("Control+z");
    await page.waitForTimeout(300);
    await saveNow(page);
    const undone = (await readBody(f)).cmds.map((o) => o.pos.z);
    t.check("ONE undo reverts the whole layout",
      Math.max(...undone) - Math.min(...undone) === 0, undone.map((z) => z.toFixed(2)).join(" "));

    await focusPane(page);
    await page.keyboard.press("Control+y");
    await page.waitForTimeout(300);
    await saveNow(page);
    const body = await readBody(f);
    const zs = body.cmds.map((o) => o.pos.z);
    const spread = Math.max(...zs) - Math.min(...zs);
    t.check("the saved layout unfolds the tangle into DEPTH", spread > 0.5,
      `z spread ${spread.toFixed(2)}`);
    const ys = body.cmds.map((o) => o.pos.y);
    t.check("y stays layered (down = later is never optimized away)",
      new Set(ys.map((y) => y.toFixed(2))).size === 2, ys.map((y) => y.toFixed(1)).join(" "));
    await page.close();
  }

  // ── 3. keyboard wiring ───────────────────────────────────────────────────
  {
    // three ops so W has MORE THAN ONE legal target; the return bar is
    // already fed (I-1 must exclude it) and self-wiring is a cycle (I-2)
    const f = await flowFixture(api, { cmd: "kbwire", label: "kbwire fixture",
      body: flowBody({
        cmds: [op("first", -2, 1.6), op("second", 2, -1.6), op("third", 4, -1.6)],
        cons: [{ src: [-1, "a"], dest: [0, "a"] }, { src: [1, "a"], dest: [-2, "a"] }],
      }) });
    const page = await open(f);
    t.check("the W keybind is in the legend",
      /W/.test(await page.locator(".fx-edit-keys").last().textContent()));
    await focusPane(page);
    const status = () => page.locator(".fx-status").textContent();

    await page.keyboard.press("Tab");
    await page.waitForTimeout(300);
    await page.keyboard.press("w");
    await page.waitForTimeout(300);
    const s1 = await status();
    t.check("W enters wire mode", /wire →/.test(s1), s1);
    const n = (s1.match(/· (\d+)\/(\d+) ·/) || [])[2];
    t.check("ONLY legal targets are offered (I-1 and I-2 filter the list)",
      n === "2", `${n} candidates`);
    const t1 = (s1.match(/wire → (\S+)/) || [])[1];
    await page.keyboard.press("ArrowRight"); await page.waitForTimeout(250);
    const t2 = ((await status()).match(/wire → (\S+)/) || [])[1];
    await page.keyboard.press("ArrowRight"); await page.waitForTimeout(250);
    const t3 = ((await status()).match(/wire → (\S+)/) || [])[1];
    t.check("arrows walk the candidates and wrap", t1 !== t2 && t3 === t1, `${t1} → ${t2} → ${t3}`);
    t.check("neither candidate is illegal", ![t1, t2].some((x) => /^return\.|^first\./.test(x || "")));

    await page.keyboard.press("Escape");
    await page.waitForTimeout(250);
    t.check("Esc cancels without mutating",
      await page.locator(".fx-save").textContent() === "saved");

    await page.keyboard.press("w");
    await page.waitForTimeout(250);
    let picked = await status();
    for (let i = 0; i < 8 && !/second\.a/.test(picked); i++) {
      await page.keyboard.press("ArrowRight");
      await page.waitForTimeout(200);
      picked = await status();
    }
    await page.keyboard.press("Enter");
    await page.waitForTimeout(400);
    t.check("Enter commits the wire", /wired →/.test(await status()));
    await saveNow(page);
    const wired = (await readBody(f)).cons.some((c) => c.src[0] === 0 && c.dest[0] === 1);
    t.check("the keyboard-authored wire is in the SAVED body", wired);
    await page.close();
  }

  // ── 4. the journal diff view ─────────────────────────────────────────────
  {
    const V1 = flowBody({ cmds: [op("first", -2, 1.6), op("second", 2, -1.6)],
      cons: [{ src: [-1, "a"], dest: [0, "a"] }, { src: [1, "a"], dest: [-2, "a"] }] });
    const clone = (o) => JSON.parse(JSON.stringify(o));
    const V2 = clone(V1); V2.cons.push({ src: [0, "a"], dest: [1, "a"] });    // a wire
    const V3 = clone(V2); V3.cmds[0].pos.z = 0.9;                             // a move
    const V4 = clone(V3); V4.input.b = { mode: "regular", type: "string", x: 0.5 };  // signature

    const f = await flowFixture(api, { cmd: "journaled", body: V1, label: "jdiff base" });
    for (const [body, label] of [[V2, "jdiff wire"], [V3, "jdiff move"], [V4, "jdiff signature"]]) {
      const r = await put(f, body, label);
      if (r.status !== "ok") throw new Error("journal fixture write failed: " + JSON.stringify(r).slice(0, 160));
    }
    const page = await open(f);
    // direct children only — the diff list nests its own <li>s inside an entry
    const rows = page.locator(".fx-journal-list > li");
    t.check("diffs start COLLAPSED (the strip stays a strip)",
      (await page.locator(".jl-diff:visible").count()) === 0);
    const openRow = async (i) => {
      await rows.nth(i).locator(".jl-label").click();
      await page.waitForTimeout(250);
      return (await rows.nth(i).locator(".jl-diff").textContent()).replace(/\s+/g, " ").trim();
    };
    const dSig = await openRow(0);
    t.check("a signature edit reads as one", /params \+ b:string/.test(dSig), dSig);
    const dMove = await openRow(1);
    t.check("a move reads with its new coordinates",
      /moved first to \(-2\.00, 1\.60, 0\.90\)/.test(dMove), dMove);
    const dWire = await openRow(2);
    t.check("a wire reads with NAMED endpoints, not indices",
      /wired first\.a to second\.a/.test(dWire), dWire);
    t.check("the entry carries a +/−/~ header", /\+\d+ −\d+ ~\d+/.test(dWire));
    t.check("accordion — exactly one diff open at a time",
      (await page.locator(".jl-diff:visible").count()) === 1);
    await rows.nth(2).locator(".jl-label").click();
    await page.waitForTimeout(250);
    t.check("clicking again collapses", (await page.locator(".jl-diff:visible").count()) === 0);

    const before = await readBody(f);
    await rows.nth(0).locator(".jl-revert").click();
    await page.waitForTimeout(1200);
    const after = await readBody(f);
    t.check("revert still works alongside the disclosure",
      "b" in (before.input ?? {}) && !("b" in (after.input ?? {})));
    await page.close();
  }
});
