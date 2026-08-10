// askrow — the agent add-on's chat surface, grafted into the notebook's
// plugin slot by the registry (dev.plugins: target dev.session, selector
// .ss-plugins). The notebook knows nothing about this control: everything
// it needs arrives through the slot's nbNotebook API plus the page's
// shared modules, and everything agent-flavored — prompts, tool defs,
// gating vocabulary, the archivist, config hints — stays on this side of
// the graft. No platform surface names this library.
var UUID = this.UUID;
(async () => {
  const el = document.getElementById(UUID);
  if (!el || typeof requireModule !== "function") return;
  const notebook = el.nbNotebook;
  if (!notebook) {
    console.warn("askrow: the slot carries no notebook API — not wiring");
    return;
  }
  const [{ store }, { chatctx }, loop, promptMod] = await Promise.all([
    requireModule("store", "askrow"),
    requireModule("chatctx", "askrow"),
    requireModule("agentloop", "askrow"),
    requireModule("agentprompt", "askrow"),
  ]);
  const toast = notebook.toast;

  const PINS_KEY = "bench.chat.pins";   // {add:[], remove:[]} vs DEFAULT_TOOLS
  const HISTORY_CELLS = 12;   // notebook cells folded into the agent's context

  const askInput = el.querySelector(".ss-ask");
  const sendBtn = el.querySelector(".ss-send");
  const ctxRows = el.querySelector(".ss-ctx-rows");
  const toolPick = el.querySelector(".ss-toolpick");
  const toolList = el.querySelector(".ss-toollist");
  const toolsBtn = el.querySelector(".ss-tools");
  const ctxChecked = new Map();
  let mcpTools = null;

  const stripThink = (t) => t.replace(/<think>[\s\S]*?<\/think>\n?/g, "").trim();

  function renderContextRows() {
    const snap = chatctx.snapshot();
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
  el.addEventListener("nb-session-open", renderContextRows);
  renderContextRows();

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

  let askConfirmed = new Set();   // commands typed-confirmed in the current ask

  function toolCell(callText, args, extra) {
    notebook.pushCell({ agent: true, call: callText,
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
      if (!store.writable()) {
        toolCell(callText, args, { error: true,
          output: "blocked: mutating tool on a read-only connection" });
        return "BLOCKED: this connection is read-only; mutating tools are unavailable.";
      }
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
    const result = await store.invoke(target.lib, target.ctl, target.cmd, args);
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
        activity.push(`· [${e.n}]${e.agent ? " (you ran)" : ""} ${e.call}(${e.args}) → ` +
          (e.error ? "err: " : "ok: ") + loop.clamp(e.output ?? "", 700));
      }
    }
    return { messages, activity };
  }

  async function ask() {
    const message = askInput.value.trim();
    if (!message) return;
    if (store.mode() !== "live") {
      toast.show("the agent needs a live connection (agent.llm on the instance)");
      return;
    }
    askInput.value = "";
    sendBtn.disabled = true;
    // history BEFORE the new cell lands, or this question shows up twice
    const { messages: turns, activity } = historyMessages();
    notebook.pushCell({ kind: "chat-user", text: message });
    const busy = notebook.busy("agent is thinking…");

    try {
      const included = chatctx.snapshot().filter((p) => ctxChecked.get(p.key) ?? true);
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
})();
