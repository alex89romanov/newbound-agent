// memory.js — the agent's long-term memory INDEX (memory v0, docs/memory.md).
// The kb library is the memory: each control is a knowledge DOMAIN —
// reference domains (platform-api, workflow, …) plus dated MONTH BUCKETS
// (m2026-07) for episodic history; the type rides naming and tags, not
// structure. A domain's `memory` facet holds a JSON array of entries
// {claim, detail?, tags, source?, confidence, time}. This module
// surfaces the INDEX — names, descs, tags, entry counts, and a STALENESS
// mark (hash-stamped sources re-checked against their referents) — as a
// viewctx fence, so the agent knows what it knows and pulls entries on
// demand through the auto-run read family; writes go through the
// validated `remember` command. The index STALE-WHILE-REVALIDATES: each
// fence snapshot serves the current index and kicks a background refresh
// (cache-busting — remember() writes server-side, so the store's record
// cache must be dropped or revalidation is a no-op), so a remembered
// entry shows in the counts by the NEXT ask, no reload needed.
// dest-agent: rides the agent add-on's own installs (the app boot and
// the bench grafts carry this module) — no part of the dev app names
// the memory system.
//
// LIBRARY control — headless: defines window.NB_MEMORY once (idempotent across
// installs). Consumers list this control as a hidden data-control child
// div and use the global from their ready.

var me = this;
var ME = document.getElementById(me.UUID);

me.ready = function () {
  if (window.NB_MEMORY) return;
  const { viewctx } = window.NB_VIEWCTX;
  const jsonP = (c2, v2) => new Promise((res2) => json(c2, v2, res2));
  const invokeP = (l2, c2, m2, a2) => new Promise((res2) => invokeCommand(l2, c2, m2, a2, res2));
  const code = (m2, a2) => invokeP("dev", "code", m2, a2);
  const readRec = async (l2, id2) => {
    const r2 = await jsonP("../app/read", "lib=" + encodeURIComponent(l2) + "&id=" + encodeURIComponent(id2));
    return r2.status === "ok" ? r2.data : new Error(r2.msg || "read failed");
  };
  const controlsOf = async (l2) => {
    const d2 = await readRec(l2, "controls");
    return d2 instanceof Error ? d2 : (d2.list ?? []);
  };
  const readFacet = (l2, c2, f2) => code("read_control_facet", { lib: l2, ctl: c2, facet: f2 });
  window.NB_MEMORY = (function () {

let index = null;       // [{name, desc, tags, entries, stale}]
let refreshing = null;  // the in-flight refresh promise, single-flight
let refreshedAt = 0;

async function refresh() {
  const controls = await controlsOf("kb");
  if (controls instanceof Error || controls.length === 0) { index = null; return; }
  const hashes = new Map();          // "lib:ctl:facet" -> current hash | null
  async function referentHash(s) {
    const key = `${s.lib}:${s.ctl}:${s.facet}`;
    if (!hashes.has(key)) {
      const r = await readFacet(s.lib, s.ctl, s.facet);
      hashes.set(key, r?.status === "ok" ? r.hash : null);
    }
    return hashes.get(key);
  }
  const out = [];
  for (const c of controls) {
    const rec = await readRec("kb", c.id);
    if (rec instanceof Error) continue;
    let entries = -1;                // -1 = unparsable; shown without count
    let stale = 0;
    let list = null;                 // kept for layer 3's tag-keyed push
    try {
      list = JSON.parse(rec.memory ?? "[]");
      entries = list.length;
      for (const e of list) {
        const s = e?.source;
        if (s?.lib && s?.ctl && s?.facet && s?.hash) {
          const cur = await referentHash(s);
          if (cur !== null && cur !== s.hash) stale += 1;
        }
      }
    } catch { entries = -1; list = null; }
    out.push({ name: c.name, desc: rec.desc ?? "", tags: rec.tags ?? "", entries, stale, list });
  }
  index = out.length ? out : null;
}

function kick() {
  if (!refreshing) {
    refreshing = refresh().catch(() => {}).finally(() => {
      refreshing = null;
      refreshedAt = Date.now();
    });
  }
  return refreshing;
}

// The first index builds as soon as this library installs — the platform
// serves the page, so the connection is simply there.
const ready = kick().catch(() => { index = null; });

viewctx.register("memory", () => {
  if (!refreshing && Date.now() - refreshedAt > 5000) kick();
  if (!index) return null;
  const lines = index.map((d) => {
    const n = d.entries >= 0
      ? ` (${d.entries}${d.stale ? `, ${d.stale} stale?` : ""})` : "";
    const t = d.tags ? ` [${d.tags}]` : "";
    return `- kb.${d.name}${n}${t} — ${d.desc || "no desc"}`;
  });
  // Recall layer 3 (docs/prompting.md): entries TAGGED for the surface
  // open on the bench are procedurally relevant — push them, capped,
  // instead of waiting for the model to pull. The workbench provider
  // names the open lib/ctl; the tag vocabulary rule is `lib.ctl`.
  let pushedBlock = "";
  const wb = viewctx.peek("workbench");
  const openTok = wb?.fields?.lib && wb?.fields?.ctl
    ? `${wb.fields.lib}.${wb.fields.ctl}` : null;
  if (openTok) {
    const hits = [];
    for (const d of index) {
      for (const e of d.list ?? []) {
        const toks = (e.tags ?? "").split(",").map((x) => x.trim());
        if (toks.includes(openTok)) hits.push({ dom: d.name, e });
      }
    }
    if (hits.length) {
      const shown = hits.slice(0, 5).map(({ dom, e }) =>
        `- [kb.${dom}] ${e.claim}${e.detail ? " — " + String(e.detail).slice(0, 200) : ""}`);
      pushedBlock = `\nRelevant now (entries tagged ${openTok}):\n`
        + shown.join("\n")
        + (hits.length > 5 ? `\n(+${hits.length - 5} more — read the domains)` : "");
    }
  }
  return {
    label: `memory · ${index.length} domain${index.length === 1 ? "" : "s"}`,
    fields: {
      memory: "Long-term memory domains (the kb library; m-prefixed buckets "
        + "are dated episodic history; \"N stale?\" = that many entries' "
        + "hash-stamped sources changed since learning — re-read the "
        + "referent before trusting them; read entries with "
        + "read_control_facet lib:\"kb\" ctl:<domain> facet:\"memory\"; "
        + "write with remember):\n" + lines.join("\n") + pushedBlock,
    },
  };
});

// ── recall layer 4: mode-keyed packs (docs/prompting.md) ──────────────
// The ask's MODE is procedurally derivable (doctrine: never waste LLM
// cycles on what a procedure can answer), and pre-loading what the mode
// probably needs beats fetching it later (doctrine: preload > fetch).
// Tags on entries stay TOPICAL and truthful; modes are selector-side
// views over them, so evolving the mode vocabulary retags nothing.
const MODES = [
  ["author-code", /\b(command|rust|python|compile|code|function|impl\b|import|snippet|body|return ?type|param)/i,
    ["rust", "ndata", "flowlang", "threading", "build", "dependencies", "imports", "procedure", "code", "llm"]],
  ["edit-ui", /\b(ui|button|link|panel|page|css|html|style|colou?r|label|title|header|font|layout|render|display|screen|chrome|editor|shelf|workbench|scene)/i,
    ["frontend", "ui", "css", "html", "workbench", "facet", "scene"]],
  ["build", /\b(build|create|add|new|make|app\b|control|library|feature|tool|skill|design)/i,
    ["architecture", "philosophy", "p2p", "dependencies", "capability", "module", "scratch", "skills"]],
  ["diagnose", /\b(error|fail|broken|bug|panic|crash|why|wrong|debug|fix|doesn|not work|stuck)/i,
    ["defect", "workaround", "method", "process", "flowlang", "mcp", "discovery"]],
  ["operate", /\b(publish|install|deploy|restart|peer|user|group|permission|timer|event|memory|remember|tags?\b|plugin|desc)/i,
    ["workflow", "process", "mcp", "discovery", "desc", "memory", "p2p", "dogfood", "runtime", "config"]],
];

function modesFor(text) {
  const t = String(text || "");
  return MODES.filter(([, re]) => re.test(t)).map(([name]) => name);
}

/** The mode-keyed pack for one ask: kb entries whose tags (or whose
    domain's tags) intersect the matched modes' tag sets. Null when no
    mode matches, nothing hits, or memory isn't live. Capped, overflow
    counted — the model is told where the rest lives. */
function packFor(text, cap = 12) {
  if (!index) return null;
  const modes = modesFor(text);
  if (!modes.length) return null;
  const want = new Set();
  for (const [name, , tags] of MODES) {
    if (modes.includes(name)) for (const t of tags) want.add(t);
  }
  const toks = (s) => String(s || "").split(",").map((x) => x.trim().toLowerCase()).filter(Boolean);
  const hits = [];
  for (const d of index) {
    const domHit = toks(d.tags).some((t) => want.has(t));
    for (const e of d.list ?? []) {
      if (domHit || toks(e.tags).some((t) => want.has(t))) hits.push({ dom: d.name, e });
    }
  }
  if (!hits.length) return null;
  const shown = hits.slice(0, cap).map(({ dom, e }) =>
    `- [kb.${dom}] ${e.claim}` + (e.detail ? ` — ${String(e.detail).slice(0, 200)}` : ""));
  const over = hits.length > cap
    ? `\n(+${hits.length - cap} more — read the domains with read_control_facet)` : "";
  return { modes, count: Math.min(hits.length, cap), total: hits.length,
    block: "```memory:pack (modes: " + modes.join(", ") + ")\n" + shown.join("\n") + over + "\n```" };
}

    return { refresh, ready, modesFor, packFor };
  })();
};
