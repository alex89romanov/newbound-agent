// askrow — the agent add-on's chat surface, grafted into the notebook's
// plugin slot by the registry (dev.plugins: target dev.session, selector
// .ss-plugins). Classic through and through: the module dependencies are
// child divs of this control (activated before me.ready fires —
// registration is idempotent), and the notebook is found the stock way,
// walking up the DOM to the first ancestor carrying `.api`. Everything
// agent-flavored — prompts, tool defs, gating vocabulary, the archivist,
// config hints — stays on this side of the graft; dev names nothing.
var me = this;
var ME = $('#' + me.UUID)[0];

me.ready = async function () {
  var up = $(ME).parent();
  while (up[0] && !up[0].api) up = up.parent();
  const notebook = up[0] ? up[0].api : null;
  if (!notebook || !notebook.pushCell) {
    console.warn("askrow: no notebook api above the graft point — not wiring");
    return;
  }
  const { viewctx } = window.NB_VIEWCTX;
  const loop = window.NB_AGENTLOOP;
  const promptMod = window.NB_AGENTPROMPT;
  const jsonP = (c2, v2) => new Promise((res2) => json(c2, v2, res2));
  const invokeP = (l2, c2, m2, a2) => new Promise((res2) => invokeCommand(l2, c2, m2, a2, res2));
  const invoke = async (l2, c2, m2, a2) => {
    const t0 = performance.now();
    const envelope = await invokeP(l2, c2, m2, a2);
    return { envelope, ms: Math.round(performance.now() - t0) };
  };
  const code = (m2, a2) => invokeP("dev", "code", m2, a2);
  const readFacet = (l2, c2, f2) => code("read_control_facet", { lib: l2, ctl: c2, facet: f2 });
  const patchFacet = (l2, c2, f2, { oldSnippet, newSnippet, base = "", label = "" }) =>
    code("patch_control_facet", { lib: l2, ctl: c2, facet: f2, old_snippet: oldSnippet,
      new_snippet: newSnippet, base, label, author: "" });

  const toast = notebook.toast;

  const PINS_KEY = "bench.chat.pins";   // {add:[], remove:[]} vs DEFAULT_TOOLS
  const HISTORY_CELLS = 12;   // notebook cells folded into the agent's context

  const askInput = ME.querySelector(".ss-ask");
  const sendBtn = ME.querySelector(".ss-send");
  const ctxRows = ME.querySelector(".ss-ctx-rows");
  const toolPick = ME.querySelector(".ss-toolpick");
  const toolList = ME.querySelector(".ss-toollist");
  const toolsBtn = ME.querySelector(".ss-tools");
  const ctxChecked = new Map();
  let mcpTools = null;

  const stripThink = (t) => t.replace(/<think>[\s\S]*?<\/think>\n?/g, "").trim();

  function renderContextRows() {
    const snap = viewctx.snapshot();
    ctxRows.replaceChildren(...snap.map(({ key, label }) => {
      const l = document.createElement("label");
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.checked = ctxChecked.get(key) ?? true;
      cb.addEventListener("change", () => ctxChecked.set(key, cb.checked));
      l.append(cb, document.createTextNode(label));
      return l;
    }));
    if (snap.length === 0) {
      const none = document.createElement("span");
      none.className = "none";
      none.textContent = "nothing open";
      ctxRows.appendChild(none);
    }
  }
  // snapshot() is live — render now, refresh whenever the ask row takes
  // focus (the moment freshness matters); no events, no observers
  renderContextRows();
  askInput.addEventListener("focus", renderContextRows);

  // Pins are OVERRIDES against DEFAULT_TOOLS: attachment is a context
  // optimization, not permission — every command stays reachable through
  // call_command, gated identically at use time.
  function pins() {
    try {
      const p = JSON.parse(localStorage.getItem(PINS_KEY)) ?? {};
      return { add: p.add ?? [], remove: p.remove ?? [] };
    } catch {
      return { add: [], remove: [] };
    }
  }

  function attachedNames() {
    const { add, remove } = pins();
    const catalogNames = new Set((mcpTools ?? []).map((t) => t.name));
    return [...new Set([...loop.DEFAULT_TOOLS, ...add])]
      .filter((n) => !remove.includes(n) && (mcpTools === null || catalogNames.has(n)));
  }

  function updateToolsBtn() {
    toolsBtn.textContent = `tools ▸ (${attachedNames().length} attached · all discoverable)`;
  }
  updateToolsBtn();

  async function ensureCatalog() {
    if (mcpTools !== null) return mcpTools;
    const r = await loop.listTools();
    mcpTools = r.status === "ok" ? (r.tools ?? []) : [];
    updateToolsBtn();   // the count means defaults ∩ catalog once known
    return mcpTools;
  }

  async function renderTools() {
    await ensureCatalog();
    const attached = new Set(attachedNames());
    toolList.replaceChildren(...(mcpTools ?? []).map((tool) => {
      const l = document.createElement("label");
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.checked = attached.has(tool.name);
      cb.addEventListener("change", () => {
        const p = pins();
        const isDefault = loop.DEFAULT_TOOLS.includes(tool.name);
        if (cb.checked) {
          p.remove = p.remove.filter((n) => n !== tool.name);
          if (!isDefault && !p.add.includes(tool.name)) p.add.push(tool.name);
        } else {
          p.add = p.add.filter((n) => n !== tool.name);
          if (isDefault && !p.remove.includes(tool.name)) p.remove.push(tool.name);
        }
        localStorage.setItem(PINS_KEY, JSON.stringify(p));
        updateToolsBtn();
      });
      const nm = document.createElement("span");
      nm.textContent = tool.name;
      const gate = document.createElement("span");
      gate.className = "tgate " + (loop.gateFor(tool.name, tool) === "auto" ? "g-auto" : "g-confirm");
      gate.textContent = loop.gateFor(tool.name, tool) === "auto" ? "auto" : "confirm";
      const desc = document.createElement("span");
      desc.className = "tdesc";
      desc.textContent = tool.description ?? "";
      l.append(cb, nm, gate, desc);
      return l;
    }));
  }

  toolsBtn.addEventListener("click", async (e) => {
    toolPick.hidden = !toolPick.hidden;
    e.currentTarget.setAttribute("aria-pressed", String(!toolPick.hidden));
    if (!toolPick.hidden && mcpTools === null) await renderTools();
  });

  // ── chat cells: THIS control's cell kinds, rendered by this control ──
  // The notebook knows nothing about chat; it renders command cells and
  // dividers, and hands custom kinds to whoever registered them.
  function renderReply(host, raw) {
    const lines = (raw ?? "").split("\n");
    let think = null;
    let code = null;
    let lang = null;
    let textBuf = [];
    const flushText = () => {
      const t = textBuf.join("\n").trim();
      if (t) host.appendChild(document.createTextNode(t + "\n"));
      textBuf = [];
    };
    for (const line of lines) {
      if (line.startsWith("<think>")) {
        flushText();
        think = [line.slice(7)];
      } else if (line.startsWith("</think>")) {
        const body = (think ?? []).join("\n").trim();
        think = null;
        if (body) {
          const btn = document.createElement("button");
          btn.className = "ss-think-btn";
          btn.textContent = "show thinking ▸";
          const div = document.createElement("div");
          div.className = "ss-think";
          div.hidden = true;
          div.textContent = body;
          btn.addEventListener("click", () => {
            div.hidden = !div.hidden;
            btn.textContent = div.hidden ? "show thinking ▸" : "hide thinking ▾";
          });
          host.append(btn, div);
        }
      } else if (think) {
        think.push(line);
      } else if (line.startsWith("```")) {
        if (code === null) {
          flushText();
          lang = line.slice(3).trim().toLowerCase();
          code = [];
        } else {
          host.appendChild(codeCard(code.join("\n"), lang));
          code = null;
        }
      } else if (code) {
        code.push(line);
      } else {
        textBuf.push(line);
      }
    }
    if (code) host.appendChild(codeCard(code.join("\n"), lang));
    flushText();
  }

  function codeCard(source, lang) {
    const card = document.createElement("div");
    card.className = "ss-code";
    const head = document.createElement("div");
    head.className = "ss-code-head";
    const tag = document.createElement("span");
    tag.className = "langtag";
    tag.textContent = lang || "code";
    head.appendChild(tag);
    const pre = document.createElement("pre");
    pre.hidden = true;
    pre.textContent = source;
    const view = document.createElement("button");
    view.textContent = "view ▸";
    view.addEventListener("click", () => {
      pre.hidden = !pre.hidden;
      view.textContent = pre.hidden ? "view ▸" : "hide ▾";
    });
    head.appendChild(view);
    const wb = viewctx.snapshot().find((p) => p.key === "workbench");
    if (["html", "css", "js"].includes(lang) && wb) {
      const apply = document.createElement("button");
      apply.className = "ss-apply";
      apply.textContent = "apply as patch ▸";
      apply.addEventListener("click", async () => {
        if (apply.textContent === "apply as patch ▸") {
          apply.textContent = `replace whole ${lang} facet of ${wb.fields.ctl}?`;
          setTimeout(() => {
            if (!apply.disabled) apply.textContent = "apply as patch ▸";
          }, 3000);
          return;
        }
        const rf = await readFacet(wb.fields.lib, wb.fields.ctl, lang);
        if (rf.status !== "ok") {
          toast.show(`apply failed: ${rf.msg}`);
          return;
        }
        const r = await patchFacet(wb.fields.lib, wb.fields.ctl, lang, {
          oldSnippet: "", newSnippet: source.replace(/\r/g, ""),
          base: rf.hash, label: "chat",
        });
        if (r.status !== "ok") {
          toast.show(`apply failed: ${r.msg}`);
          return;
        }
        apply.textContent = `applied · ${r.patch_id}`;
        apply.disabled = true;
        toast.show(`chat → patch_control_facet · ${r.patch_id} — reload the control to see it`);
      });
      head.appendChild(apply);
    }
    card.append(head, pre);
    return card;
  }

  function renderChatCell(entry) {
    const cell = document.createElement("div");
    cell.className = "ss-cell ss-chat";
    const input = document.createElement("div");
    input.className = "ss-cell-in";
    const who = entry.kind === "chat-user" ? "you ▸" : "agent ▸";
    input.innerHTML = `<span class="ss-gutter"></span><span class="who"></span>`;
    input.querySelector(".ss-gutter").textContent = `[${entry.n}·✳]`;
    input.querySelector(".who").textContent = who;
    const out = document.createElement("div");
    out.className = "ss-cell-out" + (entry.error ? " err" : "");
    if (entry.kind === "chat-agent" && !entry.error) {
      renderReply(out, entry.text);
    } else {
      out.textContent = entry.text;
    }
    cell.append(input, out);
    return cell;
  }
  notebook.addRenderer("chat-user", renderChatCell);
  notebook.addRenderer("chat-agent", renderChatCell);

  let askConfirmed = new Set();   // commands typed-confirmed in the current ask

  function toolCell(callText, args, extra) {
    notebook.pushCell({ auto: true, call: callText,
      args: JSON.stringify(args), ...extra });
  }

  /** The agent's tool executor: a visible, gated notebook cell. Handles
      the always-attached meta-tools (find_tools/describe_tool/call_command)
      client-side; everything else is a platform command, gated per call
      through the notebook's own confirm ceremonies. */
  async function execTool(call) {
    let args = {};
    try {
      args = call.arguments ? JSON.parse(call.arguments) : {};
    } catch { /* leave {} — the cell shows the raw string below */ }

    if (call.name === "find_tools") {
      const catalog = await ensureCatalog();
      const hits = loop.searchCatalog(catalog, args.query ?? "");
      toolCell("✳ find_tools", args, {
        output: hits.length
          ? hits.map((h) => `${h.name} [${h.gate}] — ${h.summary}`).join("\n")
          : "no matches",
      });
      return hits.length
        ? JSON.stringify(hits)
        : "No commands matched. Try different keywords, or list_commands on a likely control.";
    }
    if (call.name === "describe_tool") {
      const catalog = await ensureCatalog();
      const entry = catalog.find((t) => t.name === args.name);
      toolCell("✳ describe_tool", args, {
        output: entry ? `${entry.name} [${loop.gateFor(entry.name, entry)}]` : "unknown tool",
        error: !entry,
      });
      return entry
        ? JSON.stringify({ ...entry, gate: loop.gateFor(entry.name, entry) })
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
      const complaints = loop.schemaComplaints(entry.inputSchema, args);
      if (complaints) {
        toolCell("✳ call_command", { name, args }, { error: true,
          output: `arguments rejected: ${complaints}` });
        return `ARGUMENTS REJECTED for ${name}: ${complaints}. Fix and call again.`;
      }
    }

    const target = loop.parseToolName(name);
    if (!target) {
      return `ERROR: tool name "${name}" is not lib-ctl-cmd shaped`;
    }
    const callText = `${target.lib}.${target.ctl}.${target.cmd}`;
    const catalog = await ensureCatalog();
    const entry = catalog.find((t) => t.name === name);
    if (loop.gateFor(name, entry) !== "auto") {
      const confirmed = askConfirmed.has(target.cmd)
        ? await notebook.confirmLite(target.cmd,
            `the agent wants to run "${target.cmd}" again in this ask:`)
        : await notebook.confirmTyped(target.cmd);
      if (!confirmed) {
        toolCell(callText, args, { error: true,
          output: "denied by the user" });
        return "DENIED: the user declined this action. Continue without it.";
      }
      askConfirmed.add(target.cmd);
    }
    const result = await invoke(target.lib, target.ctl, target.cmd, args);
    let entryOut;
    let content;
    if (result instanceof Error) {
      entryOut = { error: true, output: result.message };
      content = `ERROR: ${result.message}`;
    } else {
      const env = result.envelope;
      const payload = env.status === "ok" ? (env.data ?? env.msg ?? env) : env.msg;
      const text = typeof payload === "string" ? payload : JSON.stringify(payload, null, 1);
      entryOut = { ms: result.ms, error: env.status !== "ok", output: text };
      content = env.status === "ok" ? loop.clamp(text, 4000) : `ERROR: ${env.msg}`;
    }
    toolCell(callText, args, entryOut);
    return content;
  }

  /** Prior notebook cells, compacted for the agent's memory. History
      stops at the newest "new context" divider (the record survives; the
      memory resets), and error turns never feed it — a failed answer
      re-anchoring the next one is how a session poisons itself. */
  function historyMessages() {
    let cells = notebook.cells();
    const lastDivider = cells.findLastIndex((e) => e.kind === "divider");
    if (lastDivider >= 0) cells = cells.slice(lastDivider + 1);
    cells = cells.slice(-HISTORY_CELLS);
    const messages = [];
    const activity = [];
    for (const e of cells) {
      if (e.error) {
        continue;
      } else if (e.kind === "chat-user") {
        messages.push({ role: "user", content: e.text });
      } else if (e.kind === "chat-agent") {
        messages.push({ role: "assistant", content: stripThink(e.text ?? "") });
      } else if (e.kind === "divider") {
        continue;
      } else {
        activity.push(`· [${e.n}]${(e.auto ?? e.agent) ? " (you ran)" : ""} ${e.call}(${e.args}) → ` +
          (e.error ? "err: " : "ok: ") + loop.clamp(e.output ?? "", 700));
      }
    }
    return { messages, activity };
  }

  async function ask() {
    const message = askInput.value.trim();
    if (!message) return;
    askInput.value = "";
    sendBtn.disabled = true;
    // history BEFORE the new cell lands, or this question shows up twice
    const { messages: turns, activity } = historyMessages();
    notebook.pushCell({ kind: "chat-user", text: message });
    const busy = notebook.busy("agent is thinking…");

    try {
      const included = viewctx.snapshot().filter((p) => ctxChecked.get(p.key) ?? true);
      let ctxMsg = loop.contextBlock(included);
      if (activity.length) {
        // The first live run drew a confident wrong conclusion from a
        // truncated result — the header now says so up front.
        ctxMsg += (ctxMsg ? "\n\n" : "") +
          "Recent notebook activity (outputs truncated — never conclude " +
          "something is absent from a truncated result; rerun the command " +
          "via a tool for the full output):\n" + activity.join("\n");
      }
      const catalog = await ensureCatalog();
      askConfirmed = new Set();   // typed-confirm ceremony resets per ask
      const tools = [...loop.META_TOOL_DEFS, ...loop.toolDefs(catalog, attachedNames())];
      const addendum = (promptMod.ADDENDUM ?? "").trim();
      const messages = [{ role: "system",
        content: (await loop.corePrompt()) + "\n\n" +
          loop.SYSTEM_PROMPT + loop.TOOLS_PROMPT +
          (addendum ? "\n\nOWNER ADDENDUM\n" + addendum : "") }];
      if (ctxMsg) {
        messages.push({ role: "user",
          content: "[CONTEXT — what the user is looking at]\n\n" + ctxMsg });
      }
      messages.push(...turns, { role: "user", content: message });

      const text = await loop.chatTurn({
        messages,
        tools,
        execTool,
        onRound: () => { busy.update("agent is using tools…"); },
      });
      busy.remove();
      notebook.pushCell({ kind: "chat-agent", text });
      // the archivist's intake (docs/memory.md) — fire-and-forget
      loop.logTurn({
        venue: "notebook",
        ask: message.slice(0, 4000),
        reply: (text ?? "").slice(0, 4000),
        tools: "",
        author: "notebook",
      }).catch(() => {});
    } catch (e) {
      busy.remove();
      let text = `error: ${e.message || "the request failed (see the instance console)"}`;
      const hint = loop.errorHint(e.message ?? "");
      if (hint) text += "\n\n" + hint;
      notebook.pushCell({ kind: "chat-agent", error: true, text });
    }
    sendBtn.disabled = false;
    askInput.focus();
  }

  sendBtn.addEventListener("click", ask);
  askInput.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      ask();
    }
  });
};
