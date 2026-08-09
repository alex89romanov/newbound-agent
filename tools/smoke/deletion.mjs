// deletion — the CONTRACT §6.1 buttons, driven through the real UI against a
// real instance. Three surfaces, one walk:
//
//   command  workbench commands pane: delete (two-click) → row gone, journaled
//   control  shelf fan: delete under the card → card gone, fan re-renders
//   library  fan header: refused while controls remain, then an emptied
//            library deletes and its stack leaves the shelf
//
// Fixtures are built through the API before the page opens (the shelf lists
// libraries at mount). Every deletion is verified API-side too — the UI
// disappearing is not the same thing as the store changing.
//
//   node tools/smoke/deletion.mjs --base http://localhost:33199 --dir /path/to/instance
import { smoke, login, agentApi, flowFixture, flowBody } from "./harness.mjs";

const BASE = (process.argv.includes("--base")
  ? process.argv[process.argv.indexOf("--base") + 1] : null) || "http://127.0.0.1:33199";

await smoke("deletion UI (real instance)", async (t, browser) => {
  const sid = await login(BASE, { dir: t.args.dir, user: t.args.user, password: t.args.password });
  const api = await agentApi(BASE, sid);

  // fixtures: smoketest.probe/passthru (command + control walk), a second
  // control so the library refusal has something left to refuse over, and a
  // control-free library for the clean library deletion
  const f = await flowFixture(api, { cmd: "passthru", body: flowBody(), label: "deletion fixture" });
  await api.call("add_control", { lib: f.lib, ctl: "bystander" });
  await api.call("add_library", { lib: "doomui" });

  const toast = async (page) => (await page.locator(".nb-toast").textContent()).trim();
  const twoClick = async (page, loc) => {
    await loc.click();
    await page.waitForTimeout(200);
    await loc.click();
    await page.waitForTimeout(800);
  };

  const page = await t.page(browser, BASE, sid);

  // ── command: the workbench row ───────────────────────────────────────────
  await page.goto(`${BASE}/bench/#/bench/${f.lib}/${f.ctlId}`, { waitUntil: "networkidle" });
  await page.waitForTimeout(2500);
  const row = page.locator(".wb-cmd-row", { hasText: "passthru" });
  t.check("the command row is there", await row.count() === 1);
  const cdel = row.locator(".wb-cmd-del");
  t.check("delete shows on a writable connection", await cdel.isVisible());
  await cdel.click();
  await page.waitForTimeout(200);
  t.check("first click arms, nothing deleted",
    await cdel.textContent() === "really delete?"
    && (await api.call("read_command", { lib: f.lib, ctl: f.ctl, cmd: f.cmd })).data?.status !== "err");
  await cdel.click();
  await page.waitForTimeout(800);
  t.check("second click deletes the row", await row.count() === 0);
  t.check("the toast names the patch entry", /delete_command → passthru.*journaled p\d+/.test(await toast(page)),
    await toast(page));
  const rc = await api.call("read_command", { lib: f.lib, ctl: f.ctl, cmd: f.cmd });
  t.check("the store agrees (read_command errs)", (rc.data ?? rc).status === "err");
  const j = await api.call("list_control_patches", { lib: f.lib, ctl: f.ctl, limit: 1 });
  t.check("journaled with both records in old",
    j.patches?.[0]?.facet === "command" && j.patches?.[0]?.cmd === "passthru"
    && JSON.parse(j.patches[0].old || "{}").impl !== undefined);

  // ── control: the shelf fan ───────────────────────────────────────────────
  // other smokes may have left their own fixtures in this library (the
  // battery shares smoketest), so every count here is RELATIVE
  await page.goto(`${BASE}/bench/#/shelf/${f.lib}`, { waitUntil: "networkidle" });
  await page.waitForTimeout(2000);
  const n0 = await page.locator(".sh-slot").count();
  t.check("the fan shows the fixture cards", n0 >= 2
    && await page.locator(".sh-slot", { hasText: "probe" }).count() === 1);
  const probeSlot = page.locator(".sh-slot", { hasText: "probe" });
  const sdel = probeSlot.locator(".sh-ctl-del");
  t.check("each card carries its delete", await sdel.isVisible());
  await twoClick(page, sdel);
  await page.waitForTimeout(1200);
  t.check("the card leaves the fan", await page.locator(".sh-slot").count() === n0 - 1
    && await page.locator(".sh-slot", { hasText: "probe" }).count() === 0);
  t.check("the stack count updated in place",
    new RegExp(`${n0 - 1} controls`).test(await page.locator(".sh-stack.on .s-meta").textContent()));
  const remaining = await api.read(f.lib, "controls");
  t.check("the store agrees (probe gone, the bystander stays)",
    remaining.data.list.every((c) => c.name !== "probe")
    && remaining.data.list.some((c) => c.name === "bystander"));

  // ── library: refusal first, then the real one ────────────────────────────
  const ldel = page.locator(".fh-del-btn");
  t.check("the header carries delete library", await ldel.isVisible());
  await twoClick(page, ldel);
  t.check("a library with controls REFUSES (rmdir semantics, relayed)",
    /delete failed: .*control/.test(await toast(page)), await toast(page));
  t.check("the refused library is still on the shelf",
    await page.locator(".sh-stack", { hasText: f.lib }).count() === 1);

  await page.goto(`${BASE}/bench/#/shelf/doomui`, { waitUntil: "networkidle" });
  await page.waitForTimeout(1500);
  await twoClick(page, page.locator(".fh-del-btn"));
  t.check("an empty library deletes", /delete_library → doomui removed/.test(await toast(page)),
    await toast(page));
  t.check("its stack leaves the shelf",
    await page.locator(".sh-stack", { hasText: "doomui" }).count() === 0);
  t.check("its fan closed", await page.locator(".sh-fan").isHidden());

  // cleanup through the same API: EVERY remaining control goes (this smoke
  // runs last in the battery, so this clears the other smokes' smoketest
  // fixtures too and leaves the instance clean for the next full run)
  const left = await api.read(f.lib, "controls");
  for (const c of left.data?.list ?? []) {
    await api.call("delete_control", { lib: f.lib, ctl: c.name, author: "smoke" });
  }
  await api.call("delete_library", { lib: f.lib, author: "smoke" });
  await page.close();
});
