#!/usr/bin/env node
// flow3d-check.mjs — pure (no browser, no WebGL) checks for the 3D flow editor's
// data + projection layers. Run: `node tools/flow3d-check.mjs`.
//
// 1. flowdoc parse/validate: the crate specimen is clean; broken inputs produce
//    diagnostics (I-1…I-10 + structural R-1), never throws.
// 2. Structural round-trip: parse → serialize → parse is stable (R-2: structures,
//    not bytes).
// 3. front-ortho PARITY (acceptance criterion 2): on the crate specimen (all
//    z=0), floweditor3d's projected op-center and terminal positions reproduce
//    the 2D viewer's layout at S = 140 px/unit — compared numerically against a
//    faithful copy of controls/floweditor/floweditor.js's geometry math.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { parse, DIAG, propagationRounds, diffFlow } from "../assets/flowdoc.js";
import { project, halfH, frontOrtho, wireCurvePoint } from "../assets/flowproject.js";
import { tidy, untangle, layerAssign, components, relax, countCrossings } from "../assets/flowlayout.js";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "..");
let failed = 0;
const check = (name, cond, extra = "") => {
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}${extra ? "  — " + extra : ""}`);
  if (!cond) failed++;
};

const specimen = JSON.parse(readFileSync(resolve(ROOT, "harness/fixtures/flow_sample.json"), "utf8")).response.data;

// ── 1. flowdoc parse / validate ─────────────────────────────────────────────
const doc = parse(specimen);
const diags = doc.diagnostics();
const errs = diags.filter((d) => d.severity === "error");
check("specimen parses with no ERROR diagnostics", errs.length === 0, errs.map((d) => d.message).join("; "));
check("specimen has both bars", Object.keys(doc.root.input).length === 2 && Object.keys(doc.root.output).length === 1);
check("specimen has 4 top-level ops", doc.root.cmds.length === 4);
check("specimen local carries a sub-case with 4 ops",
  doc.root.cmds[0].localdata && doc.root.cmds[0].localdata.cmds.length === 4);

// broken input → diagnostics, never a throw
const brokenNode = parse({ input: {}, output: {}, cons: [], cmds: [{ name: "x", type: "primitive", width: 1, pos: { x: 0, y: 0 }, in: { a: {} }, out: {} }] });
const bd = brokenNode.diagnostics();
check("missing pos.z → struct diagnostic (I-9)", bd.some((d) => d.code === DIAG.STRUCT && /pos\.z/.test(d.message)));
check("Node missing mode/type/x → struct diagnostics (R-1)", bd.filter((d) => d.code === DIAG.STRUCT && /Node\./.test(d.message)).length >= 3);
check("unwired input a → I-1", bd.some((d) => d.code === DIAG.I1_UNWIRED));

// a wire cycle → I-2 (op0.out → op1.in, op1.out → op0.in)
const cyc = parse({ input: {}, output: {}, cmds: [
  { name: "+", type: "primitive", width: 1, pos: { x: 0, y: 1, z: 0 }, in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } },
  { name: "+", type: "primitive", width: 1, pos: { x: 0, y: -1, z: 0 }, in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } },
], cons: [{ src: [0, "a"], dest: [1, "a"] }, { src: [1, "a"], dest: [0, "a"] }] });
check("wire cycle → I-2", cyc.diagnostics().some((d) => d.code === DIAG.I2_CYCLE));

// `next` with no nextcase → I-4
const nextNoCase = parse({ input: {}, output: {}, cons: [], cmds: [
  { name: "m", type: "match", ctype: "boolean", width: 1, pos: { x: 0, y: 0, z: 0 }, in: { a: { mode: "", type: "", x: 0 } }, out: {}, condition: { value: true, rule: "next" } },
] });
check("`next` rule with no next case → I-4", nextNoCase.diagnostics().some((d) => d.code === DIAG.I4_NEXT));

// condition on a constant → I-10
const condConst = parse({ input: {}, output: {}, cons: [], cmds: [
  { name: "1", type: "constant", ctype: "int", width: 1, pos: { x: 0, y: 0, z: 0 }, in: {}, out: { a: { mode: "", type: "", x: 0 } }, condition: { value: true, rule: "next" } },
] });
check("condition on a constant → I-10", condConst.diagnostics().some((d) => d.code === DIAG.I10_COND));

// ── 2. structural round-trip ────────────────────────────────────────────────
const once = doc.serialize();
const twice = parse(once).serialize();
check("parse→serialize→parse is structurally stable", JSON.stringify(once) === JSON.stringify(twice));
check("serialized ops all carry pos.{x,y,z}, width, in, out (I-9)",
  once.cmds.every((op) => op.pos && "x" in op.pos && "y" in op.pos && "z" in op.pos && "width" in op && op.in && op.out));
check("serialized nodes all carry mode/type/x (R-1)",
  Object.values(once.input).every((n) => "mode" in n && "type" in n && "x" in n));

// ── 3. front-ortho parity vs the 2D viewer ──────────────────────────────────
// Faithful copy of controls/floweditor/floweditor.js's world→screen math.
const S = 140, NODE_H = 46, BAR_H = 36, MARGIN = 60;
function twoDLayout(c) {
  const xs = c.cmds.map((op) => op.pos.x), ys = c.cmds.map((op) => op.pos.y);
  const minX = Math.min(-1.5, ...xs) - 0.8, maxX = Math.max(1.5, ...xs) + 0.8;
  const minY = Math.min(0, ...ys) - 0.5, maxY = Math.max(0, ...ys) + 0.5;
  const sx = (x) => (x - minX) * S + MARGIN;
  const sy = (y) => BAR_H + 50 + (maxY - y) * S;
  const opCenter = c.cmds.map((op) => ({ x: sx(op.pos.x), y: sy(op.pos.y) }));
  const opTermX = c.cmds.map((op) => {
    const w = Math.max(op.width * S * 0.6, 90), cx = sx(op.pos.x);
    const tx = (n) => cx + Math.max(-w / 2 + 8, Math.min(w / 2 - 8, (n.x ?? 0) * S));
    const clamped = Object.values(op.in).concat(Object.values(op.out)).some((n) => Math.abs((n.x ?? 0) * S) > w / 2 - 8);
    return { in: Object.fromEntries(Object.entries(op.in).map(([k, n]) => [k, tx(n)])),
             out: Object.fromEntries(Object.entries(op.out).map(([k, n]) => [k, tx(n)])), clamped };
  });
  return { opCenter, opTermX };
}

const two = twoDLayout(doc.root);
const proj = project(doc.root);

// pull the 3D op-body world positions from the projected specs (active deck d0)
const opSpecs = doc.root.cmds.map((_, i) => proj.specs.find((s) => s.id === `d0/op/${i}`));
check("projection emits every op body", opSpecs.every(Boolean));

// front-ortho maps world→screen; solve the screen origin from op 0 so we compare
// the LAYOUT (relative positions), which is what "reproduces the 2D picture" means.
const ref3D = opSpecs[0].position, ref2D = two.opCenter[0];
const originX = ref2D.x - ref3D.x * S;
const originY = ref2D.y - (-ref3D.y * S); // frontOrtho does the y-flip; solve origin
let maxCenterErr = 0;
opSpecs.forEach((s, i) => {
  const p = frontOrtho(s.position, S, originX, originY);
  maxCenterErr = Math.max(maxCenterErr, Math.hypot(p.x - two.opCenter[i].x, p.y - two.opCenter[i].y));
});
check("op-center front-ortho parity (px error < 1e-6)", maxCenterErr < 1e-6, `max ${maxCenterErr.toExponential(2)} px`);

// horizontal terminal offsets: the persisted node.x maps identically at S=140.
// The 2D viewer draws op boxes at 0.6×width and CLAMPS terminals to that
// compressed box; the 3D box is full `width` (design §2.3) so it honors node.x
// faithfully. So parity is total on un-clamped terminals, and on clamped ones
// the 3D view is *more* faithful to the persisted data than the 2D viewer.
let maxTermErr = 0, unclamped = 0, clampedTerms = 0, clampFaithful = 0;
doc.root.cmds.forEach((op, i) => {
  const h = halfH(op.type);
  const both = [["in", op.in, +h], ["out", op.out, -h]];
  for (const [which, nodes, dy] of both) {
    for (const [term, n] of Object.entries(nodes)) {
      const world = { x: op.pos.x + (n.x ?? 0), y: op.pos.y + dy, z: op.pos.z };
      const p = frontOrtho(world, S, originX, originY);
      if (two.opTermX[i].clamped) {
        clampedTerms++;
        // 3D world x, un-mapped, equals the persisted offset op.pos.x + node.x
        if (Math.abs((p.x - originX) / S - world.x) < 1e-9) clampFaithful++;
        continue;
      }
      maxTermErr = Math.max(maxTermErr, Math.abs(p.x - two.opTermX[i][which][term]));
      unclamped++;
    }
  }
});
check("un-clamped terminals: horizontal parity (px error < 1e-6)", maxTermErr < 1e-6, `max ${maxTermErr.toExponential(2)} px over ${unclamped} terminals`);
check("clamped terminals: 3D honors persisted node.x (2D compresses them)",
  clampedTerms > 0 && clampFaithful === clampedTerms,
  `${clampFaithful}/${clampedTerms} faithful — 2D viewer clamps these into its 0.6×width box`);

// wires connect the correct sockets: every top-level connection resolves to a
// tube spec whose endpoints match the addressed terminal positions.
const wireSpecs = proj.specs.filter((s) => s.id.startsWith("d0/wire/"));
check("projection emits every wire", wireSpecs.length === doc.root.cons.length, `${wireSpecs.length}/${doc.root.cons.length}`);

// ── 4. mutations with inverses (3D-2, design §5.3/§5.4) ─────────────────────
{
  const d = parse(specimen), c = d.root;
  // move op 1 and undo
  const p0 = { ...c.cmds[1].pos };
  const mv = d.moveOp(c, 1, { x: 2.5, y: -1, z: 1.2 });
  check("moveOp applies", c.cmds[1].pos.z === 1.2 && c.cmds[1].pos.x === 2.5);
  mv.undo();
  check("moveOp undo restores", c.cmds[1].pos.x === p0.x && c.cmds[1].pos.z === p0.z);
  mv.redo();
  check("moveOp redo re-applies", c.cmds[1].pos.z === 1.2);

  // resize with the 0.6 floor
  const rz = d.resizeOp(c, 1, 0.1);
  check("resizeOp floors at 0.6", c.cmds[1].width === 0.6);
  rz.undo(); check("resizeOp undo restores", Math.abs(c.cmds[1].width - 1.75) < 1e-9);

  // terminal move clamps to ±(width/2 − 0.1)
  const tm = d.moveTerminal(c, 1, "in", "a", -99);
  check("moveTerminal clamps", Math.abs(c.cmds[1].in.a.x - (-(1.75 / 2 - 0.1))) < 1e-9);
  tm.undo(); check("moveTerminal undo restores", Math.abs(c.cmds[1].in.a.x - (-0.375)) < 1e-9);
}

// ── 5. wiring invariants I-1 (fan-in 1) and I-2 (no cycle) ───────────────────
{
  // two ops, one wire 0.a → 1.a
  const two = parse({
    input: {}, output: {},
    cmds: [
      { name: "+", type: "primitive", width: 1, pos: { x: 0, y: 1, z: 0 }, in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } },
      { name: "+", type: "primitive", width: 1, pos: { x: 0, y: -1, z: 0 }, in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } },
    ],
    cons: [{ src: [0, "a"], dest: [1, "a"] }],
  });
  const c = two.root;
  check("I-1: wiring an already-wired input is refused", two.addWire(c, { src: [0, "a"], dest: [1, "a"] }).error === "input already wired");
  check("I-2: a wire that closes a cycle is refused", two.addWire(c, { src: [1, "a"], dest: [0, "a"] }).error === "would create a cycle");
  check("self-wire refused", two.addWire(c, { src: [0, "a"], dest: [0, "a"] }).error != null);
  // remove the wire, then the reverse wire becomes legal (no cycle)
  const rm = two.removeWire(c, 0);
  check("removeWire applies", c.cons.length === 0);
  const add = two.addWire(c, { src: [1, "a"], dest: [0, "a"] });
  check("legal wire applies after removal", !add.error && c.cons.length === 1);
  add.undo(); check("addWire undo removes it", c.cons.length === 0);
  rm.undo(); check("removeWire undo restores at index", c.cons.length === 1 && c.cons[0].src[0] === 0);

  // structural round-trip survives mutations
  const before = two.serialize();
  const m = two.moveOp(c, 0, { x: 3, y: 3, z: 0.5 });
  m.undo();
  check("round-trip stable after mutate+undo", JSON.stringify(two.serialize()) === JSON.stringify(before));
}

// ── 6. authoring mutations (3D-3, design §5.5) ──────────────────────────────
{
  const d = parse(specimen), c = d.root;
  const N = c.cmds.length, W = c.cons.length;
  const snap = () => JSON.stringify(d.serialize());
  const before = snap();

  // addOp appends; removeOp rewrites cons indices in the same mutation
  const add = d.addOp(c, { name: "trim", type: "primitive", width: 1.75, pos: { x: 4, y: 0, z: 0 },
    in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } });
  check("addOp appends (index = old length)", !add.error && add.index === N && c.cmds.length === N + 1);
  const wireToNew = d.addWire(c, { src: [1, "a"], dest: [N, "a"] });
  check("wire to the new op", !wireToNew.error && c.cons.length === W + 1);
  // remove op 0 (the local): its wires drop, higher indices shift down
  const touching = d.wiresTouching(c, 0);
  const rm = d.removeOp(c, 0);
  check("removeOp drops the op + its wires", !rm.error && c.cmds.length === N && c.cons.length === W + 1 - touching);
  check("removeOp rewrote higher indices down", c.cons.some((w) => w.src[0] === 0 && c.cmds[0].name === "+")
    && c.cons.every((w) => w.src[0] < c.cmds.length && w.dest[0] < c.cmds.length));
  rm.undo(); wireToNew.undo(); add.undo();
  check("add/wire/remove undone → byte-identical structures", snap() === before);

  // renameTerminal rewrites cons + the signature case (bar rename)
  const rn = d.renameTerminal(c, -1, "out", "a", "alpha");
  check("bar rename rewrites wires", !rn.error && c.cons.some((w) => w.src[0] === -1 && w.src[1] === "alpha")
    && !c.cons.some((w) => w.src[0] === -1 && w.src[1] === "a") && "alpha" in c.input);
  rn.undo();
  check("bar rename undo restores", snap() === before);

  // splice: interpose a new op into wire 0
  const add2 = d.addOp(c, { name: "to_string", type: "primitive", width: 1.75, pos: { x: 0, y: 2, z: 0 },
    in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } });
  const w0 = { src: [...c.cons[0].src], dest: [...c.cons[0].dest] };
  const sp = d.spliceIntoWire(c, 0, add2.index);
  check("splice replaces one wire with two through the op",
    !sp.error && c.cons.length === W + 1
    && c.cons[0].src[0] === w0.src[0] && c.cons[0].dest[0] === add2.index
    && c.cons[1].src[0] === add2.index && c.cons[1].dest[0] === w0.dest[0]);
  sp.undo(); add2.undo();
  check("splice undo restores", snap() === before);

  // matcher toggle: constant (op 2) ⇄ match
  const mt = d.setMatcher(c, 2, true);
  check("constant → matcher gains input a + legal ctype", !mt.error && c.cmds[2].type === "match"
    && "a" in c.cmds[2].in && ["int","decimal","boolean","string","null"].includes(c.cmds[2].ctype));
  mt.undo();
  check("matcher undo restores", snap() === before);

  // condition guards: I-10 (constant) and I-4 (next w/o chain)
  check("condition on a constant refused (I-10)", d.setCondition(c, 2, { value: true, rule: "fail" }).error != null);
  check("`next` without a next case refused (I-4)", d.setCondition(c, 1, { value: true, rule: "next" }, { chainHasNext: false }).error != null);
  const cond = d.setCondition(c, 1, { value: true, rule: "terminate" });
  check("legal condition applies", !cond.error && c.cmds[1].condition.rule === "terminate");
  cond.undo();
  check("condition undo restores", snap() === before);

  // terminal add/mode/type on a bar and an op
  const at = d.addTerminal(c, -1, "out", "c", "integer");
  check("bar addTerminal allocates a free x slot", !at.error && "c" in c.input && c.input.c.x > Math.max(c.input.a.x, c.input.b.x));
  check("duplicate terminal refused (I-3)", d.addTerminal(c, -1, "out", "c").error != null);
  at.undo();
  const lm = d.setTerminalMode(c, 1, "out", "a", "loop", "a");
  check("loop output pairs with an input (I-8)", !lm.error && c.cmds[1].out.a.mode === "loop" && c.cmds[1].out.a.loop === "a");
  check("loop pairing to a missing input refused (I-8)", d.setTerminalMode(c, 1, "out", "a", "loop", "zz").error != null);
  lm.undo();
  check("all authoring undone → byte-identical structures", snap() === before);

  // case chain: add copies the signature; removeCase guards I-4
  const ac = d.addCase(c);
  check("addCase appends an empty deck sharing the bars", !ac.error && c.nextcase
    && Object.keys(c.nextcase.input).join() === Object.keys(c.input).join() && c.nextcase.cmds.length === 0);
  const nx = d.setCondition(c, 1, { value: true, rule: "next" }, { chainHasNext: true });
  check("`next` legal once the chain has a case", !nx.error);
  check("removing the last case is refused while `next` points at it",
    d.removeCase(c, 1).error != null);
  nx.undo();
  const rc = d.removeCase(c, 1);
  check("removeCase splices the chain", !rc.error && !c.nextcase);
  rc.undo(); check("removeCase undo restores the deck", !!c.nextcase);
  ac.undo();
  check("case authoring undone → byte-identical structures", snap() === before);
}

// ── 7. auto-layout proposals (3D-4, design §7.1) ────────────────────────────
{
  // a diamond: 0 feeds 1 and 2; both feed 3 — plus a disjoint pair 4→5.
  const mk = (name, x, y) => ({ name, type: "primitive", width: 1.5, pos: { x, y, z: 0 },
    in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } });
  const d = parse({ input: {}, output: {},
    cmds: [mk("s", 0, 3), mk("l", -3, 0), mk("r", 3, 0), mk("j", 0, -3), mk("p", 9, 9), mk("q", 9, 6)],
    cons: [
      { src: [0, "a"], dest: [1, "a"] }, { src: [0, "a"], dest: [2, "a"] },
      { src: [1, "a"], dest: [3, "a"] },
      { src: [4, "a"], dest: [5, "a"] },
    ] });
  const c = d.root;
  const depth = layerAssign(c);
  check("layering: source 0, mids 1, join 2, pair 0/1",
    depth[0] === 0 && depth[1] === 1 && depth[2] === 1 && depth[3] === 2 && depth[4] === 0 && depth[5] === 1,
    JSON.stringify(depth));
  check("components: two", components(c).count === 2);

  const t = tidy(c);
  check("tidy proposes a position per op", t.positions.length === 6 && t.positions.every(Boolean));
  check("tidy: y strictly descends per wire hop (down = later)",
    c.cons.filter((w) => w.src[0] >= 0 && w.dest[0] >= 0)
      .every((w) => t.positions[w.src[0]].y > t.positions[w.dest[0]].y));
  check("tidy keeps z untouched", t.positions.every((p, i) => p.z === c.cmds[i].pos.z));
  const layer1 = [1, 2].map((i) => t.positions[i]);
  check("tidy: same-layer ops don't overlap in x",
    Math.abs(layer1[0].x - layer1[1].x) >= 1.5 + 0.6 - 1e-9);

  const u = untangle(c);
  check("untangle: disjoint components separate in z",
    u.positions[0].z !== u.positions[4].z &&
    new Set(u.positions.map((p) => p.z)).size === 2, JSON.stringify([...new Set(u.positions.map((p) => p.z))]));
  check("untangle: z stays within ±1.5", u.positions.every((p) => Math.abs(p.z) <= 1.5 + 1e-9));
  check("untangle: y layering identical to tidy's", u.positions.every((p, i) => p.y === t.positions[i].y));

  // applyLayout is ONE undo step and restores exactly
  const before = JSON.stringify(d.serialize());
  const cmd = d.applyLayout(c, t.positions, t.label);
  check("applyLayout applies", !cmd.error && c.cmds[3].pos.y === t.positions[3].y);
  cmd.undo();
  check("applyLayout undo restores byte-identical structures", JSON.stringify(d.serialize()) === before);
  check("applyLayout refuses a mismatched proposal", d.applyLayout(c, [{ x: 0, y: 0, z: 0 }], "bad").error != null);

  // legacy planarity: tidy on an all-z=0 case proposes all-z=0 (§2.7 safety)
  check("tidy keeps a legacy case planar", t.positions.every((p) => p.z === 0));
}

// ── 7b. untangle's per-layer xz relaxation (3D-P1, design §7.1) ─────────────
{
  const mk = (name, x, y) => ({ name, type: "primitive", width: 1.5, pos: { x, y, z: 0 },
    in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } });

  // A crossing that x-ORDERING can fix is fixed by tidy's barycenter sweep —
  // the relaxation exists for what survives that. So the fixture must be a
  // genuinely NON-PLANAR tangle: K3,3 (every source wired to every sink) has
  // no crossing-free 2-layer drawing at any ordering, and it is ONE connected
  // component, so component separation (the v1 untangle) can't help either.
  const crosswise = parse({ input: {}, output: {},
    cmds: [mk("s0", -2, 3), mk("s1", 2, 3), mk("t0", -2, 0), mk("t1", 2, 0)],
    cons: [{ src: [0, "a"], dest: [3, "a"] }, { src: [1, "a"], dest: [2, "a"] }] }).root;
  check("a merely crosswise pair is already fixed by tidy's ordering",
    countCrossings(crosswise, layerAssign(crosswise), tidy(crosswise).positions) === 0);

  const cmds = [mk("s0", -3, 3), mk("s1", 0, 3), mk("s2", 3, 3),
                mk("t0", -3, 0), mk("t1", 0, 0), mk("t2", 3, 0)];
  const cons = [];
  for (let s = 0; s < 3; s++) for (let t = 3; t < 6; t++) cons.push({ src: [s, "a"], dest: [t, "a"] });
  const tangle = parse({ input: {}, output: {}, cmds, cons }).root;
  check("tangle is one component", components(tangle).count === 1);
  const td = layerAssign(tangle);
  const flat = tidy(tangle).positions;
  const before = countCrossings(tangle, td, flat);
  check("tidy alone can't clear a non-planar tangle", before > 0, `${before} crossings`);

  const after = countCrossings(tangle, td, relax(tangle, td, flat));
  check("relaxation resolves the crossing in depth", after < before, `${before} → ${after}`);
  check("untangle() now applies the relaxation",
    countCrossings(tangle, td, untangle(tangle).positions) < before);

  // y is hard-pinned THROUGHOUT: down = later is never optimized away (§7.1)
  const r = relax(tangle, td, flat);
  check("relaxation never moves y", r.every((p, i) => p.y === flat[i].y));
  check("relaxation keeps z within ±1.5", r.every((p) => Math.abs(p.z) <= 1.5 + 1e-9));

  // same-layer op bodies never interpenetrate (the projection is a hard
  // constraint, not a force — same idiom as forcelayout's separation)
  const byLayer = new Map();
  td.forEach((d, i) => { if (!byLayer.has(d)) byLayer.set(d, []); byLayer.get(d).push(i); });
  let minClear = Infinity;
  for (const L of byLayer.values()) {
    for (let a = 0; a < L.length; a++) {
      for (let b = a + 1; b < L.length; b++) {
        const i = L[a], j = L[b];
        const needX = (tangle.cmds[i].width + tangle.cmds[j].width) / 2 + 0.6;
        // clear if separated on EITHER axis; measure the slack on the better one
        minClear = Math.min(minClear,
          Math.max(Math.abs(r[j].x - r[i].x) - needX, Math.abs(r[j].z - r[i].z) - 0.9));
      }
    }
  }
  // (the projection converges to an epsilon, not to exact zero — Gauss–Seidel)
  check("same-layer boxes never overlap after relaxation", minClear >= -1e-6,
    `slack ${minClear.toExponential(2)}`);

  // determinism: no randomness anywhere (resume safety, reviewable diffs)
  check("relaxation is deterministic",
    JSON.stringify(relax(tangle, td, flat)) === JSON.stringify(relax(tangle, td, flat)));

  // an ALREADY-CLEAN graph is left alone in z: legacy planar cases that have
  // nothing to untangle don't get gratuitously unfolded
  const clean = parse({ input: {}, output: {},
    cmds: [mk("a", 0, 3), mk("b", 0, 0)],
    cons: [{ src: [0, "a"], dest: [1, "a"] }] }).root;
  const cd = layerAssign(clean);
  const cleanFlat = tidy(clean).positions;
  check("nothing to untangle ⇒ z untouched",
    relax(clean, cd, cleanFlat).every((p, i) => p.z === cleanFlat[i].z));
}

// ── 7c. journal diff (3D-P3): structural, in the graph's vocabulary ─────────
{
  const mk = (n, x, y) => ({ name: n, type: "primitive", width: 1.5, pos: { x, y, z: 0 },
    in: { a: { mode: "regular", type: "integer", x: 0 } },
    out: { a: { mode: "regular", type: "integer", x: 0 } } });
  const BASE = {
    input: { a: { mode: "regular", type: "integer", x: 0.25 } },
    output: { a: { mode: "regular", type: "integer", x: 0 } },
    cmds: [mk("first", -2, 1.6), mk("second", 2, -1.6)],
    cons: [{ src: [-1, "a"], dest: [0, "a"] }, { src: [1, "a"], dest: [-2, "a"] }],
  };
  const clone = (o) => JSON.parse(JSON.stringify(o));
  const texts = (d) => d.changes.map((c) => c.text);

  check("identical bodies diff to nothing", diffFlow(BASE, BASE).changes.length === 0);

  const wired = clone(BASE); wired.cons.push({ src: [0, "a"], dest: [1, "a"] });
  const dw = diffFlow(BASE, wired);
  check("a new wire reads as one wire+ in NAMES, not indices",
    dw.changes.length === 1 && dw.changes[0].kind === "wire+" &&
    dw.changes[0].text === "wired first.a to second.a", JSON.stringify(texts(dw)));
  check("the reverse diff is the unwire",
    diffFlow(wired, BASE).changes[0].kind === "wire-");

  const moved = clone(BASE); moved.cmds[0].pos.z = 0.9;
  check("a move reads as a move with the new coords",
    /^moved first to \(-2\.00, 1\.60, 0\.90\)$/.test(texts(diffFlow(BASE, moved))[0]),
    JSON.stringify(texts(diffFlow(BASE, moved))));

  const renamed = clone(BASE); renamed.cmds[1].name = "third";
  const dr = diffFlow(BASE, renamed);
  check("a rename reads as a rename and does NOT churn that op's wires",
    dr.changes.length === 1 && dr.changes[0].kind === "op~" &&
    dr.changes[0].text === "renamed second to third", JSON.stringify(texts(dr)));

  // the hard case: op identity is positional (cons carry INDICES), so removing
  // a middle op renumbers everything after it. The matcher must report one
  // removal, not a cascade of false edits.
  const three = clone(BASE);
  three.cmds.splice(1, 0, mk("middle", 0, 0));
  three.cons = [{ src: [-1, "a"], dest: [0, "a"] }, { src: [0, "a"], dest: [1, "a"] },
                { src: [1, "a"], dest: [2, "a"] }, { src: [2, "a"], dest: [-2, "a"] }];
  const pruned = clone(three);
  pruned.cmds.splice(1, 1);                       // drop `middle`
  pruned.cons = [{ src: [-1, "a"], dest: [0, "a"] }, { src: [0, "a"], dest: [1, "a"] },
                 { src: [1, "a"], dest: [-2, "a"] }];
  const dp = diffFlow(three, pruned);
  check("removing a MIDDLE op reports one removal, not renumbering noise",
    dp.changes.filter((c) => c.kind === "op-").length === 1 &&
    // anchored: "removed" CONTAINS "moved" — an unanchored regex passes nothing
    dp.changes.every((c) => !/^(moved|renamed) /.test(c.text)), JSON.stringify(texts(dp)));
  check("its two wires vanish and the bypass appears (by name)",
    texts(dp).includes("unwired first.a to middle.a") &&
    texts(dp).includes("unwired middle.a to second.a") &&
    texts(dp).includes("wired first.a to second.a"), JSON.stringify(texts(dp)));

  // signature (the bars) and deck-chain changes
  const sig = clone(BASE); sig.input.b = { mode: "regular", type: "string", x: 0.5 };
  check("adding a parameter reads as a signature change",
    texts(diffFlow(BASE, sig))[0] === "params + b:string");
  const retyped = clone(BASE); retyped.output.a.type = "string";
  check("retyping a return reads with both types",
    texts(diffFlow(BASE, retyped))[0] === "return a: integer to string");
  const decked = clone(BASE); decked.nextcase = clone(BASE);
  check("adding a case reads as a deck change",
    diffFlow(BASE, decked).changes.some((c) => c.kind === "deck+"));
  check("removing that case is the mirror",
    diffFlow(decked, BASE).changes.some((c) => c.kind === "deck-"));

  // locals are summarized, never recursed (an unbounded nested diff buries it)
  const withLocal = clone(BASE);
  withLocal.cmds[0].type = "local";
  withLocal.cmds[0].localdata = clone(BASE);
  const dl = diffFlow(BASE, withLocal);
  check("a local sub-flow is summarized, not expanded",
    texts(dl).some((t) => /local sub-flow added/.test(t)) &&
    !texts(dl).some((t) => /wired/.test(t)), JSON.stringify(texts(dl)));

  // counts drive the "+2 −1 ~3" header
  const many = clone(wired); many.cmds[0].name = "start"; many.input.c = { mode: "regular", type: "int", x: 0 };
  const dm = diffFlow(BASE, many);
  // a wire+ is `added`; a rename and a signature edit are both `changed`
  // (kind "sig" is a modification of the bars, not an addition to the graph)
  check("counts tally by kind",
    dm.counts.added === 1 && dm.counts.removed === 0 && dm.counts.changed === 2,
    JSON.stringify(dm.counts) + " " + JSON.stringify(texts(dm)));

  // a body that isn't valid JSON must diagnose, not throw (journal entries are
  // strings from the store and can be anything)
  let threw = false;
  try { diffFlow("{not json", JSON.stringify(BASE)); } catch { threw = true; }
  check("a malformed body diffs without throwing", !threw);
}

// ── 8. propagation schedule + wire-curve math (3D-5, design §3.5) ───────────
{
  const d = parse(specimen);
  const { rounds, unreached } = propagationRounds(d.root);
  const firedInOrder = rounds.flat();
  check("schedule fires every wire of the clean specimen", firedInOrder.length === d.root.cons.length && unreached.length === 0,
    `${rounds.length} rounds, ${firedInOrder.length}/${d.root.cons.length} wires`);
  // causality: any wire OUT of op i fires in a strictly later round than the
  // last wire INTO op i.
  const roundOf = new Map();
  rounds.forEach((r, ri) => r.forEach((k) => roundOf.set(k, ri)));
  let causal = true;
  d.root.cmds.forEach((op, i) => {
    const into = d.root.cons.map((w, k) => [w, k]).filter(([w]) => w.dest[0] === i).map(([, k]) => roundOf.get(k));
    const outOf = d.root.cons.map((w, k) => [w, k]).filter(([w]) => w.src[0] === i).map(([, k]) => roundOf.get(k));
    if (into.length && outOf.length && Math.min(...outOf) <= Math.max(...into)) causal = false;
  });
  check("schedule is causal (outputs after all inputs)", causal);

  // §1.5's name-only eager pass, both directions:
  // (a) an unwired input whose NAME is a dest nowhere is pre-marked done —
  //     the op still runs (the interpreter does not hang here);
  const softBroken = parse({ input: { a: { mode: "", type: "", x: 0 } }, output: { a: { mode: "", type: "", x: 0 } },
    cmds: [{ name: "+", type: "primitive", width: 1, pos: { x: 0, y: 0, z: 0 },
      in: { a: { mode: "", type: "", x: 0 }, b: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } }],
    cons: [{ src: [-1, "a"], dest: [0, "a"] }, { src: [0, "a"], dest: [-2, "a"] }] });
  const sb = propagationRounds(softBroken.root);
  check("unwired input with a globally-unwired NAME still runs (interpreter semantics)",
    sb.unreached.length === 0 && sb.rounds.flat().length === 2);
  // (b) the I-1 hang: an unwired input whose name IS wired elsewhere waits
  //     forever — its op's output wire never fires.
  const hang = parse({ input: { a: { mode: "", type: "", x: 0 } }, output: { a: { mode: "", type: "", x: 0 } },
    cmds: [
      { name: "+", type: "primitive", width: 1, pos: { x: 0, y: 1, z: 0 },
        in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } },
      { name: "trim", type: "primitive", width: 1, pos: { x: 2, y: 1, z: 0 },
        in: { a: { mode: "", type: "", x: 0 } }, out: { a: { mode: "", type: "", x: 0 } } }, // input `a` unwired, but `a` IS a dest name globally
    ],
    cons: [{ src: [-1, "a"], dest: [0, "a"] }, { src: [1, "a"], dest: [-2, "a"] }] });
  const hg = propagationRounds(hang.root);
  check("I-1 hang: colliding-name unwired input blocks its op's out-wire",
    hg.unreached.length === 1 && hang.root.cons[hg.unreached[0]].src[0] === 1);

  // wire curve: endpoints exact; interior stays between; initial motion is -Y
  const from = { x: -1, y: 1, z: 0 }, to = { x: 1, y: -1, z: 0.5 };
  const p0 = wireCurvePoint(from, to, 0), p1 = wireCurvePoint(from, to, 1);
  check("curve endpoints exact", Math.hypot(p0.x - from.x, p0.y - from.y, p0.z - from.z) < 1e-12 &&
    Math.hypot(p1.x - to.x, p1.y - to.y, p1.z - to.z) < 1e-12);
  const pEarly = wireCurvePoint(from, to, 0.05);
  check("curve exits the source straight down (vertical tangent)", pEarly.y < from.y && Math.abs(pEarly.x - from.x) < 0.02);
}

console.log(`\n${failed === 0 ? "ALL PASS" : failed + " FAILED"}`);
process.exit(failed ? 1 : 0);
