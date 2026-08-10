// agentloop.js — the client side of "chat with the code", the way the
// owner called it (2026-07-25): skip control_query AND tool_loop, call
// agent.llm.chat_llm directly. This module owns what control_query used to
// do server-side — the system prompt, the context fences, the history —
// plus the tool loop the notebook can actually SHOW: chat_llm returns
// normalized tool_calls; the session renders each as a cell and gates
// mutating ones behind its typed confirm before the loop continues.
//
// chat_llm contract (agent.llm, JSONObject envelope — payload under data):
//   in:  messages (system/user/assistant/tool), tools (OpenAI function defs)
//   out: {kind:"text", content, assistant_message}
//      | {kind:"tool_calls", tool_calls:[{id,name,arguments(JSON string)}],
//         assistant_message}   — assistant_message goes back on the list.
// MCP tools (agent.plugin.list_tools) arrive as {name:"lib-ctl-cmd",
// description, inputSchema} — inputSchema is a real JSON Schema.


// ── this library's own wire ─────────────────────────────────
// The platform's store carries no add-on-specific API: the provider owns
// its library name and command names. Everything below rides the store's
// generic by-name surface; resolution failures come back as err envelopes
// (never throws), so callers branch on status only.
//
// LIBRARY control — headless: defines window.NB_AGENTLOOP once (idempotent across
// installs). Consumers list this control as a hidden data-control child
// div and use the global from their ready.

var me = this;
var ME = document.getElementById(me.UUID);

me.ready = function () {
  if (window.NB_AGENTLOOP) return;
  const { packFor, ready: memoryReady } = window.NB_MEMORY;
  const jsonP = (c2, v2) => new Promise((res2) => json(c2, v2, res2));
  const invokeP = (l2, c2, m2, a2) => new Promise((res2) => invokeCommand(l2, c2, m2, a2, res2));
  const invoke = async (l2, c2, m2, a2) => {
    const t0 = performance.now();
    const envelope = await invokeP(l2, c2, m2, a2);
    return { envelope, ms: Math.round(performance.now() - t0) };
  };
  const code = (m2, a2) => invokeP("dev", "code", m2, a2);
  const readFacet = (l2, c2, f2) => code("read_control_facet", { lib: l2, ctl: c2, facet: f2 });
  window.NB_AGENTLOOP = (function () {

async function agentCall(ctl, cmd, args) {
  const r = await invoke("agent", ctl, cmd, args);
  return r.envelope;
}

/** The MCP tool list (agent.plugin.list_tools, FLAT {tools:[...]}). */
function listTools() {
  return agentCall("plugin", "list_tools", {});
}

/** One chat_llm completion (agent.llm — the OpenAI-compatible bridge to
    the instance's configured model). JSONObject envelope: the payload
    ({kind, content | tool_calls, assistant_message}) sits under data.
    chatTurn below drives this directly — control_query and tool_loop are
    deliberately not called. */
function chatLlm(messages, tools) {
  return agentCall("llm", "chat_llm", { messages, tools });
}

/** Draft a command description (agent.plugin.describe_command, String). */
function describeCommand(args) {
  return agentCall("plugin", "describe_command", args);
}

/** The archivist's intake (docs/memory.md) — the session fires and
    forgets; resolution failures resolve to Error values, not rejections. */
function logTurn(entry) {
  return invoke("agent", "archivist", "log_turn", entry);
}

/** chat_llm reads system.apps.agent.runtime (VLLM_URL/VLLM_MODEL) —
    absent when the agent app isn't exposed. The session shows whatever
    non-empty string this returns beneath a failed ask. */
function errorHint(msg) {
  if (/Key '(agent|VLLM_URL|VLLM_MODEL)' not found/.test(msg ?? "")) {
    return "This instance's agent app isn't configured: add `agent` to " +
      "config.properties apps=, put VLLM_URL and VLLM_MODEL in " +
      "runtime/agent/botd.properties, and restart " +
      "(tools/scratch-instance.md has the recipe).";
  }
  return "";
}

// Generous: the autonomous developer workflow legitimately spends rounds
// on compile-debug cycles.
const MAX_ROUNDS = 10;

// The system prompt lives client-side now — the cost of skipping
// control_query, and the point: the bench knows what the model is looking
// at and how its replies get used. The platform-knowledge sections are
// the owner's, folded in from agent.llm.tool_loop's prompt (his call:
// without them the model can't write ndata-correct code or reason about
// the platform). Model-agnostic (Qwen or Claude).
// The platform-knowledge CORE lives in the store (dev.prompts `prompt`
// facet — docs/prompting.md): ONE journaled curriculum shared with the
// agent app's tool_loop. SYSTEM_PROMPT here is the notebook's SHELL —
// venue, fences, memory, answer style; session assembles core + shell +
// TOOLS_PROMPT + the owner addendum at ask time.
const SYSTEM_PROMPT = `Right now you are speaking through the session notebook of Bench, the Newbound IDE — the user sees every tool call you make as a notebook cell.

WHAT YOU SEE
- Current code arrives in fenced blocks labeled lib:ctl.facet (facets) or lib:ctl:command.lang (command bodies; the first line may be a synthesized signature comment). A flow body arrives as JSON labeled lib:ctl:command.flow.
- "Recent notebook activity" lists commands run in this session, with OUTPUTS TRUNCATED. Never conclude that something is absent because you don't see it in a truncated result — rerun the command via a tool when you need the full output.

MEMORY IN THIS NOTEBOOK
- Your context includes a memory:index fence: every kb domain with entry counts, tags, staleness marks ("N stale?" — re-read those referents before trusting the marked entries), and entries pushed for the surface open on the bench ("Relevant now").
- Memory FORMATION is automatic here: the archivist reviews completed sessions in the background and files what is durable (journaled, tagged unreviewed). Do not file memories on your own initiative. When the user explicitly asks you to remember something, call dev-code-remember — it runs without confirmation, because the request itself is the authorization.

HOW TO ANSWER
- Be concise and concrete. If you are unsure, say so plainly rather than inventing platform behavior.
- To propose a facet change, reply with ONE complete fenced block per changed facet, tagged html, css, or js. The user can apply such a block as a journaled whole-facet patch in one click — a partial snippet cannot be applied, so always return the full facet.
- Command-body suggestions: a fenced block tagged rust or python containing the full body.`;

// Fetched lazily at ask time (the connection exists by then; module
// install happens before it does). Cached per page; a failed fetch
// clears the cache so the next ask retries, and the error is LOUD —
// a silently thin prompt regresses the confabulation lesson.
let corePromise = null;
function corePrompt() {
  if (!corePromise) {
    corePromise = (async () => {
      const r = await readFacet("agent", "prompts", "prompt");
      if (r?.status === "ok" && r.exists && (r.source ?? "").trim()) {
        return r.source.trim();
      }
      corePromise = null;
      throw new Error("dev.prompts `prompt` facet unavailable — the platform curriculum is store-resident (docs/prompting.md)");
    })();
  }
  return corePromise;
}

// The tool model (owner's design, 2026-07-25): DISCOVERY is always on,
// ATTACHMENT is a context optimization, AUTHORIZATION happens per call.
const TOOLS_PROMPT = `

TOOLS
Newbound commands are your tools, named library-control-command (e.g. dev-code-read_command). Call them through the tool-calling interface — never by describing a call in text. Every call you make appears as a cell in the user's notebook.

- A core set is attached with full schemas. EVERY other platform command is also available: use find_tools(query) to discover commands for a task, describe_tool(name) for a full schema, and call_command(name, args) to invoke anything not directly attached. Never claim a capability is missing until find_tools says so.
- Authorization is per call, not per tool: read-only commands run immediately; anything mutating or potentially destructive stops for the user's typed confirmation first — so do not hesitate to REQUEST a powerful command when the task needs it; the user decides at the moment of use. A denial is the user's decision: respect it and continue without that action.
- Read before you write. For edits prefer the journaled, revertible commands: dev-code-patch_control_facet for UI facets, dev-code-patch_command_body / upsert_command for command bodies, dev-code-read_flow_body / write_flow_body for flow commands.`;

// Newbound meta types -> Rust types, for synthesized signatures (ported
// from control_query's lookup_rust_api_data_type).
const RUST_TYPE = {
  FLAT: "DataObject", JSONObject: "DataObject", JSONArray: "DataArray",
  InputStream: "DataBytes", float: "f64", Integer: "i64", Boolean: "bool",
  Any: "Data", NULL: "DNull",
};
const rustType = (t) => RUST_TYPE[t] ?? "String";

/** The context prompt, ported from control_query: an editing preamble plus
    one fence per included piece, tagged the way the backend prompt always
    tagged them. `included` = viewctx snapshot entries the user checked. */
function contextBlock(included) {
  const merged = {};
  for (const p of included) Object.assign(merged, p.fields);
  const { lib, ctl } = merged;
  let out = "";
  if (lib || ctl) {
    out = `We are editing the ${ctl} control in the ${lib} library.`;
    for (const facet of ["html", "css", "js"]) {
      const v = (merged[facet] ?? "").trim?.() ?? "";
      if (v) out += `\n\n\`\`\`${lib}:${ctl}.${facet}\n${v}\n\`\`\``;
    }
    if (merged.code && merged.cmdname) {
      const lang = merged.lang === "rust" ? "rs" : merged.lang === "python" ? "py" : merged.lang;
      let signature = "";
      if (merged.cmdparams) {
        const ps = merged.cmdparams.map((p) =>
          lang === "rs" ? `${p.name}: ${rustType(p.type)}` : p.name).join(", ");
        signature = lang === "rs"
          ? `// fn ${merged.cmdname}(${ps}) -> ${rustType(merged.cmdreturn ?? "String")}\n`
          : `# def ${merged.cmdname}(${ps}):\n`;
      }
      out += `\n\n\`\`\`${lib}:${ctl}:${merged.cmdname}.${lang}\n${signature}${merged.code.trim()}\n\`\`\``;
    }
    if (merged.flow && merged.cmdname) {
      out += `\n\n\`\`\`${lib}:${ctl}:${merged.cmdname}.flow\n${merged.flow}\n\`\`\``;
    }
  }
  if (merged.memory) {
    out += (out ? "\n\n" : "") + `\`\`\`memory:index\n${merged.memory.trim()}\n\`\`\``;
  }
  return out;
}

const clamp = (s, n = 2000) =>
  s.length <= n ? s : s.slice(0, n) + `\n…[${s.length - n} chars clipped]`;

/** MCP tool descriptors -> OpenAI function defs, enabled names only. */
function toolDefs(mcpTools, enabledNames) {
  const on = new Set(enabledNames);
  return (mcpTools ?? [])
    .filter((t) => on.has(t.name))
    .map((t) => ({
      type: "function",
      function: {
        name: t.name,
        description: t.description ?? "",
        parameters: t.inputSchema ?? { type: "object", properties: {} },
      },
    }));
}

// ── the tool model: defaults, meta-tools, gating ──────────

// Attached-by-default: the read family (auto-run) plus the journaled
// workhorses (each call confirm-gated). Everything else stays reachable
// through call_command. The picker stores per-user add/remove overrides.
const DEFAULT_TOOLS = [
  "dev-code-read_control_facet", "dev-code-read_command",
  "dev-code-read_flow_body", "dev-code-list_control_patches",
  "dev-code-list_libraries", "dev-code-list_controls",
  "dev-code-list_commands", "dev-code-list_assets",
  "dev-code-search_commands",
  "dev-code-patch_control_facet", "dev-code-patch_command_body",
  "dev-code-upsert_command", "dev-code-write_flow_body",
  "dev-code-invoke_command", "dev-code-evaluate_rust",
  "dev-code-remember",
];

/** The always-attached meta-tools: discovery + the gateway. */
const META_TOOL_DEFS = [
  {
    type: "function",
    function: {
      name: "find_tools",
      description: "Search ALL platform commands (not just attached tools) by keywords against their names and descriptions. Returns matches with their authorization tier. Use this before concluding a capability is missing.",
      parameters: { type: "object",
        properties: { query: { type: "string", description: "Keywords, e.g. 'delete asset' or 'rust eval'" } },
        required: ["query"] },
    },
  },
  {
    type: "function",
    function: {
      name: "describe_tool",
      description: "Full descriptor (description + JSON Schema of arguments) for one platform command by its lib-ctl-cmd name. Use before call_command when unsure of the arguments.",
      parameters: { type: "object",
        properties: { name: { type: "string", description: "Tool name, e.g. dev-code-delete_asset" } },
        required: ["name"] },
    },
  },
  {
    type: "function",
    function: {
      name: "call_command",
      description: "Invoke ANY platform command by its lib-ctl-cmd name — attached or not. Arguments are validated against the command's schema; mutating commands stop for the user's confirmation like every other call.",
      parameters: { type: "object",
        properties: {
          name: { type: "string", description: "Tool name, e.g. dev-code-delete_asset" },
          args: { type: "object", description: "The command's arguments, per its schema" },
        },
        required: ["name", "args"] },
    },
  },
];

const READ_TOOL_PREFIXES = [
  "list", "read", "get", "search", "describe", "lib_info", "check",
  "current", "info", "peers", "libs", "apps", "assets", "asset", "lookup",
];

/** Authorization tier for a catalog entry: "auto" (read — runs directly)
    or "confirm" (stops at the typed confirm). The TAGS convention wins
    when present — a command whose MCP tags include agent-auto or
    agent-confirm is classified by the owner (describe() forwards the
    record's `tags` field once the flowlang side does; empty today,
    forward-compatible). `groups` is SECURITY groups and is never read
    for gating. Fallback: the read-prefix heuristic on the command name. */
// remember runs WITHOUT confirmation on explicit user request — the ask
// itself is the authorization (docs/memory.md); autonomous memory
// formation belongs to the archivist, and the prompt says so.
const AUTO_OVERRIDES = new Set(["dev-code-remember"]);

function gateFor(name, entry) {
  if (AUTO_OVERRIDES.has(name)) return "auto";
  const tags = entry?.tags ?? [];
  if (tags.includes("agent-confirm")) return "confirm";
  if (tags.includes("agent-auto")) return "auto";
  const cmd = parseToolName(name)?.cmd ?? name;
  return READ_TOOL_PREFIXES.some((p) => cmd === p || cmd.startsWith(p + "_"))
    ? "auto" : "confirm";
}

/** Light client-side check of args against a catalog inputSchema — enough
    for the model to self-correct: missing required keys and gross type
    mismatches, reported together. Null = fine. */
function schemaComplaints(schema, args) {
  if (!schema || typeof args !== "object" || args === null) return null;
  const problems = [];
  for (const req of schema.required ?? []) {
    if (!(req in args)) problems.push(`missing required argument '${req}'`);
  }
  const kinds = { string: "string", number: "number", boolean: "boolean" };
  for (const [k, v] of Object.entries(args)) {
    const spec = schema.properties?.[k];
    if (!spec) continue;
    if (spec.type in kinds && typeof v !== kinds[spec.type]) {
      problems.push(`argument '${k}' should be a ${spec.type}`);
    } else if (spec.type === "object" && (typeof v !== "object" || v === null || Array.isArray(v))) {
      problems.push(`argument '${k}' should be an object`);
    } else if (spec.type === "array" && !Array.isArray(v)) {
      problems.push(`argument '${k}' should be an array`);
    }
  }
  return problems.length ? problems.join("; ") : null;
}

/** Search the catalog for find_tools: token match on name + description. */
function searchCatalog(catalog, query, limit = 12) {
  const tokens = (query ?? "").toLowerCase().split(/[^a-z0-9_]+/).filter(Boolean);
  if (!tokens.length) return [];
  const scored = [];
  for (const t of catalog) {
    const name = t.name.toLowerCase();
    const desc = (t.description ?? "").toLowerCase();
    // Name hits must dominate: nearly every DESCRIPTION contains "newbound"
    // or "command", so flat token counting degenerated to catalog order (a
    // real trace ranked close_stream above read_command for "read command").
    let score = 0;
    let matched = 0;
    for (const tok of tokens) {
      if (name.includes(tok)) { score += 5; matched++; }
      else if (desc.includes(tok)) { score += 1; matched++; }
    }
    if (!matched) continue;
    if (matched === tokens.length) score += 3;
    scored.push([score, t]);
  }
  scored.sort((a, b) => b[0] - a[0]);
  return scored.slice(0, limit).map(([, t]) => ({
    name: t.name,
    gate: gateFor(t.name, t),
    summary: (t.summary ?? t.description ?? "").slice(0, 180),
  }));
}

/** "lib-ctl-cmd" -> {lib, ctl, cmd} (ctl may itself contain dashes). */
function parseToolName(name) {
  const parts = name.split("-");
  if (parts.length < 3) return null;
  return { lib: parts[0], ctl: parts.slice(1, -1).join("-"), cmd: parts.at(-1) };
}

/**
 * One conversational turn, driving chat_llm until it answers in text.
 * execTool(call) is the session's: it renders the cell, gates mutating
 * commands behind the typed confirm, runs it, and resolves the tool-result
 * CONTENT string (or a denial note). Returns the final text.
 */
async function chatTurn({ messages, tools, execTool, onRound }) {
  const seen = new Map();   // result text -> tool name, for the context guard
  // Recall layer 4: the mode-keyed pack for THIS ask, APPENDED to the one
  // system message on every provider call but never written into the
  // venue's own messages array — the conversation the venue keeps must
  // not accumulate a pack per turn. Rebuilt per call because the loop
  // pushes tool exchanges into `messages` as it goes. One implementation
  // covers every venue that speaks through chatTurn.
  let pack = null;
  try {
    await memoryReady;
    const ask = [...messages].reverse().find((m) => m.role === "user")?.content ?? "";
    pack = packFor(ask);
  } catch { pack = null; }
  globalThis.__nbAgentPack = pack
    ? { modes: pack.modes, count: pack.count, total: pack.total, at: Date.now() }
    : null;   // instrumentation: a failed turn should show what was injected
  // ONE system message, at index 0. vLLM's chat template raises
  // "System message must be at the beginning." for a system message at
  // any later index — including index 1, ahead of every user turn — so
  // the pack is APPENDED to the existing system prompt rather than
  // inserted beside it. Appending (not prepending) also keeps the core
  // curriculum as a stable prefix, so server-side prefix caching still
  // hits. The venue's own array is never mutated; the merged copy lives
  // only for the duration of the call.
  const convo = () => {
    if (!pack) return messages;
    if (messages[0]?.role === "system") {
      return [{ ...messages[0], content: messages[0].content + "\n\n" + pack.block },
              ...messages.slice(1)];
    }
    return [{ role: "system", content: pack.block }, ...messages];
  };
  for (let round = 0; round < MAX_ROUNDS; round++) {
    const r = await chatLlm(convo(), tools);
    if (r.status !== "ok") throw new Error(r.msg ?? "chat_llm failed");
    const d = r.data ?? {};
    // Failures ride INSIDE the FLAT envelope: wrapper-caught panics as
    // {status:"err", msg} (a missing VLLM config lands here), chat_llm's own
    // transport/parse errors as {kind:"error", content}. Returning either as
    // a normal reply renders an EMPTY or misleading cell — throw, so every
    // venue's catch shows an honest error.
    if (d.status === "err") throw new Error(d.msg ?? "chat_llm failed");
    if (d.kind === "error") throw new Error(d.content ?? "chat_llm failed");
    if (d.kind !== "tool_calls") {
      return d.content ?? "";
    }
    messages.push(d.assistant_message);
    onRound?.(d.tool_calls);
    for (const call of d.tool_calls) {
      const content = await execTool(call);
      // Context guard: if an identical result already sits in this turn's
      // messages (the model re-read an unchanged facet — a real trace showed
      // the same 26KB source pulled three times), elide the duplicate TEXT.
      // The tool still ran; only the repeated payload is replaced.
      const prior = seen.get(content);
      messages.push({ role: "tool", tool_call_id: call.id,
        content: prior !== undefined && content.length > 500
          ? `[result identical to the ${prior} call earlier this turn — unchanged; reuse that output]`
          : content });
      if (prior === undefined) seen.set(content, call.function?.name ?? "tool");
    }
  }
  // Budget exhausted. A bare stop misreports turns whose LAST round did the
  // work (a real trace: the successful whole-facet patch landed on round 10
  // and the user was told the agent stopped without an answer). Force one
  // toolless wrap-up call; fall back to the honest stop line if it fails.
  messages.push({ role: "user", content:
    "[Tool budget exhausted. Do not request tools. Answer NOW from what you already have: what you completed, what succeeded or failed (name any patch ids), and what remains undone.]" });
  try {
    const r = await chatLlm(convo(), []);
    if (r.status === "ok") {
      const d = r.data ?? {};
      if (d.status !== "err" && d.kind !== "error" && d.kind !== "tool_calls"
          && (d.content ?? "").trim()) {
        return d.content;
      }
    }
  } catch { /* fall through to the honest stop */ }
  return "(stopped: the agent used its whole tool budget without a final answer)";
}

    return { listTools, chatLlm, describeCommand, logTurn, errorHint, MAX_ROUNDS, SYSTEM_PROMPT, corePrompt, TOOLS_PROMPT, contextBlock, clamp, toolDefs, DEFAULT_TOOLS, META_TOOL_DEFS, gateFor, schemaComplaints, searchCatalog, parseToolName, chatTurn };
  })();
};
