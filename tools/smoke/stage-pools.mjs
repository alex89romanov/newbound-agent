// stage-pools — the shared nb_three batching core (vendor/nb_three/pools.js)
// driven through the flow stage's public API. Two claims:
//
//   1. the BUDGET (flow3d-design §7.2): many objects, few draw calls, and
//      the framed 500-op case — the overview a user actually asks for —
//      does not cost two calls per op;
//   2. pooling is INVISIBLE: pick / move / resize / emphasis / hide /
//      remove behave exactly as they did with individual meshes.
//
// Needs no instance: mock mode over a static file server.
import { smoke, staticServer } from "./harness.mjs";

await smoke("stage pools + op-body instancing", async (t, browser) => {
  const srv = await staticServer(t.args.port ? Number(t.args.port) : 8471);
  try {
    const page = await t.page(browser, srv.url, null, { viewport: { width: 1000, height: 700 } });
    await page.goto(srv.url + "/index.html", { waitUntil: "networkidle" });

    // ── sockets + wires: the pools that already existed ──
    const big = await page.evaluate(async () => {
      const { mount } = await import("/vendor/nb_three/stage.js");
      const host = document.createElement("div");
      host.style.cssText = "position:fixed;inset:0;width:1200px;height:800px";
      document.body.appendChild(host);
      const st = mount(host, {});
      window.__st = st; window.__host = host;
      const specs = [];
      for (let i = 0; i < 500; i++) specs.push({ id: `s${i}`, kind: "socket",
        position: { x: (i % 25) * 0.4 - 5, y: -Math.floor(i / 25) * 0.6, z: 0 },
        params: { r: 0.06 }, material: "accent", pickable: true });
      for (let i = 0; i < 700; i++) {
        const a = specs[i % 500].position, b = specs[(i * 7 + 3) % 500].position;
        specs.push({ id: `w${i}`, kind: "tube", position: { x: 0, y: 0, z: 0 },
          params: { from: a, to: b, r: 0.035 }, pickable: true });
      }
      st.patch(specs);
      return st.stats();
    });
    t.check("500 sockets + 700 wires pool into their two meshes",
      big.pooledSockets === 500 && big.pooledWires === 700,
      `sockets ${big.pooledSockets}, wires ${big.pooledWires}`);
    t.check("1200 objects draw in under 20 calls", big.calls < 20, `${big.calls} calls`);

    const after = await page.evaluate(() => {
      const st = window.__st;
      st.patch([{ id: "s3", position: { x: 2, y: 2, z: 0 } }]);
      st.patch([{ id: "s4", emphasis: "select" }]);
      st.patch([{ id: "s5", visible: false }]);
      st.remove("s6"); st.remove("w9"); st.remove("s499");   // middle AND last slots
      return st.stats();
    });
    t.check("mutations keep the pools consistent",
      after.pooledSockets === 498 && after.pooledWires === 699,
      `sockets ${after.pooledSockets}, wires ${after.pooledWires}`);
    t.check("mutations do not grow the draw calls", after.calls <= big.calls,
      `${big.calls} → ${after.calls}`);

    // ── op bodies: the framed budget, which is what P4 fixed ──
    const framed = await page.evaluate(() => {
      const st = window.__st;
      const out = {};
      for (const N of [100, 300, 500]) {
        st.clear();
        const specs = [];
        for (let i = 0; i < N; i++) specs.push({ id: `o${i}`, kind: "box",
          position: { x: (i % 25) * 2 - 25, y: -Math.floor(i / 25) * 1.6, z: 0 },
          params: { w: 1.5, h: 0.6, d: 0.6 }, material: "paper", pickable: true });
        st.patch(specs);
        // frame the WHOLE case: nothing culled — the measurement that matters
        st.setPose({ position: { x: 0, y: -16, z: 80 }, target: { x: 0, y: -16, z: 0 } });
        out[N] = st.stats().calls;
      }
      return out;
    });
    t.note(`framed op bodies: 100→${framed[100]}, 300→${framed[300]}, 500→${framed[500]} calls`);
    t.check("framed op bodies do NOT cost two calls each (was 200/600/1000)",
      framed[500] < 10 && framed[300] < 10 && framed[100] < 10,
      `500 ops → ${framed[500]} calls`);

    // ── pooling stays invisible: each subject at the origin, head-on ──
    const inv = await page.evaluate(() => {
      const st = window.__st;
      const box = st.canvas.getBoundingClientRect();
      const CX = box.left + box.width / 2, CY = box.top + box.height / 2;
      const at = () => st.pick(CX, CY)?.id ?? null;
      const body = (id, pos) => ({ id, kind: "box", position: pos,
        params: { w: 1.5, h: 0.6, d: 0.6 }, material: "paper", pickable: true });
      const r = {};
      st.clear();
      st.patch([body("solo", { x: 0, y: 0, z: 0 })]);
      st.setPose({ position: { x: 0, y: 0, z: 8 }, target: { x: 0, y: 0, z: 0 } });
      r.pick = at();
      st.patch([{ id: "solo", position: { x: 6, y: 0, z: 0 } }]); r.movedAway = at();
      st.patch([{ id: "solo", position: { x: 0, y: 0, z: 0 } }]); r.movedBack = at();
      st.patch([{ id: "solo", params: { w: 3, h: 0.6, d: 0.6 } }]);
      st.patch([{ id: "solo", emphasis: "select" }]);          r.resized = at();
      st.patch([{ id: "solo", visible: false }]);              r.hidden = at();
      st.patch([{ id: "solo", visible: true }]);               r.shown = at();
      const before = st.stats();
      st.remove("solo");
      r.removed = at();
      const nowS = st.stats();
      r.shrank = before.pooledBodies - nowS.pooledBodies === 1 &&
                 before.pooledEdges - nowS.pooledEdges === 1;
      // a kind that must STAY individual still behaves
      st.clear();
      st.patch([{ id: "sp", kind: "spine-box", position: { x: 0, y: 0, z: 0 },
        params: { w: 1.5, h: 0.6, d: 0.6 }, material: "paper", pickable: true }]);
      r.individual = at();
      st.dispose(); window.__host.remove();
      return r;
    });
    t.check("a pooled body picks by id", inv.pick === "solo", JSON.stringify(inv.pick));
    t.check("moving un-picks the old spot and picks the new",
      inv.movedAway === null && inv.movedBack === "solo");
    t.check("resize + emphasis keep it pickable", inv.resized === "solo");
    t.check("hidden is not pickable, shown is again",
      inv.hidden === null && inv.shown === "solo");
    t.check("removal un-picks and shrinks BOTH pools", inv.removed === null && inv.shrank);
    t.check("a non-pooled kind (spine-box) still picks", inv.individual === "sp",
      JSON.stringify(inv.individual));
  } finally {
    srv.stop();
  }
});
