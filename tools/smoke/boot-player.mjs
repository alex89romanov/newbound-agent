// boot-player — the published bench's BOOT CHAIN and the stock-mountable
// `player` control, against a REAL instance. These were sim-backed before,
// which is exactly why a gap slipped through: the `player` control had never
// been committed to the store, and the peer app depends on it, so a deploy
// from a clean checkout was broken while every simulated test passed.
//
// Three passes:
//   boot     /<lib>/ → the stock-mount stub → MODMAP → blob module graph →
//            frame → shelf, all from control FACETS (no repo files served)
//   route    #/player/<lib>/<ctl> renders a scene and takes setState
//   stock    installControl(el, 'bench', 'player', …) inside ANOTHER app's
//            page — the peer app's own path, the one that actually broke
//
//   node tools/smoke/boot-player.mjs --base http://localhost:33199 --dir DIR
import { smoke, login, agentApi } from "./harness.mjs";

const BASE = (process.argv.includes("--base")
  ? process.argv[process.argv.indexOf("--base") + 1] : null) || "http://127.0.0.1:33199";

// A scene with one tappable node and a wire, so a tap is observable through
// the player's own api rather than by looking at pixels.
const SCENE = {
  v: 1,
  state: [{ name: "lit", type: "boolean", value: false },
          { name: "label", type: "string", value: "smoke" }],
  nodes: [
    { id: "cube", kind: "box", params: { size: { x: 1, y: 1, z: 1 } },
      material: { token: "accent" }, affordances: { tap: true } },
    { id: "tag", kind: "text", pos: { x: 0, y: 1.2, z: 0 }, text: "smoke" },
  ],
  bindings: [{ target: "tag.text", expr: "label" },
             { target: "cube.visible", expr: "!lit" }],
  wires: [{ on: "cube.tap", do: [{ set: "lit", to: "!lit" }] }],
  env: { lights: "default", grid: false },
};

await smoke("bench boot + stock player (real instance)", async (t, browser) => {
  const sid = await login(BASE, { dir: t.args.dir, user: t.args.user, password: t.args.password });
  const api = await agentApi(BASE, sid);
  const LIB = t.args.lib || "smoketest", CTL = "scened";

  await api.call("add_library", { lib: LIB });
  await api.call("add_control", { lib: LIB, ctl: CTL });
  const cur = await api.call("read_control_scene", { lib: LIB, ctl: CTL });
  const w = await api.call("write_control_scene", {
    lib: LIB, ctl: CTL, scene: SCENE, base: cur.hash || "",
    label: "smoke scene", author: "smoke" });
  t.check("a scene facet can be authored through the API", w.status === "ok",
    JSON.stringify(w).slice(0, 120));

  // ── 1. the boot chain ────────────────────────────────────────────────────
  {
    const page = await t.page(browser, BASE, sid);
    await page.goto(`${BASE}/bench/`, { waitUntil: "networkidle" });
    await page.waitForTimeout(2500);
    t.check("the published app serves a page", await page.title() !== "");
    t.check("the frame mounted from control facets",
      await page.locator(".nb-frame").count() > 0);
    t.check("the shelf rendered the store's libraries",
      await page.locator(".nb-shelf, .sh-lib, .sh-card").count() > 0);
    // Where did the code come from? Blob module imports do NOT appear in the
    // resource timeline, so "are there blobs" is unobservable. What IS
    // observable is that control js arrives through the platform (jsapi /
    // app-read) and that NO repo path is fetched — which is the regression
    // that would matter: a bench serving files instead of facets.
    const src = await page.evaluate(() =>
      performance.getEntriesByType("resource").map((r) => r.name));
    t.check("control code is served by the platform, not from repo files",
      src.some((n) => /\/app\/(jsapi|read)/.test(n)) &&
      !src.some((n) => /\/(controls|assets|vendor)\//.test(n) && !/\/app\/asset\//.test(n)),
      `${src.filter((n) => /\/app\/read/.test(n)).length} facet reads, ` +
      `${src.filter((n) => /\/(controls|assets)\//.test(n)).length} repo-path fetches`);
    await page.close();
  }

  // ── 2. the #/player route ────────────────────────────────────────────────
  {
    const page = await t.page(browser, BASE, sid);
    await page.goto(`${BASE}/bench/#/player/${LIB}/${CTL}`, { waitUntil: "networkidle" });
    await page.waitForTimeout(2500);
    t.check("the player route renders the scene",
      await page.locator(".sp-canvas canvas, canvas").count() > 0);
    const labels = await page.locator(".ss-label").allTextContents();
    t.check("a text node renders as a DOM label", labels.includes("smoke"),
      JSON.stringify(labels));

    // the bridge the peer app used before the stock control existed
    const echoed = await page.evaluate(async () => {
      window.__msgs = [];
      window.addEventListener("message", (e) => { if (e.data?.sceneState) window.__msgs.push(e.data.sceneState); });
      window.postMessage({ sceneSet: { field: "label", value: "changed" } }, "*");
      await new Promise((r) => setTimeout(r, 500));
      return [...document.querySelectorAll(".ss-label")].map((l) => l.textContent);
    });
    t.check("setState over the postMessage bridge re-renders the binding",
      echoed.includes("changed"), JSON.stringify(echoed));
    await page.close();
  }

  // ── 3. the stock mount, inside ANOTHER app's page ────────────────────────
  // This is the path peer.peer uses: a stock-dialect app calls installControl
  // for bench's `player`, which bootstraps the bench module graph itself.
  {
    const page = await t.page(browser, BASE, sid);
    await page.goto(`${BASE}/peer/`, { waitUntil: "networkidle" });
    await page.waitForTimeout(2500);
    t.check("a peer app page loads with the stock api", await page.evaluate(
      () => typeof window.installControl === "function"));

    const res = await page.evaluate(async ([lib, ctl]) => {
      const host = document.createElement("div");
      host.style.cssText = "position:fixed;inset:0;width:900px;height:600px;z-index:99999";
      document.body.appendChild(host);
      const api = await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("installControl never called back")), 25000);
        window.installControl(host, "bench", "player", (a) => { clearTimeout(timer); resolve(a); },
          { lib, ctl });
      });
      await new Promise((r) => api.waitReady(r));
      const before = api.stateOf().label;
      api.setState("label", "via the stock mount");
      await new Promise((r) => setTimeout(r, 600));
      const labels = [...host.querySelectorAll(".ss-label")].map((l) => l.textContent);
      const canvas = !!host.querySelector("canvas");
      api.dispose(); host.remove();
      return { before, labels, canvas };
    }, [LIB, CTL]);

    t.check("installControl mounts bench's player inside another app", res.canvas);
    t.check("the mounted player exposes the scene's state", res.before === "smoke",
      JSON.stringify(res.before));
    t.check("setState through the direct api re-renders",
      res.labels.includes("via the stock mount"), JSON.stringify(res.labels));
    await page.close();
  }
});
