#!/usr/bin/env node
// scene-check.mjs — pure (no browser, no WebGL) checks for the scene facet's
// data layers (scene-facet-design SC-0). Run: `node tools/scene-check.mjs`.
//
// 1. sceneexpr: grammar, precedence, semantics, dependency extraction, and
//    that bad input errors without throwing.
// 2. scenedoc parse/validate: the specimen is clean; broken docs produce
//    SD-1…SD-8/SD-10 diagnostics, never throws.
// 3. Round-trip BYTE-stability (acceptance 3): untouched docs serialize
//    byte-identical, unknown keys included; composed mutation+undo chains
//    restore the exact bytes (the flowdoc identity lesson).

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { parse as parseExpr, evalExpr, compile } from "../assets/sceneexpr.js";
import { parse as parseScene, DIAG, bindablePaths, parseTarget, readProp } from "../assets/scenedoc.js";
import { TOKEN_NAMES, resolveMaterial } from "../assets/scenetokens.js";
import { project, envOf, deltaFor, instanceSpec } from "../assets/sceneproject.js";
import { createRuntime } from "../assets/scenerun.js";
import { step as forceStep, energy } from "../assets/forcelayout.js";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "..");
let failed = 0;
const check = (name, cond, extra = "") => {
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}${extra ? "  — " + extra : ""}`);
  if (!cond) failed++;
};
const ev = (src, scope) => {
  const p = parseExpr(src);
  if (!p.ok) return { parseError: p.error };
  return evalExpr(p.ast, scope || {});
};
const val = (src, scope) => ev(src, scope).value;

// ── 1. sceneexpr ────────────────────────────────────────────────────────────
check("precedence: 2+3*4", val("2+3*4") === 14);
check("parens: (2+3)*4", val("(2+3)*4") === 20);
check("unary minus binds tight: -2*3+10", val("-2*3+10") === 4);
check("ternary: right-assoc chain", val("1 ? 2 : 0 ? 3 : 4") === 2 && val("0 ? 2 : 0 ? 3 : 4") === 4);
check("comparisons and equality", val("2 < 3") === true && val("'a' == 'a'") === true && val("2 != 2") === false);
check("string concat via +", val("'v' + 1") === "v1" && val("1 + 2 + 'x'") === "3x");
check("boolean ops short-circuit", ev("true || missing").ok && val("true || missing") === true && ev("false && missing").ok);
check("modulo and division", val("7 % 3") === 1 && val("1 / 2") === 0.5);
check("functions: clamp/lerp/round", val("clamp(5, 0, 2)") === 2 && val("lerp(0, 10, 0.3)") === 3 && val("round(2.5)") === 3);
check("deg/rad round-trip", Math.abs(val("deg(rad(90))") - 90) < 1e-12);
check("PI folds to a literal (not a dep)", (() => { const p = parseExpr("PI * 2"); return p.ok && p.deps.size === 0 && Math.abs(evalExpr(p.ast, {}).value - Math.PI * 2) < 1e-12; })());
check("deps extraction", (() => { const p = parseExpr("open ? t * speed : clamp(x, 0, 1)"); return p.ok && [...p.deps].sort().join(",") === "open,speed,t,x"; })());
check("scope lookup + locals shadow nothing weird", val("a + b", { a: 1, b: 2 }) === 3);
check("unknown identifier at eval is a soft error", (() => { const r = ev("nope + 1"); return r.ok === false && /unknown identifier/.test(r.error); })());
check("parse error: unterminated string", parseExpr("'abc").ok === false);
check("parse error: unknown function", parseExpr("wat(1)").ok === false && /unknown function/.test(parseExpr("wat(1)").error));
check("parse error: bad arity", parseExpr("clamp(1, 2)").ok === false && /3 arguments/.test(parseExpr("clamp(1, 2)").error));
check("parse error: trailing garbage", parseExpr("1 2").ok === false);
check("parse error: empty", parseExpr("   ").ok === false);
check("compile() convenience", (() => { const c = compile("n * 2"); return c.ok && c.run({ n: 4 }).value === 8 && c.deps.has("n"); })());
check("negation and not", val("!false") === true && val("-(3)") === -3);
check("null literal", val("null") === null);

// ── 2. scenedoc: the specimen ───────────────────────────────────────────────
const specimenText = readFileSync(resolve(ROOT, "harness/fixtures/scene_sample.json"), "utf8");
const specimen = JSON.parse(specimenText);
const doc = parseScene(specimen);
{
  const diags = doc.diagnostics();
  const errs = diags.filter((d) => d.severity === "error");
  check("specimen has no ERROR diagnostics", errs.length === 0, errs.map((d) => `${d.path}: ${d.message}`).join("; "));
  check("specimen is animated (t in a binding)", doc.animated() === true);
  check("byId and childrenOf", doc.byId("knob").type === "node" && doc.childrenOf("base").nodes.length === 4);
  check("state defaults", doc.stateDefaults().open === false && doc.stateDefaults().speed === 0.6);
  check("readProp applies kind defaults", doc.read("arm", "scale.x") === 1 && doc.read("knob", "radius") === 0.3 && doc.read("base", "visible") === true);
  check("bindablePaths: box binds size, sphere binds radius",
    bindablePaths("box").includes("size.y") && bindablePaths("sphere").includes("radius") && !bindablePaths("sphere").includes("size.y"));
  check("parseTarget splits at the first dot", (() => { const t = parseTarget("knob.material.token"); return t.id === "knob" && t.path === "material.token"; })());
  check("material tokens resolve per theme", resolveMaterial({ token: "accent" }, "light").color === "#62BD8C" && TOKEN_NAMES.includes("glass"));
}

// ── 3. round-trip byte-stability ────────────────────────────────────────────
const orig = JSON.stringify(specimen);
check("untouched round-trip is byte-identical", JSON.stringify(doc.serialize()) === orig);
check("serialize is a deep copy", (() => { const s = doc.serialize(); s.nodes[0].id = "hax"; return doc.root.nodes[0].id === "base"; })());
{
  const withUnknown = JSON.parse(specimenText);
  withUnknown.poses = [{ future: true }];
  withUnknown.nodes[0].custom_key = { nested: [1, 2] };
  const d2 = parseScene(withUnknown);
  check("unknown keys survive round-trip byte-identical", JSON.stringify(d2.serialize()) === JSON.stringify(withUnknown));
  check("unknown top-level key is an SD-10 info", d2.diagnostics().some((d) => d.code === DIAG.SD10 && d.path === "poses"));
}

// ── 4. diagnostics on broken docs (never throws) ────────────────────────────
{
  const d = parseScene("not json {");
  check("bad JSON string → diagnostic, not throw", d.diagnostics().some((x) => x.severity === "error"));
}
{
  const d = parseScene({ nodes: [{ id: "a", kind: "box" }, { id: "a", kind: "sphere" }] });
  check("SD-1 duplicate id", d.diagnostics().some((x) => x.code === DIAG.SD1 && /duplicate/.test(x.message)));
}
{
  const d = parseScene({ nodes: [{ id: "a", kind: "box", parent: "ghost" }] });
  check("SD-2 missing parent", d.diagnostics().some((x) => x.code === DIAG.SD2));
}
{
  const d = parseScene({ nodes: [{ id: "a", kind: "box", pos: { x: "3.5" } }] });
  check("SD-3 string transform (the legacy wart) is an error", d.diagnostics().some((x) => x.code === DIAG.SD3 && /legacy/.test(x.message)));
}
{
  const d = parseScene({ nodes: [{ id: "a", kind: "box", material: { token: "chartreuse" } }] });
  check("SD-4 unknown token warns", d.diagnostics().some((x) => x.code === DIAG.SD4 && x.severity === "warn"));
}
{
  const d = parseScene({ nodes: [{ id: "a", kind: "sphere" }],
    bindings: [{ target: "a.size.x", expr: "1" }, { target: "a.radius", expr: "1" }, { target: "a.radius", expr: "2" }] });
  const diags = d.diagnostics();
  check("SD-5 non-bindable path + duplicate target", diags.some((x) => x.code === DIAG.SD5 && /not bindable/.test(x.message)) && diags.some((x) => x.code === DIAG.SD5 && /duplicate/.test(x.message)));
}
{
  const d = parseScene({ state: [{ name: "n", type: "number", value: 1 }], nodes: [{ id: "a", kind: "box" }],
    bindings: [{ target: "a.pos.x", expr: "n + ghost" }] });
  check("SD-6 out-of-scope identifier", d.diagnostics().some((x) => x.code === DIAG.SD6 && /ghost/.test(x.message)));
}
{
  const d = parseScene({ nodes: [{ id: "a", kind: "box" }], wires: [{ on: "a.tap", do: [{ set: "missing", to: "1" }] }] });
  const diags = d.diagnostics();
  check("SD-7 undeclared affordance warns + bad set target errors",
    diags.some((x) => x.code === DIAG.SD7 && /affordance/.test(x.message)) && diags.some((x) => x.code === DIAG.SD7 && /unknown state field/.test(x.message)));
}
{
  const d = parseScene({ nodes: [{ id: "p", kind: "box", affordances: { drag: { plane: "xz" } } }],
    state: [{ name: "v", type: "number", value: 0 }],
    wires: [{ on: "p.dragmove", do: [{ set: "v", to: "x + dx" }] }] });
  check("drag locals are in scope under drag events", d.diagnostics().filter((x) => x.code === DIAG.SD6).length === 0);
}
{
  const d = parseScene({ state: [{ name: "x", type: "number", value: "one" }, { name: "x", type: "number", value: 1 }] });
  const diags = d.diagnostics();
  check("SD-8 bad default + duplicate name", diags.some((x) => x.code === DIAG.SD8 && /not a number/.test(x.message)) && diags.some((x) => x.code === DIAG.SD8 && /duplicate/.test(x.message)));
}

// ── 5. mutations with inverses ──────────────────────────────────────────────
function freshDoc() { return parseScene(JSON.parse(specimenText)); }
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const m = d.setProp("base", "pos.x", 2.5);
  check("setProp applies", !m.error && d.read("base", "pos.x") === 2.5);
  m.undo();
  check("setProp undo restores bytes", JSON.stringify(d.serialize()) === before);
  m.redo();
  check("setProp redo re-applies", d.read("base", "pos.x") === 2.5);
}
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const m = d.setProp("arm", "scale.y", 2);  // arm has NO scale key — containers get created
  check("setProp creates missing containers", !m.error && d.read("arm", "scale.y") === 2);
  m.undo();
  check("undo deletes created containers (byte-stable)", JSON.stringify(d.serialize()) === before);
}
{
  const d = freshDoc();
  check("setProp refuses a bound target", /bound/.test(d.setProp("arm", "rot.y", 1).error || ""));
  check("setProp refuses wrong type", /number/.test(d.setProp("base", "pos.x", "wat").error || ""));
  check("setProp refuses non-editable path", (d.setProp("knob", "size.x", 1).error || "").length > 0);
}
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const m = d.movePos("puck", { x: 1, y: 0.65, z: -0.4 });
  check("movePos is one mutation", !m.error && d.read("puck", "pos.z") === -0.4);
  m.undo();
  check("movePos undo restores bytes", JSON.stringify(d.serialize()) === before);
}
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const add = d.addNode({ id: "fin", kind: "cone", parent: "base", pos: { x: 0.5, y: 0.9, z: 0 } });
  check("addNode applies", !add.error && d.byId("fin") !== null);
  const rm = d.removeNode("fin");
  check("removeNode applies", !rm.error && d.byId("fin") === null);
  rm.undo(); add.undo();
  check("add/remove undone restores bytes", JSON.stringify(d.serialize()) === before);
  check("removeNode refuses while referenced", /referenced/.test(d.removeNode("arm").error || ""));
  check("addNode refuses duplicate id", /taken/.test(d.addNode({ id: "base", kind: "box" }).error || ""));
}
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const m = d.renameId("knob", "orb");
  check("renameId rewrites bindings and wires", !m.error &&
    d.bindings().some((b) => b.target === "orb.scale.x") && d.wires().some((w) => w.on === "orb.tap") && d.byId("orb").entry.parent === "arm");
  m.undo();
  check("renameId undo restores bytes", JSON.stringify(d.serialize()) === before);
  check("renameId refuses a taken id", /taken/.test(d.renameId("knob", "base").error || ""));
}
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const m = d.setBinding("puck.material.token", "'danger'");
  check("setBinding adds", !m.error && d.boundTargets().has("puck.material.token"));
  const m2 = d.setBinding("puck.material.token", "'glass'", { ms: 100 });
  check("setBinding replaces in place (replace-only)", !m2.error && d.bindings().filter((b) => b.target === "puck.material.token").length === 1);
  m2.undo(); m.undo();
  check("setBinding undo chain restores bytes", JSON.stringify(d.serialize()) === before);
  check("setBinding refuses a bad expr", /expression/.test(d.setBinding("puck.pos.x", "1 +").error || ""));
  check("setBinding refuses a non-bindable path", (d.setBinding("puck.nope", "1").error || "").length > 0);
}
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const a = d.addState({ name: "count", type: "number", value: 0 });
  const w = d.addWire({ on: "knob.tap", do: [{ set: "count", to: "count + 1" }] });
  check("addState + addWire apply", !a.error && !w.error);
  check("removeState refuses while used", /used by/.test(d.removeState("count").error || ""));
  const rw = d.removeWire(d.wires().length - 1);
  const rs = d.removeState("count");
  check("removeWire then removeState succeed", !rw.error && !rs.error);
  rs.undo(); rw.undo(); rw.redo(); rs.redo();
  rs.undo(); rw.undo(); w.undo(); a.undo();
  check("state/wire lifecycle undo restores bytes", JSON.stringify(d.serialize()) === before);
}
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const m = d.setAffordance("lbl", "tap", true);
  check("setAffordance applies", !m.error && d.byId("lbl").entry.affordances.tap === true);
  m.undo();
  check("setAffordance undo restores bytes", JSON.stringify(d.serialize()) === before);
  const m2 = d.setAffordance("knob", "hover", null);
  check("clearing an affordance", !m2.error && !("hover" in d.byId("knob").entry.affordances));
  m2.undo();
  check("clear-affordance undo restores bytes", JSON.stringify(d.serialize()) === before);
}
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const m1 = d.addMount({ id: "gauge", lib: "bench", ctl: "widget", at: "pad" });
  const m2 = d.setMountOverride("gauge", "level", "speed / 3");
  const m3 = d.setMountOverride("gauge", "level", null);
  const m4 = d.removeMount("gauge");
  check("mount lifecycle applies", !m1.error && !m2.error && !m3.error && !m4.error);
  m4.undo(); m3.undo(); m2.undo(); m1.undo();
  check("mount lifecycle undo restores bytes", JSON.stringify(d.serialize()) === before);
}
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const m = d.setEnv("grid", false);
  check("setEnv applies", !m.error && d.env().grid === false);
  m.undo();
  check("setEnv undo restores bytes", JSON.stringify(d.serialize()) === before);
}

// ── 6. composed undo chain (the flowdoc lesson) ─────────────────────────────
{
  const d = freshDoc();
  const before = JSON.stringify(d.serialize());
  const chain = [];
  const push = (m) => { if (m.error) { check(`composed chain step refused: ${m.error}`, false); } else chain.push(m); };
  push(d.addNode({ id: "fin", kind: "cone", parent: "base" }));
  push(d.setProp("fin", "pos.y", 0.9));
  push(d.setProp("fin", "material.token", "danger"));
  push(d.setBinding("fin.rot.z", "t * 2"));
  push(d.addState({ name: "hits", type: "number", value: 0 }));
  push(d.setAffordance("fin", "tap", true));
  push(d.addWire({ on: "fin.tap", do: [{ set: "hits", to: "hits + 1" }] }));
  push(d.renameId("fin", "sail"));
  push(d.movePos("sail", { x: -0.4, y: 1, z: 0.2 }));
  check("composed chain applied clean", d.byId("sail") !== null && d.bindings().some((b) => b.target === "sail.rot.z"));
  const errsNow = d.diagnostics().filter((x) => x.severity === "error");
  check("composed doc still validates clean", errsNow.length === 0, errsNow.map((x) => x.message).join("; "));
  for (let i = chain.length - 1; i >= 0; i--) chain[i].undo();
  check("composed undo chain restores EXACT bytes", JSON.stringify(d.serialize()) === before);
  for (const m of chain) m.redo();
  check("composed redo chain re-applies", d.byId("sail") !== null && d.read("sail", "pos.y") === 1);
  for (let i = chain.length - 1; i >= 0; i--) chain[i].undo();
  check("undo after redo restores EXACT bytes again", JSON.stringify(d.serialize()) === before);
}

// ── 7. odds and ends ────────────────────────────────────────────────────────
{
  const d = freshDoc();
  check("newId avoids collisions", d.newId("knob") === "knob1" && d.newId("zZ--") !== "");
  check("referencesTo lists the delete preview", d.referencesTo("arm").some((r) => /knob/.test(r)));
  const d2 = parseScene({});
  check("empty doc: accessors are safe, not animated", d2.nodes().length === 0 && d2.animated() === false && d2.diagnostics().filter((x) => x.severity === "error").length === 0);
}

// ── 8. projection ───────────────────────────────────────────────────────────
{
  const d = freshDoc();
  const specs = project(d, { theme: "light" });
  const byId = new Map(specs.map((s) => [s.id, s]));
  check("projection: one spec per node", byId.size === d.nodes().length && byId.has("base") && byId.has("pad"));
  check("projection: parents resolve", byId.get("knob").parent === "arm" && byId.get("base").parent === null);
  check("projection: params + material resolved", byId.get("base").params.size.x === 2 && byId.get("base").material.color === "#e8e4dc");
  check("projection: text spec", byId.get("lbl").text.text === "specimen" && byId.get("lbl").params === undefined);
  check("projection: prefix + parent compose", project(d, { prefix: "g:", parent: "g" })[0].id === "g:base" && project(d, { prefix: "g:", parent: "g" })[0].parent === "g");
  const computed = new Map([["knob.scale.x", 1.4], ["knob.material.token", "good"]]);
  const cs = project(d, { computed, theme: "light" });
  const knob = cs.find((s) => s.id === "knob");
  check("projection: computed overlays doc values", knob.scale.x === 1.4 && knob.material.color === "#4FAE6E");
  check("envOf defaults", envOf(d).lights === "default" && envOf(d).grid === true);
  const df = deltaFor(d, "knob", "material.token", computed, "light");
  check("deltaFor: material re-resolves", df.path === "material" && df.value.color === "#4FAE6E");
  check("deltaFor: transform passthrough", deltaFor(d, "base", "pos.x", null, "light").value === 0);
  check("deltaFor: geometry param flagged", deltaFor(d, "base", "size.x", null, "light").geometry === true);
}

// ── 9. runtime (stub stage + manual clock) ──────────────────────────────────
function stubStage() {
  return {
    scenes: [], deltas: [], renders: 0,
    setScene(specs, env) { this.scenes.push({ specs, env }); },
    applyDelta(id, path, value) { this.deltas.push({ id, path, value }); },
    requestRender() { this.renders++; },
    last(id, path) { for (let i = this.deltas.length - 1; i >= 0; i--) { const d = this.deltas[i]; if (d.id === id && d.path === path) return d; } return null; },
  };
}
{
  let clock = 1000;
  const stage = stubStage();
  const rt = createRuntime({ doc: freshDoc(), stage, theme: "light", reduced: true, now: () => clock });
  await rt.start();
  const specs = stage.scenes[0].specs;
  const knob = specs.find((s) => s.id === "knob");
  check("runtime boot: scene pushed with computed bindings", stage.scenes.length === 1 && knob.scale.x === 1);
  check("runtime boot: text binding computed", specs.find((s) => s.id === "lbl").text.text === "specimen");
  rt.handleTap("knob");
  check("tap wire flips state", rt.stateOf().open === true);
  check("tap → scale delta (reduced: snapped)", stage.last("knob", "scale.x")?.value === 1.4);
  check("tap → material delta re-resolved", stage.last("knob", "material")?.value.color === "#4FAE6E");
  check("tap → label delta", stage.last("lbl", "text")?.value === "specimen · open");
  rt.handleDrag({ type: "move", id: "puck", x: 1.5, y: 0, z: 0, dx: 0.5, dy: 0, dz: 0 });
  check("drag wire uses locals", rt.stateOf().speed === 2.5);
  clock = 3000;
  rt.step();
  const rot = stage.last("arm", "rot.y");
  check("t clock drives animated bindings", rot && Math.abs(rot.value - 2 * 2.5) < 1e-9);
  rt.dispose();
}
{
  // easing with a manual clock (not reduced)
  let clock = 0;
  const stage = stubStage();
  const rt = createRuntime({ doc: freshDoc(), stage, theme: "light", reduced: false, now: () => clock });
  await rt.start();
  rt.pause(); // keep the clock manual — no timers in node
  rt.resume;  // (not resumed: we drive step() by hand)
  rt._running = true;
  rt.handleTap("knob");
  check("ease: no snap on change", stage.last("knob", "scale.x") === null);
  clock = 80; rt.step();
  const mid = stage.last("knob", "scale.x");
  check("ease: mid-tween value", mid && mid.value > 1 && mid.value < 1.4);
  clock = 300; rt.step();
  check("ease: lands exactly", stage.last("knob", "scale.x").value === 1.4);
  rt.dispose();
}
{
  // mounts: props-down, events-up, prefixing, cycles
  const parent = parseScene({
    state: [{ name: "level", type: "number", value: 0.5 }, { name: "pings", type: "number", value: 0 }],
    nodes: [{ id: "root", kind: "group" }, { id: "pad", kind: "slot", parent: "root" }],
    mounts: [{ id: "gauge", lib: "bench", ctl: "meter", at: "pad", state: { fill: "level * 2" } }],
    wires: [{ on: "gauge.ping", do: [{ set: "pings", to: "pings + n" }] }],
  });
  const child = parseScene({
    state: [{ name: "fill", type: "number", value: 0 }],
    nodes: [{ id: "bar", kind: "box", affordances: { tap: true } }],
    bindings: [{ target: "bar.scale.y", expr: "fill" }],
    wires: [{ on: "bar.tap", do: [{ emit: "ping", with: { n: "2" } }] }],
  });
  const stage = stubStage();
  const diags = [];
  const rt = createRuntime({ doc: parent, stage, theme: "light", reduced: true, now: () => 0,
    loadDoc: async (lib, ctl) => (lib === "bench" && ctl === "meter" ? child : null),
    onDiag: (m) => diags.push(m) });
  await rt.start();
  const specs = stage.scenes[0].specs;
  check("mount: child specs are prefixed under the mountpoint",
    specs.some((s) => s.id === "gauge" && s.kind === "mountpoint" && s.parent === "pad")
    && specs.some((s) => s.id === "gauge:bar" && s.parent === "gauge"));
  check("mount: props-down override evaluated at boot", specs.find((s) => s.id === "gauge:bar").scale.y === 1);
  rt.setState("level", 1.5);
  check("mount: props-down reacts to parent state", stage.last("gauge:bar", "scale.y")?.value === 3);
  rt.handleTap("gauge:bar");
  check("mount: events-up with payload", rt.stateOf().pings === 2);
  rt.dispose();
  check("mount: no SD-9 diags in the clean case", diags.length === 0, diags.join("; "));
}
{
  // self-mount cycle → SD-9 diagnostic + placeholder (no child instance)
  const selfdoc = parseScene({ nodes: [{ id: "n", kind: "box" }], mounts: [{ id: "again", lib: "l", ctl: "c" }] });
  const stage = stubStage();
  const diags = [];
  const rt = createRuntime({ doc: selfdoc, stage, theme: "light", reduced: true, now: () => 0,
    loadDoc: async () => selfdoc, onDiag: (m) => diags.push(m) });
  await rt.start();
  check("mount cycle → SD-9 diagnostic", diags.some((m) => /SD-9/.test(m)));
  rt.dispose();
}
{
  // invoke: gated absence → diag; present → then-steps see result
  const doc = parseScene({
    state: [{ name: "out", type: "string", value: "" }],
    nodes: [{ id: "b", kind: "box", affordances: { tap: true } }],
    wires: [{ on: "b.tap", do: [{ invoke: "lib.ctl.cmd", args: { q: "'hi'" }, then: [{ set: "out", to: "result" }] }] }],
  });
  const diags = [];
  const rt1 = createRuntime({ doc, stage: stubStage(), reduced: true, now: () => 0, onDiag: (m) => diags.push(m) });
  await rt1.start();
  rt1.handleTap("b");
  check("invoke without the gate → honest diag", diags.some((m) => /writable live/.test(m)));
  rt1.dispose();
  const rt2 = createRuntime({ doc, stage: stubStage(), reduced: true, now: () => 0,
    invoke: async (lib, ctl, cmd, args) => `${lib}.${ctl}.${cmd}:${args.q}` });
  await rt2.start();
  rt2.handleTap("b");
  await new Promise((r) => setTimeout(r, 0));
  check("invoke result flows into then-set", rt2.stateOf().out === "lib.ctl.cmd:hi");
  rt2.dispose();
}

// ── 11. links + collections (SC-6/SC-7, design §2.2/§2.8a) ─────────────────
check("member access: item.name", (() => { const p = parseExpr("item.displayname"); return p.ok && evalExpr(p.ast, { item: { displayname: "dagrun" } }).value === "dagrun" && p.deps.has("item") && !p.deps.has("displayname"); })());
check("member access: missing key → null, non-object → soft error",
  ev("item.ghost", { item: {} }).value === null && ev("n.x", { n: 4 }).ok === false);
check("member access composes in expressions", val("'orb:' + item.a", { item: { a: "hugin" } }) === "orb:hugin");
{
  const d = parseScene({ nodes: [
    { id: "a", kind: "box" }, { id: "b", kind: "box" },
    { id: "l", kind: "link", from: "a", to: "b", material: { token: "wire" } }] });
  check("static link validates clean", d.diagnostics().filter((x) => x.severity === "error").length === 0);
  const specs = project(d);
  const l = specs.find((s) => s.id === "l");
  check("link projects with endpoints + material", l && l.from === "a" && l.to === "b" && l.material.color === "#7d8aa0");
  check("removeNode refuses a link endpoint", /link l/.test(d.removeNode("a").error || ""));
}
{
  const d = parseScene({ nodes: [{ id: "l", kind: "link", from: "a", to: "ghost" }] });
  check("SD-12 unresolvable static link endpoints", d.diagnostics().filter((x) => x.code === DIAG.SD12).length === 2);
}
{
  const bad = parseScene({
    state: [{ name: "list", type: "string", value: "" }],
    nodes: [{ id: "e", kind: "box", each: "nope", props: { "size.q": "1", "pos.x": "item.x + ghost" } }],
    bindings: [{ target: "e.pos.x", expr: "1" }] });
  const diags = bad.diagnostics();
  check("SD-11: unknown each field, bad props path, bad identifier",
    diags.some((x) => x.code === DIAG.SD11 && /unknown state field/.test(x.message))
    && diags.some((x) => x.code === DIAG.SD11 && /not bindable/.test(x.message))
    && diags.some((x) => x.code === DIAG.SD6 && /ghost/.test(x.message)));
  check("SD-5: bindings may not target an each-template", diags.some((x) => x.code === DIAG.SD5 && /each-template/.test(x.message)));
}
const peerText = readFileSync(resolve(ROOT, "harness/fixtures/scene_peer.json"), "utf8");
{
  const d = parseScene(JSON.parse(peerText));
  const errs = d.diagnostics().filter((x) => x.severity === "error");
  check("peer specimen validates clean", errs.length === 0, errs.map((x) => `${x.path}: ${x.message}`).join("; "));
  check("peer specimen round-trips byte-identical", JSON.stringify(d.serialize()) === JSON.stringify(JSON.parse(peerText)));
  check("templates skipped in runtime projection", project(d).length === 0);
  check("templates ghost in the editor's build view", project(d, { templates: "gizmo" }).length === 3);
  check("removeState refuses an each source", /each on/.test(d.removeState("peers").error || ""));
  const ov = new Map([["orb.pos.x", 2.4], ["orb.material.token", "good"]]);
  const spec = instanceSpec(d, d.byId("orb").entry, "node", "freyja", ov, {});
  check("instanceSpec: id, overrides, template statics", spec.id === "orb:freyja" && spec.pos.x === 2.4 && spec.material.color === "#4FAE6E" && spec.params.radius === 0.5);
}
function stubStage2() {
  const s = stubStage();
  s.objects = new Map();
  s.upserts = 0; s.removed = [];
  s.upsert = (spec) => { s.objects.set(spec.id, spec); s.upserts++; };
  s.removeObject = (id) => { s.objects.delete(id); s.removed.push(id); };
  return s;
}
{
  let clock = 0;
  const stage = stubStage2();
  const diags = [];
  const rt = createRuntime({ doc: parseScene(JSON.parse(peerText)), stage, theme: "light",
    reduced: true, now: () => clock, onDiag: (m) => diags.push(m) });
  await rt.start();
  check("collections materialize: 4 orbs + 4 tags + 3 links", stage.objects.size === 11, `${stage.objects.size}: ${[...stage.objects.keys()].join(",")}`);
  const orb = stage.objects.get("orb:freyja");
  check("instance props applied (pos + token)", orb && orb.pos.x === 2.4 && orb.material.color === "#62BD8C");
  check("self peer gets the good token", stage.objects.get("orb:dagrun").material.color === "#4FAE6E");
  const line = stage.objects.get("line:c2");
  check("each-link endpoints from item fields", line && line.from === "orb:dagrun" && line.to === "orb:hugin");
  check("labels carry item text", stage.objects.get("tag:munin").text.text === "munin");
  // tap an instance → template wire with item locals → focus → token delta
  rt.handleTap("orb:freyja");
  check("collection tap: item locals reach the wire", rt.stateOf().focus === "freyja");
  check("focus re-syncs dependent props (agent token)", stage.last("orb:freyja", "material")?.value.color === "#D96BA0");
  // keyed diffing: move one, drop one, add one
  const peers = rt.stateOf().peers.filter((p) => p.id !== "munin").map((p) => p.id === "hugin" ? { ...p, x: -4 } : p);
  peers.push({ id: "skadi", displayname: "skadi", x: 3, z: -3 });
  rt.setState("peers", peers);
  await new Promise((r) => setTimeout(r, 0));
  check("diff: departed peer + its tag removed", stage.removed.includes("orb:munin") && stage.removed.includes("tag:munin"));
  check("diff: new peer spawned", stage.objects.has("orb:skadi") && stage.objects.get("tag:skadi").text.text === "skadi");
  check("diff: kept peer moved (reduced: snapped delta)", stage.last("orb:hugin", "pos.x")?.value === -4);
  // t-driven props tick through the clock
  clock = 1000;
  rt.step();
  check("t-driven instance props (spin)", Math.abs((stage.last("orb:skadi", "rot.y")?.value ?? 0) - 0.4) < 1e-9);
  check("no runtime diags on the clean specimen", diags.length === 0, diags.join("; "));
  rt.dispose();
}
{
  // per-instance EASE with a manual clock (not reduced)
  let clock = 0;
  const stage = stubStage2();
  const rt = createRuntime({ doc: parseScene(JSON.parse(peerText)), stage, theme: "light",
    reduced: false, now: () => clock });
  await rt.start();
  rt._running = true;
  const moved = rt.stateOf().peers.map((p) => p.id === "freyja" ? { ...p, x: 5 } : p);
  rt.setState("peers", moved);
  await new Promise((r) => setTimeout(r, 0));
  clock = 150; rt.step();
  const mid = stage.last("orb:freyja", "pos.x");
  check("collection ease: mid-tween between 2.4 and 5", mid && mid.value > 2.4 && mid.value < 5, String(mid && mid.value));
  clock = 400; rt.step();
  check("collection ease: lands exactly", stage.last("orb:freyja", "pos.x").value === 5);
  rt.dispose();
}
{
  // each-MOUNTS: per-item props-down + events-up with the instance key
  const parent = parseScene({
    state: [
      { name: "gauges", type: "json", "value": [
        { id: "cpu", level: 0.3 }, { id: "mem", level: 0.9 }] },
      { name: "pokes", type: "json", value: [] },
      { name: "boost", type: "number", value: 0 },
      { name: "lastpoke", type: "string", value: "" }],
    nodes: [{ id: "rack", kind: "group" }],
    mounts: [{ id: "dial", each: "gauges", lib: "bench", ctl: "meter", at: "rack",
      props: { "pos.x": "index * 2" },
      state: { fill: "item.level", mark: "boost" } }],
    wires: [{ on: "dial.ping", do: [{ set: "lastpoke", to: "key" }] }],
  });
  const child = parseScene({
    state: [{ name: "fill", type: "number", value: 0 },
            { name: "mark", type: "number", value: 0 }],
    nodes: [{ id: "bar", kind: "box", affordances: { tap: true } }],
    bindings: [{ target: "bar.scale.y", expr: "fill" }, { target: "bar.pos.y", expr: "mark" }],
    wires: [{ on: "bar.tap", do: [{ emit: "ping" }] }],
  });
  const stage = stubStage2();
  const rtDiags = [];
  const rt = createRuntime({ doc: parent, stage, theme: "light", reduced: true, now: () => 0,
    onDiag: (m) => rtDiags.push(m),
    loadDoc: async (l, c) => (l === "bench" && c === "meter" ? child : null) });
  await rt.start();
  check("each-mount: mountpoints placed by props", stage.objects.get("dial:cpu")?.pos.x === 0 && stage.objects.get("dial:mem")?.pos.x === 2);
  check("each-mount: child scenes spawned under mountpoints", stage.objects.get("dial:mem:bar")?.parent === "dial:mem");
  check("each-mount: per-item props-down", stage.objects.get("dial:mem:bar")?.scale.y === 0.9);
  rt.handleTap("dial:cpu:bar");
  check("each-mount: events-up carry the instance key", rt.stateOf().lastpoke === "cpu");
  // the focus-reactivity fix: a NON-item dependency of the per-item
  // props-down changes → children update immediately, no collection sync
  // (the peer bug: tap→focused waited for the sleeping feed's next push)
  rt.setState("boost", 2);
  check("each-mount: non-item override dep reaches children WITHOUT a sync",
    stage.last("dial:cpu:bar", "pos.y")?.value === 2 && stage.last("dial:mem:bar", "pos.y")?.value === 2);
  check("each-mount: no spurious item diagnostics", rtDiags.length === 0, rtDiags.join("; "));
  const gauges = rt.stateOf().gauges.filter((g) => g.id !== "cpu");
  rt.setState("gauges", gauges);
  await new Promise((r) => setTimeout(r, 0));
  check("each-mount: departed instance fully disposed", stage.removed.includes("dial:cpu") && stage.removed.includes("dial:cpu:bar"));
  rt.dispose();
}
{
  // forcelayout: pure, deterministic, settles — and the molecular
  // invariants: substepping (no tunneling) + separation projection
  let items = [{ id: "a", anchored: true }, { id: "b" }, { id: "c" }];
  const cons = [{ a: "a", b: "b" }];
  for (let i = 0; i < 100; i++) items = forceStep(items, cons, 0.1);
  const [a, b, c] = items;
  const dAB = Math.hypot(a.x - b.x, a.z - b.z);
  const dAC = Math.hypot(a.x - c.x, a.z - c.z);
  check("forcelayout: connected pair sits nearer than the stranger", dAB < dAC, `dAB=${dAB.toFixed(2)} dAC=${dAC.toFixed(2)}`);
  check("forcelayout: settles (energy under epsilon)", energy(items) < 0.01, String(energy(items)));
  check("forcelayout: pure (inputs untouched)", items !== forceStep(items, cons, 0.1) && typeof items[0].x === "number");
  check("forcelayout: positions bounded", items.every((it) => Math.abs(it.x) <= 8 && Math.abs(it.z) <= 8));
  const minSep = (list) => {
    let m = Infinity;
    for (let i = 0; i < list.length; i++) for (let j = i + 1; j < list.length; j++)
      m = Math.min(m, Math.hypot(list[i].x - list[j].x, list[i].z - list[j].z));
    return m;
  };
  check("forcelayout: settled separation honors dMin", minSep(items) >= 1.2 - 1e-9, String(minSep(items)));
  // projection: spawned overlapping → separated after ONE step
  let tight = [{ id: "p", x: 0, z: 0 }, { id: "q", x: 0.05, z: 0 }, { id: "r", x: 0, z: 0.03 }];
  tight = forceStep(tight, [], 0.1);
  check("projection: overlapping spawns separate in one step", minSep(tight) >= 1.2 - 1e-9, String(minSep(tight)));
  // tunneling: high closing speed cannot pass through the barrier
  let fast = [{ id: "p", x: -3, z: 0, vx: 6, vz: 0 }, { id: "q", x: 3, z: 0, vx: -6, vz: 0 }];
  let worst = Infinity;
  for (let i = 0; i < 30; i++) { fast = forceStep(fast, [], 0.1); worst = Math.min(worst, minSep(fast)); }
  check("no tunneling: closest approach at speed stays >= dMin", worst >= 1.2 - 1e-9, String(worst));
  // a crushing spring (restLength 0) still cannot defeat the invariant
  let crush = [{ id: "p", x: -2, z: 0 }, { id: "q", x: 2, z: 0 }];
  for (let i = 0; i < 60; i++) crush = forceStep(crush, [{ a: "p", b: "q" }], 0.1, { restLength: 0, spring: 8 });
  check("projection beats a crushing spring", minSep(crush) >= 1.2 - 1e-9, String(minSep(crush)));
  // determinism: same inputs, same outputs
  const d1 = forceStep([{ id: "a" }, { id: "b" }], cons, 0.1);
  const d2 = forceStep([{ id: "a" }, { id: "b" }], cons, 0.1);
  check("forcelayout: deterministic", JSON.stringify(d1) === JSON.stringify(d2));
}

// ── 10. boundary greps (design acceptances 1–2) ─────────────────────────────
{
  const { execSync } = await import("node:child_process");
  const grep = (pattern, path) => {
    try { return execSync(`grep -rln ${JSON.stringify(pattern)} ${path}`, { cwd: ROOT }).toString().trim(); }
    catch { return ""; }
  };
  const threeLeaks = grep("three.module.js", "assets/ controls/");
  check("THREE imported only in vendor/nb_three/", threeLeaks === "", threeLeaks);
  const sceneFiles = "assets/sceneexpr.js assets/scenedoc.js assets/sceneproject.js assets/scenerun.js assets/scenetokens.js";
  const evil = grep("new Function(", sceneFiles) + grep("eval(", sceneFiles);
  check("no eval / new Function in scene modules", evil === "", evil);
}

console.log(failed ? `\n${failed} FAILED` : "\nALL CHECKS PASSED");
process.exit(failed ? 1 : 0);
