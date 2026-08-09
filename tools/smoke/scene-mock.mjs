// scene-mock — the scene stack in mock mode: the editor's build/play modes,
// and the peer-constellation specimen through the standalone sceneplayer
// (collections, links, tap-with-item-locals, keyed diffing).
// Needs no instance — a static file server is enough.
import { smoke, staticServer } from "./harness.mjs";

await smoke("scene editor + peer specimen (mock)", async (t, browser) => {
  const srv = await staticServer(t.args.port ? Number(t.args.port) : 8471);
  try {
    const page = await t.page(browser, srv.url, null);
    await page.goto(srv.url + "/index.html", { waitUntil: "networkidle" });
    await page.waitForTimeout(400);

    // ── the peer constellation specimen ──
    const out = await page.evaluate(async () => {
      const { mountControl } = await import("/assets/loader.js");
      const doc = await (await fetch("/harness/fixtures/scene_peer.json")).json();
      const host = document.createElement("div");
      host.style.cssText = "position:fixed;inset:0;z-index:9999;background:#171a1e";
      document.body.appendChild(host);
      const api = await mountControl("sceneplayer", host, { doc, caption: "peer specimen" });
      await new Promise((r) => setTimeout(r, 800));
      window.__peer = { api, host };
      return {
        canvas: !!host.querySelector("canvas"),
        labels: [...host.querySelectorAll(".ss-label")].map((l) => l.textContent).sort(),
      };
    });
    t.check("the player renders the specimen", out.canvas);
    t.check("an `each` collection mounts one child per item",
      out.labels.length >= 3, JSON.stringify(out.labels));

    // a tap must carry the ITEM through the wire (per-instance locals)
    const cbox = await page.locator("canvas").last().boundingBox();
    await page.mouse.click(cbox.x + cbox.width / 2, cbox.y + cbox.height / 2);
    await page.waitForTimeout(300);
    const focus = await page.evaluate(() => window.__peer.api.stateOf().focus);
    t.check("a tap carries the instance key through the wire", !!focus, JSON.stringify(focus));

    // keyed diffing: dropping an item disposes exactly that instance
    const after = await page.evaluate(async () => {
      const { api, host } = window.__peer;
      const peers = api.stateOf().peers;
      api.setState("peers", peers.slice(0, -1));
      await new Promise((r) => setTimeout(r, 400));
      const labels = [...host.querySelectorAll(".ss-label")].map((l) => l.textContent).sort();
      api.dispose(); host.remove();
      return labels;
    });
    t.check("keyed diffing disposes the departed instance only",
      after.length === out.labels.length - 1, JSON.stringify(after));

    // ── the editor tolerates each/link docs in build mode ──
    const ed = await page.evaluate(async () => {
      const { mountControl } = await import("/assets/loader.js");
      const doc = await (await fetch("/harness/fixtures/scene_peer.json")).json();
      const host = document.createElement("div");
      host.style.cssText = "position:fixed;inset:0;z-index:9999;background:#171a1e";
      document.body.appendChild(host);
      await mountControl("sceneeditor", host, {
        lib: "x", name: "y", record: { scene: doc }, toast: { show() {} } });
      await new Promise((r) => setTimeout(r, 700));
      const rows = [...host.querySelectorAll(".se-panel .se-kind")].map((k) => k.textContent);
      const diag = host.querySelector(".se-diagbtn").textContent;
      host.remove();
      return { rows, diag };
    });
    t.check("the editor tree badges collections", ed.rows.some((r) => /each/.test(r)),
      JSON.stringify(ed.rows));
    t.check("a valid collection doc raises no diagnostics", ed.diag === "✓", ed.diag);
  } finally {
    srv.stop();
  }
});
