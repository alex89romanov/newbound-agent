// chat — the Agent app's UI (docs/agent-app.md). A standalone venue for
// the same machinery the notebook's ask row drives: agentloop's chatTurn
// over chat_llm, the full tool model (auto-run reads, typed confirm for
// mutating calls, meta-tools), the store-resident curriculum core, and
// the memory fence. Plus the surfaces the IDE doesn't have: the MEMORY
// INSPECTOR (domains, entries, stale checks, the owner's audit — bless /
// edit / forget — and remember-by-form), the PROMPTS editors (core /
// addendum / archivist — all journaled facet patches), and the ARCHIVIST
// strip (queue depth + sweep now).
import { store } from "../../assets/store.js";
import { viewctx } from "../../assets/viewctx.js";
import * as agent from "../../assets/agentloop.js";
import { ADDENDUM } from "../../assets/agentprompt.js";
import "../../assets/memory.js";   // registers the memory fence provider

const CHAT_SHELL = `Right now you are speaking through the Newbound Agent app — a standalone chat. There is no IDE around you; every tool call you make renders as a visible cell in this thread, and mutating commands stop for the user's typed confirmation.

THIS APP'S OWN UI
- This chat interface IS a Newbound control: lib "agent", control "chat" (html/css/js facets); the page boot is "agent.agent". When the user asks you to change this app's UI, go straight there: dev-code-read_control_facet on the right agent.chat facet, then ONE minimal exact-match dev-code-patch_control_facet (base = the hash you just read). No searching needed; a reload shows the change.

WHAT YOU SEE
- The context may include a memory:index fence listing your long-term memory domains ("N stale?" = hash-stamped sources changed since learning — re-read those referents before trusting the marked entries).

MEMORY IN THIS CHAT
- Memory FORMATION is automatic: the archivist reviews completed sessions in the background. Do not file memories on your own initiative. When the user explicitly asks you to remember something, call dev-code-remember — it runs without confirmation, because the request itself is the authorization.

HOW TO ANSWER
- Be concise and concrete. If you are unsure, say so plainly rather than inventing platform behavior.`;

const STORE_KEY = "agent.chat.v1";

export async function init(host) {
  // ── the connection ───────────────────────────────────────────────────
  // The loader's ensureConnected runs before any mounted control's init,
  // with the read-only platform default. The agent app is a WRITE venue by
  // design — the memory audit and the prompt editors are its purpose;
  // every write is journaled and the server enforces real permissions
  // regardless — so reconnect writable. The dev app's SAVED posture
  // (bench.connection) is put back untouched: this page's writability is
  // its own, not a flag quietly flipped on the IDE.
  if (!store.writable()) {
    const savedRaw = localStorage.getItem("bench.connection");
    const r = await store.connect({ mode: "live", baseUrl: location.origin,
      sessionid: "", writable: true });
    if (savedRaw === null) localStorage.removeItem("bench.connection");
    else localStorage.setItem("bench.connection", savedRaw);
    if (r instanceof Error) {
      host.querySelector(".ag-thread").textContent =
        "not connected: " + r.message + " — sign in via the instance and reload.";
      return {};
    }
  }

  // ── tabs ─────────────────────────────────────────────────────────────
  for (const tab of host.querySelectorAll(".ag-tab")) {
    tab.addEventListener("click", () => {
      for (const t of host.querySelectorAll(".ag-tab")) t.classList.toggle("on", t === tab);
      for (const p of host.querySelectorAll(".ag-pane")) p.hidden = p.dataset.pane !== tab.dataset.tab;
      if (tab.dataset.tab === "memory") loadDomains();
      if (tab.dataset.tab === "prompts") loadPrompts();
    });
  }

  // ── the archivist strip ──────────────────────────────────────────────
  const archQ = host.querySelector(".ag-arch-q");
  async function refreshArchivist() {
    const r = await store.invoke("agent", "archivist", "queue_status", {});
    if (r instanceof Error || r.envelope.status !== "ok") { archQ.textContent = ""; return; }
    const n = r.envelope.queued ?? 0;
    archQ.textContent = `archivist: ${n} turn${n === 1 ? "" : "s"} queued`;
  }
  host.querySelector(".ag-sweep").addEventListener("click", async (ev) => {
    const btn = ev.currentTarget;
    btn.disabled = true;
    btn.textContent = "sweeping…";
    const r = await store.invoke("agent", "archivist", "consolidate", {});
    btn.disabled = false;
    btn.textContent = "sweep now";
    const env = r instanceof Error ? { status: "err", msg: r.message } : r.envelope;
    note(host.querySelector(".ag-arch-note"), env.status === "ok"
      ? `swept ${env.swept ?? 0} · filed ${env.filed ?? 0} · skipped ${env.skipped ?? 0}`
      : `sweep failed: ${env.msg ?? "?"}`, env.status !== "ok");
    await refreshArchivist();
    if (!host.querySelector('[data-pane="memory"]').hidden) loadDomains();
  });
  refreshArchivist();

  // ── chat: transcript + persistence ───────────────────────────────────
  const thread = host.querySelector(".ag-thread");
  let messages = [];       // the raw conversation (openai shapes), persisted
  let transcript = [];     // rendered cells {kind, title?, text, error?}
  try {
    const saved = JSON.parse(localStorage.getItem(STORE_KEY) ?? "null");
    if (saved && Array.isArray(saved.messages)) {
      messages = saved.messages;
      transcript = saved.transcript ?? [];
    }
  } catch { /* fresh */ }
  const persist = () => {
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify({
        messages: messages.slice(-60), transcript: transcript.slice(-120) }));
    } catch { /* storage full — the chat still works */ }
  };

  function cellEl(entry) {
    const div = document.createElement("div");
    div.className = "ag-cell ag-" + entry.kind + (entry.error ? " err" : "");
    if (entry.title) {
      const h = document.createElement("div");
      h.className = "ag-cell-title";
      h.textContent = entry.title;
      div.appendChild(h);
    }
    const body = document.createElement("div");
    body.className = "ag-cell-body";
    renderRich(body, entry.text ?? "");
    div.appendChild(body);
    return div;
  }

  /** think-folds + fenced code; everything else stays text. */
  function renderRich(el, raw) {
    let text = String(raw);
    const think = text.match(/<think>([\s\S]*?)<\/think>/);
    if (think) {
      const det = document.createElement("details");
      det.className = "ag-think";
      const sum = document.createElement("summary");
      sum.textContent = "thinking…";
      det.append(sum, document.createTextNode(think[1].trim()));
      el.appendChild(det);
      text = text.replace(think[0], "").trim();
    }
    const parts = text.split(/```([\s\S]*?)```/);
    parts.forEach((part, i) => {
      if (!part.trim()) return;
      if (i % 2 === 1) {
        const pre = document.createElement("pre");
        pre.textContent = part.replace(/^[a-z:.\-\d]*\n/i, "");
        el.appendChild(pre);
      } else {
        const p = document.createElement("div");
        p.textContent = part.trim();
        el.appendChild(p);
      }
    });
  }

  function pushCell(entry) {
    transcript.push(entry);
    thread.appendChild(cellEl(entry));
    thread.scrollTop = thread.scrollHeight;
    persist();
    return entry;
  }
  for (const entry of transcript) thread.appendChild(cellEl(entry));
  thread.scrollTop = thread.scrollHeight;

  // new session — clears THIS BROWSER's transcript (the model context and
  // the rendered cells). Two-click like every destructive act here; memory,
  // journals, and the archivist queue are untouched.
  host.querySelector(".ag-newsession").addEventListener("click", (ev) => {
    const btn = ev.currentTarget;
    if (btn.textContent !== "really clear?") {
      btn.textContent = "really clear?";
      setTimeout(() => { btn.textContent = "new session"; }, 2500);
      return;
    }
    btn.textContent = "new session";
    messages = [];
    transcript = [];
    try { localStorage.removeItem(STORE_KEY); } catch { /* fine */ }
    thread.innerHTML = "";
  });

  // ── the tool model (the notebook's ceremony, this venue's chrome) ────
  const confirmEl = host.querySelector(".ag-confirm");
  const askConfirmed = new Set();
  function askConfirm(cmdName, lite) {
    return new Promise((done) => {
      confirmEl.hidden = false;
      confirmEl.querySelector(".ag-confirm-msg").textContent = lite
        ? `run ${cmdName} again?`
        : `the agent wants to run ${cmdName} — type the command name to allow it:`;
      const input = confirmEl.querySelector(".ag-confirm-input");
      const okBtn = confirmEl.querySelector(".ag-confirm-ok");
      input.hidden = lite;
      input.value = "";
      const finish = (v) => { confirmEl.hidden = true; done(v); };
      okBtn.hidden = !lite;
      okBtn.onclick = () => finish(true);
      input.onkeydown = (ev) => {
        if (ev.key === "Enter") finish(input.value.trim() === cmdName);
        if (ev.key === "Escape") finish(false);
      };
      confirmEl.querySelector(".ag-confirm-cancel").onclick = () => finish(false);
      if (!lite) input.focus();
    });
  }

  let mcpTools = null;
  async function ensureCatalog() {
    if (mcpTools !== null) return mcpTools;
    const r = await agent.listTools();
    mcpTools = r.status === "ok" ? (r.tools ?? []) : [];
    return mcpTools;
  }

  const toolCell = (title, args, out) => pushCell({
    kind: "tool", error: !!out.error,
    title: title + " " + agent.clamp(JSON.stringify(args ?? {}), 160),
    text: agent.clamp(out.output ?? "", 1200),
  });

  async function execTool(call) {
    let args = {};
    try { args = call.arguments ? JSON.parse(call.arguments) : {}; } catch { /* raw below */ }

    if (call.name === "find_tools") {
      const catalog = await ensureCatalog();
      const hits = agent.searchCatalog(catalog, args.query ?? "");
      toolCell("✳ find_tools", args, { output: hits.length
        ? hits.map((h) => `${h.name} [${h.gate}] — ${h.summary}`).join("\n") : "no matches" });
      return hits.length ? JSON.stringify(hits)
        : "No commands matched. Try different keywords, or list_commands on a likely control.";
    }
    if (call.name === "describe_tool") {
      const catalog = await ensureCatalog();
      const entry = catalog.find((t) => t.name === args.name);
      toolCell("✳ describe_tool", args, {
        output: entry ? `${entry.name} [${agent.gateFor(entry.name, entry)}]` : "unknown tool",
        error: !entry });
      return entry ? JSON.stringify({ ...entry, gate: agent.gateFor(entry.name, entry) })
        : `Unknown tool "${args.name}" — use find_tools to search the catalog.`;
    }
    let name = call.name;
    if (call.name === "call_command") {
      name = args.name;
      args = (args.args && typeof args.args === "object") ? args.args : {};
      const catalog = await ensureCatalog();
      const entry = catalog.find((t) => t.name === name);
      if (!entry) {
        toolCell("✳ call_command", { name }, { error: true, output: "unknown tool" });
        return `Unknown tool "${name}" — use find_tools to search the catalog.`;
      }
      const complaints = agent.schemaComplaints(entry.inputSchema, args);
      if (complaints) {
        toolCell("✳ call_command", { name, args }, { error: true,
          output: `arguments rejected: ${complaints}` });
        return `ARGUMENTS REJECTED for ${name}: ${complaints}. Fix and call again.`;
      }
    }
    const target = agent.parseToolName(name);
    if (!target) return `ERROR: tool name "${name}" is not lib-ctl-cmd shaped`;
    const callText = `${target.lib}.${target.ctl}.${target.cmd}`;
    const catalog = await ensureCatalog();
    const entry = catalog.find((t) => t.name === name);
    if (agent.gateFor(name, entry) !== "auto") {
      const confirmed = await askConfirm(target.cmd, askConfirmed.has(target.cmd));
      if (!confirmed) {
        toolCell(callText, args, { error: true, output: "denied by the user" });
        return "DENIED: the user declined this action. Continue without it.";
      }
      askConfirmed.add(target.cmd);
    }
    const result = await store.invoke(target.lib, target.ctl, target.cmd, args);
    if (result instanceof Error) {
      toolCell(callText, args, { error: true, output: result.message });
      return `ERROR: ${result.message}`;
    }
    const env = result.envelope;
    const payload = env.status === "ok" ? (env.data ?? env.msg ?? env) : env.msg;
    const text = typeof payload === "string" ? payload : JSON.stringify(payload, null, 1);
    toolCell(callText, args, { error: env.status !== "ok", output: text });
    return env.status === "ok" ? agent.clamp(text, 4000) : `ERROR: ${env.msg}`;
  }

  // ── ask ──────────────────────────────────────────────────────────────
  const form = host.querySelector(".ag-ask");
  const input = host.querySelector(".ag-input");
  const sendBtn = host.querySelector(".ag-send");
  form.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const message = input.value.trim();
    if (!message || sendBtn.disabled) return;
    input.value = "";
    sendBtn.disabled = true;
    pushCell({ kind: "user", text: message });
    const busy = pushCell({ kind: "busy", text: "thinking…" });
    const busyEl = thread.lastElementChild;
    try {
      const core = await agent.corePrompt();
      const catalog = await ensureCatalog();
      const attached = agent.DEFAULT_TOOLS.filter((n) => catalog.some((t) => t.name === n));
      const tools = [...agent.META_TOOL_DEFS, ...agent.toolDefs(catalog, attached)];
      const addendum = (ADDENDUM ?? "").trim();
      if (messages.length === 0 || messages[0].role !== "system") {
        messages.unshift({ role: "system", content: "" });
      }
      messages[0].content = core + "\n\n" + CHAT_SHELL + agent.TOOLS_PROMPT +
        (addendum ? "\n\nOWNER ADDENDUM\n" + addendum : "");
      const ctx = agent.contextBlock(viewctx.snapshot());
      if (ctx) {
        messages.push({ role: "user", content: "[CONTEXT]\n\n" + ctx },
                      { role: "user", content: message });
      } else {
        messages.push({ role: "user", content: message });
      }
      const text = await agent.chatTurn({
        messages, tools, execTool,
        onRound: () => { busyEl.querySelector(".ag-cell-body").textContent = "agent is using tools…"; },
      });
      messages.push({ role: "assistant", content: text });
      busyEl.remove();
      transcript.splice(transcript.indexOf(busy), 1);
      pushCell({ kind: "agent", text });
      store.invoke("agent", "archivist", "log_turn", {
        venue: "agent-app", ask: message.slice(0, 4000),
        reply: (text ?? "").slice(0, 4000), tools: "", author: "agent-app",
      }).catch(() => {});
      refreshArchivist();
    } catch (e) {
      busyEl.remove();
      transcript.splice(transcript.indexOf(busy), 1);
      let text = `error: ${e.message || "the request failed"}`;
      if (/Key '(agent|VLLM_URL|VLLM_MODEL)' not found/.test(e.message ?? "")) {
        text += "\n\nThis instance's agent app isn't configured for chat: add " +
          "`agent` to config.properties apps=, put VLLM_URL and VLLM_MODEL in " +
          "runtime/agent/botd.properties, and restart.";
      }
      pushCell({ kind: "agent", error: true, text });
    }
    sendBtn.disabled = false;
    input.focus();
  });
  input.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter" && !ev.shiftKey) {
      ev.preventDefault();
      form.requestSubmit();
    }
  });
  host.querySelector(".ag-clear").addEventListener("click", () => {
    messages = [];
    transcript = [];
    thread.replaceChildren();
    persist();
  });

  // ── memory: inspector + the owner's audit ────────────────────────────
  const domainsEl = host.querySelector(".ag-domains");
  const entriesEl = host.querySelector(".ag-entries");
  const memCap = host.querySelector(".ag-mem-cap");
  const journalEl = host.querySelector(".ag-mem-journal");
  let openDomainName = null;

  async function loadDomains() {
    store.invalidateControls("kb");
    const controls = await store.controls("kb");
    domainsEl.replaceChildren();
    if (controls instanceof Error) {
      domainsEl.textContent = "kb unavailable: " + controls.message;
      return;
    }
    for (const c of controls.slice().sort((a, b) => a.name.localeCompare(b.name))) {
      store.invalidateControl("kb", c.id);
      const rec = await store.control("kb", c.id);
      const btn = document.createElement("button");
      btn.className = "ag-domain" + (c.name === openDomainName ? " on" : "");
      let n = "?";
      try { n = JSON.parse((rec instanceof Error ? "[]" : rec.memory) ?? "[]").length; } catch { n = "!"; }
      btn.innerHTML = "";
      btn.textContent = `kb.${c.name} (${n})`;
      btn.title = (rec instanceof Error ? "" : rec.desc) ?? "";
      btn.addEventListener("click", () => openDomain(c.name));
      domainsEl.appendChild(btn);
    }
  }

  async function openDomain(name) {
    openDomainName = name;
    for (const b of domainsEl.querySelectorAll(".ag-domain")) {
      b.classList.toggle("on", b.textContent.startsWith(`kb.${name} `) || b.textContent === `kb.${name}`);
    }
    const r = await store.readFacet("kb", name, "memory");
    entriesEl.replaceChildren();
    journalEl.replaceChildren();
    if (r.status !== "ok") { entriesEl.textContent = "read failed: " + r.msg; return; }
    let entries;
    try { entries = JSON.parse(r.source || "[]"); }
    catch { entriesEl.textContent = "this domain's memory facet is not valid JSON — repair it in the journal's history"; return; }
    memCap.textContent = `kb.${name} — ${entries.length} entr${entries.length === 1 ? "y" : "ies"}; every change below is a journaled patch`;

    const save = async (next, label) => {
      const body = JSON.stringify(next, null, 2) + "\n";
      const pr = await store.patchFacet("kb", name, "memory",
        { oldSnippet: r.source, newSnippet: body, base: r.hash, label });
      if (pr.status !== "ok") { note(memCap, `save failed: ${pr.msg}`, true); return false; }
      await openDomain(name);
      await loadDomains();
      return true;
    };

    entries.forEach((e, i) => {
      const card = document.createElement("div");
      card.className = "ag-entry" + ((e.tags ?? "").split(",").some((t) => t.trim() === "unreviewed") ? " unreviewed" : "");
      const claim = document.createElement("div");
      claim.className = "ag-entry-claim";
      claim.textContent = e.claim ?? "(no claim)";
      card.appendChild(claim);
      if (e.detail) {
        const d = document.createElement("div");
        d.className = "ag-entry-detail";
        d.textContent = e.detail;
        card.appendChild(d);
      }
      const meta = document.createElement("div");
      meta.className = "ag-entry-meta lbl";
      const when = e.time ? new Date(e.time).toISOString().slice(0, 10) : "";
      meta.textContent = [e.tags ? `[${e.tags}]` : "", e.confidence ?? "", when,
        e.source?.doc ? `→ ${e.source.doc}` :
        e.source?.lib ? `→ ${e.source.lib}.${e.source.ctl}.${e.source.facet}` : ""]
        .filter(Boolean).join(" · ");
      card.appendChild(meta);
      if (e.source?.lib && e.source?.hash) {
        store.readFacet(e.source.lib, e.source.ctl, e.source.facet).then((ref) => {
          if (ref.status === "ok" && ref.hash !== e.source.hash) {
            const s = document.createElement("span");
            s.className = "ag-stale";
            s.textContent = "stale? referent changed";
            meta.appendChild(s);
          }
        });
      }
      const row = document.createElement("div");
      row.className = "ag-entry-actions";
      if ((e.tags ?? "").split(",").some((t) => t.trim() === "unreviewed")) {
        const bless = document.createElement("button");
        bless.textContent = "✓ reviewed";
        bless.title = "clear the unreviewed tag — the owner's audit blessing this entry";
        bless.addEventListener("click", async () => {
          const next = entries.map((x, j) => j !== i ? x : { ...x,
            tags: (x.tags ?? "").split(",").map((t) => t.trim())
              .filter((t) => t && t !== "unreviewed").join(",") });
          await save(next, `memory: reviewed — ${(e.claim ?? "").slice(0, 50)}`);
        });
        row.appendChild(bless);
      }
      const edit = document.createElement("button");
      edit.textContent = "edit";
      edit.addEventListener("click", () => {
        const ta = document.createElement("textarea");
        ta.className = "ag-entry-edit";
        ta.rows = 8;
        ta.value = JSON.stringify(e, null, 2);
        const ok = document.createElement("button");
        ok.textContent = "save entry";
        ok.addEventListener("click", async () => {
          let parsed;
          try { parsed = JSON.parse(ta.value); }
          catch { note(memCap, "that entry is not valid JSON", true); return; }
          await save(entries.map((x, j) => (j === i ? parsed : x)),
            `memory: edit — ${(parsed.claim ?? e.claim ?? "").slice(0, 50)}`);
        });
        card.append(ta, ok);
        edit.remove();
      });
      const del = document.createElement("button");
      del.textContent = "✕";
      del.title = "forget this entry (journaled — the history keeps it)";
      del.addEventListener("click", async () => {
        if (del.textContent !== "really forget?") {
          del.textContent = "really forget?";
          setTimeout(() => { del.textContent = "✕"; }, 2500);
          return;
        }
        await save(entries.filter((_, j) => j !== i),
          `memory: forget — ${(e.claim ?? "").slice(0, 50)}`);
      });
      row.append(edit, del);
      card.appendChild(row);
      entriesEl.appendChild(card);
    });

    const j = await store.listPatches("kb", name, 10);
    if (j.status === "ok") {
      for (const p of (j.patches ?? []).filter((p) => p.facet === "memory")) {
        const line = document.createElement("div");
        line.className = "ag-journal-line lbl";
        line.textContent = `${p.patch_id} · ${p.author} · ${p.label ?? ""}`;
        journalEl.appendChild(line);
      }
    }
  }

  host.querySelector(".ag-remember").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (!openDomainName) { note(memCap, "open a domain first", true); return; }
    const g = (c) => host.querySelector(c).value.trim();
    const entry = { claim: g(".ag-rem-claim") };
    if (g(".ag-rem-detail")) entry.detail = g(".ag-rem-detail");
    if (g(".ag-rem-tags")) entry.tags = g(".ag-rem-tags");
    entry.confidence = host.querySelector(".ag-rem-conf").value;
    const user = await store.user();
    const r = await store.agentCall("remember", {
      lib: "kb", domain: openDomainName, entry,
      author: user?.displayname || user?.id || "owner",
    });
    if (r.status !== "ok") { note(memCap, `remember failed: ${r.msg}`, true); return; }
    host.querySelector(".ag-rem-claim").value = "";
    host.querySelector(".ag-rem-detail").value = "";
    host.querySelector(".ag-rem-tags").value = "";
    await openDomain(openDomainName);
    await loadDomains();
  });

  host.querySelector(".ag-newdomain").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const name = host.querySelector(".ag-newdomain-name").value.trim();
    if (!/^[a-z][a-z0-9-]*$/.test(name)) { note(memCap, "lowercase + dashes", true); return; }
    const r = await store.agentCall("add_control", { lib: "kb", ctl: name });
    if (r.status !== "ok") { note(memCap, `add failed: ${r.msg}`, true); return; }
    host.querySelector(".ag-newdomain-name").value = "";
    await loadDomains();
  });

  // ── prompts: three journaled editors ─────────────────────────────────
  const PROMPTS = [
    { key: "core", lib: "dev", ctl: "prompts", facet: "prompt",
      cap: "the platform-knowledge CORE (dev.prompts) — every consumer assembles on this" },
    { key: "addendum", lib: "agent", ctl: "agentprompt", facet: "js",
      cap: "the OWNER ADDENDUM — appended to every notebook/chat prompt; your experiment surface",
      extract: (src) => {
        const m = src.match(/export const ADDENDUM = `([\s\S]*?)`;/);
        return m ? m[1].replace(/\\`/g, "`").replace(/\\\$/g, "$").replace(/\\\\/g, "\\") : null;
      },
      inject: (src, text) => {
        const esc = text.replace(/\\/g, "\\\\").replace(/`/g, "\\`").replace(/\$/g, "\\$");
        return src.replace(/export const ADDENDUM = `[\s\S]*?`;/,
          "export const ADDENDUM = `" + esc + "`;");
      } },
    { key: "archivist", lib: "agent", ctl: "archivist", facet: "prompt",
      cap: "the ARCHIVIST's extraction prompt — what gets remembered, and how conservatively" },
  ];
  let promptsLoaded = false;
  async function loadPrompts() {
    if (promptsLoaded) return;
    promptsLoaded = true;
    for (const p of PROMPTS) {
      const block = host.querySelector(`.ag-prompt[data-prompt="${p.key}"]`);
      const ta = block.querySelector("textarea");
      const status = block.querySelector(".ag-prompt-note");
      block.querySelector(".ag-prompt-cap").textContent = p.cap;
      const r = await store.readFacet(p.lib, p.ctl, p.facet);
      if (r.status !== "ok") { status.textContent = "read failed: " + r.msg; ta.disabled = true; continue; }
      let text = r.source;
      if (p.extract) {
        const inner = p.extract(r.source);
        if (inner === null) {
          status.textContent = "unexpected facet shape — edit it in the Development app instead";
          ta.disabled = true;
          continue;
        }
        text = inner;
      }
      ta.value = text;
      block.querySelector(".ag-prompt-save").addEventListener("click", async () => {
        const cur = await store.readFacet(p.lib, p.ctl, p.facet);
        if (cur.status !== "ok") { note(status, "re-read failed: " + cur.msg, true); return; }
        const next = p.inject ? p.inject(cur.source, ta.value) : ta.value;
        const pr = await store.patchFacet(p.lib, p.ctl, p.facet,
          { oldSnippet: cur.source, newSnippet: next, base: cur.hash,
            label: `prompt: ${p.key} edited in the agent app` });
        note(status, pr.status === "ok"
          ? "saved (journaled) — takes effect on the next ask"
          : `save failed: ${pr.msg}`, pr.status !== "ok");
      });
    }
  }

  function note(el, text, isErr) {
    el.textContent = text;
    el.classList.toggle("err", !!isErr);
    setTimeout(() => { if (el.textContent === text) el.textContent = ""; }, 6000);
  }

  return { dispose() { /* nothing persistent beyond localStorage */ } };
}
