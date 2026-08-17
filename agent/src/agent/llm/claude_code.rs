use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use ndata::dataobject::DataObject;
use std::process::{Command, Stdio};
use std::io::Write;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["messages", "tools"] {
        if !o.has(p) {
            let mut e = DataObject::new();
            e.put_string("status", "err");
            e.put_string("msg", &format!("missing required parameter: {}", p));
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", e);
            return result_obj;
        }
    }
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let arg_0: DataArray = o.get_array("messages");
        let arg_1: DataArray = o.get_array("tools");
        claude_code(arg_0, arg_1)
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_object("a", ax);
            result_obj
        }
        Err(err) => {
            let mut err_obj = DataObject::new();
            err_obj.put_string("status", "err");

            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic occurred".to_string()
            };

            err_obj.put_string("msg", &msg);
            // Wrapped in the same `a` envelope a successful return uses.
            // Unwrapped, callers that unpack the envelope (newbound's
            // format_result, for one) report an opaque 500 — "Not an object:
            // DString(\"err\")" — instead of this message.
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", err_obj);
            result_obj
        }
    }
}

pub fn claude_code(messages: DataArray, tools: DataArray) -> DataObject {
// A CUSTOM ARM for chat_llm's LLM_CTL escape hatch: inference through the
// Claude Code CLI (the same binary the Claude Agent SDK spawns) instead of a
// metered API key. Wire it up with, in runtime/agent/botd.properties:
//
//   LLM=CLAUDECODE
//   LLM_CTL=agent:llm:claude_code
//
// WHY THIS IS NOT A DROP-IN MODEL PROVIDER. chat_llm's other arms answer ONE
// model turn and hand any tool calls back for tool_loop to execute. Claude
// Code is an agent harness: it runs its OWN loop with ITS OWN tools and
// returns only when the work is done. So this command always answers
// kind:"text" - never kind:"tool_calls" - and tool_loop terminates on it.
// That is the honest mapping, not a limitation to route around: the turn was
// delegated wholesale, and the text IS the finished result.
//
// Which means the `tools` newbound passes are deliberately IGNORED - they
// name commands only this process can run. To let the delegate actually do
// agentic work, point it at newbound's own MCP server instead, which exposes
// every store command rather than the subset tool_loop happened to forward:
//
//   CLAUDE_CODE_MCP={"mcpServers":{"newbound":{"command":"./target/release/newbound","args":["mcp"]}}}
//   CLAUDE_CODE_PERMISSION_MODE=bypassPermissions
//
// AUTH IS THE POINT. `claude` authenticates from the OAuth login in ~/.claude,
// so calls draw on a Pro/Max subscription rather than API credits. An
// ANTHROPIC_API_KEY in the server's environment would silently override that
// and bill credits instead, so it is REMOVED from the child environment
// unless CLAUDE_CODE_ALLOW_API_KEY=on. Never pass --bare via CLAUDE_CODE_ARGS:
// it makes auth strictly API-key and never reads OAuth, defeating the purpose.

fn err_out(msg: &str) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("kind", "error");
    o.put_string("content", msg);
    o
}
fn text_result(msg: &str) -> DataObject {
    let mut a = DataObject::new();
    a.put_string("role", "assistant");
    a.put_string("content", msg);
    let mut o = DataObject::new();
    o.put_string("kind", "text");
    o.put_string("content", msg);
    o.put_object("assistant_message", a);
    o
}
fn opt(meta: &DataObject, key: &str, default: &str) -> String {
    match meta.try_get_string(key) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => default.to_string(),
    }
}
// ndata's try_from_string panics on well-formed JSON that is not an OBJECT;
// wrapping before parsing makes it total. Same guard chat_llm uses.
fn obj_from_str(s: &str) -> Option<DataObject> {
    match DataObject::try_from_string(&format!("{{\"a\":{}}}", s)) {
        Ok(w) => { let d = w.get_property("a"); if d.is_object() { Some(d.object()) } else { None } },
        Err(_) => None,
    }
}

let _ = tools;   // see the note above

let meta = (|| -> Option<DataObject> {
    let s = DataStore::globals().try_get_object("system").ok()?;
    let a = s.try_get_object("apps").ok()?;
    let g = a.try_get_object("agent").ok()?;
    g.try_get_object("runtime").ok()
})();
let meta = match meta {
    Some(m) => m,
    None => return err_out("the agent app is not configured: add `agent` to config.properties apps= and restart; runtime/agent/botd.properties holds the LLM settings"),
};

// ── flatten the conversation ─────────────────────────────────────────────
// Same convention chat_llm's own LLM_CTL text path uses, so a conversation
// reads identically whichever custom arm handles it: system messages become
// the system prompt, a lone user turn goes through verbatim (a plain ASK
// should not arrive wearing a "USER:" label), and anything longer gets role
// labels because otherwise it is unreadable.
let mut system = String::new();
let mut turns: Vec<(String, String)> = Vec::new();
for i in 0..messages.len() {
    let m = messages.get_object(i);
    let role = m.try_get_string("role").unwrap_or_default();
    let content = m.try_get_string("content").unwrap_or_default();
    if content.is_empty() { continue; }
    if role == "system" {
        if !system.is_empty() { system.push_str("\n\n"); }
        system.push_str(&content);
    } else {
        turns.push((role, content));
    }
}
let convo = if turns.len() == 1 && turns[0].0 == "user" {
    turns[0].1.clone()
} else {
    turns.iter().map(|(r, c)| format!("{}: {}\n\n", r.to_uppercase(), c))
         .collect::<String>()
};
let convo = convo.trim().to_string();
if convo.is_empty() { return err_out("claude_code: nothing to send - the conversation carried no user or assistant content"); }

// ── the resident context (docs/claudecode-arm.md) ────────────────────────
// A dev session gets its environment knowledge from CLAUDE.md; the inside
// delegate gets it HERE, at the layer that knows the environment. Injected
// only when the delegate has hands (CLAUDE_CODE_MCP set) - a bare oracle
// call has no tools for these rules to govern and every token is paid per
// turn. CLAUDE_CODE_CONTEXT=off suppresses it; the OWNER ADDENDUM (the
// agentprompt control) is the place to extend it.
if !opt(&meta, "CLAUDE_CODE_MCP", "").is_empty()
    && opt(&meta, "CLAUDE_CODE_CONTEXT", "on") != "off" {
    if !system.is_empty() { system.push_str("\n\n"); }
    system.push_str(concat!(
"WHERE YOU ARE\n",
"You are the frontier mind INSIDE a live Newbound instance, answering through its agent; the instance's own MCP server is your hands. Newbound is a peer-to-peer web platform: one live, journaled object graph where code is data - commands, flows, UI facets, and memories are records in the content-addressed store. Any tool guidance above about find_tools/call_command describes a different harness; YOUR tools are the MCP ones described next.\n\n",
"YOUR TOOLS\n",
"- Every store command is an MCP tool named lib-control-command (e.g. dev-code-read_command). Discover the rest with dev-code-search_commands; a command's desc is its manual.\n",
"- EVERY declared parameter must be passed on every call - there are no optional parameters.\n",
"- This is the LIVE instance, not a sandbox. Writes go only through platform commands, never by editing data/ files directly. Prefer the journaled, revertible edits: dev-code-patch_control_facet for UI facets, dev-code-patch_command_body or upsert_command for command bodies. Read before you write; destructive experiments belong in a disposable copy of the checkout, not here.\n\n",
"MEMORY\n",
"- Orient before nontrivial work: agent-archivist-recall searches this instance's federated memory - the brain plus every library's shipped manuals. Trust its staleness marks.\n",
"- Deposit with agent-archivist-remember when the user asks you to remember something, or when you learned something durable doing work they requested. Never file speculation.\n\n",
"THE DEEPER STORY\n",
"- The docs ride the agent repo checkout under docs/: understandingloop.md (the doctrine), perception-contract.md (the sensor contract), runbook-5b.md and claudecode-arm.md (the resident model service and this very bridge). Read them with your file tools if you have them, or ask the owner."));
}

// ── argv ─────────────────────────────────────────────────────────────────
let bin = opt(&meta, "CLAUDE_CODE_BIN", "claude");
let mut args: Vec<String> = vec![
    "-p".to_string(),
    "--output-format".to_string(), "json".to_string(),
    // A bridge call is stateless; persisting every turn would litter the
    // user's /resume picker with machine traffic.
    "--no-session-persistence".to_string(),
];
let model = opt(&meta, "CLAUDE_CODE_MODEL", "");
if !model.is_empty() { args.push("--model".to_string()); args.push(model); }
let effort = opt(&meta, "CLAUDE_CODE_EFFORT", "");
if !effort.is_empty() { args.push("--effort".to_string()); args.push(effort); }
if !system.is_empty() {
    // REPLACING the default system prompt is what makes this affordable:
    // Claude Code's own prompt plus CLAUDE.md discovery measured ~38k cache-
    // creation tokens on a trivial call, against ~200 with the prompt
    // replaced and built-ins off - a ~170x difference per turn. Set
    // CLAUDE_CODE_SYSTEM_MODE=append to keep Claude Code's prompt and pay for
    // it, which is what you want when the delegate is doing real agentic work.
    let mode = opt(&meta, "CLAUDE_CODE_SYSTEM_MODE", "replace");
    args.push(if mode == "append" { "--append-system-prompt".to_string() }
              else { "--system-prompt".to_string() });
    args.push(system.clone());
}
let mcp = opt(&meta, "CLAUDE_CODE_MCP", "");
if !mcp.is_empty() {
    // Accepts a path OR a literal JSON string. --strict-mcp-config keeps the
    // delegate off whatever servers the invoking user happens to have.
    args.push("--mcp-config".to_string()); args.push(mcp);
    args.push("--strict-mcp-config".to_string());
}
let perm = opt(&meta, "CLAUDE_CODE_PERMISSION_MODE", "");
if !perm.is_empty() { args.push("--permission-mode".to_string()); args.push(perm); }
// Whitespace-split, so a value containing spaces cannot be expressed here -
// the knobs above cover those.
for a in opt(&meta, "CLAUDE_CODE_ARGS", "").split_whitespace() { args.push(a.to_string()); }
// LAST, always: --tools is variadic and would swallow any non-flag argument
// that followed it. Empty disables the built-in toolset (MCP tools are
// unaffected); "default" restores it; or name them, e.g. "Bash,Read".
args.push("--tools".to_string());
args.push(opt(&meta, "CLAUDE_CODE_TOOLS", ""));

let mut cmd = Command::new(&bin);
cmd.args(&args)
   .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
let cwd = opt(&meta, "CLAUDE_CODE_CWD", "");
if !cwd.is_empty() { cmd.current_dir(&cwd); }
if opt(&meta, "CLAUDE_CODE_ALLOW_API_KEY", "off") != "on" {
    // The whole reason this arm exists: an inherited key would quietly move
    // billing from the subscription back onto metered credits.
    cmd.env_remove("ANTHROPIC_API_KEY");
    cmd.env_remove("ANTHROPIC_AUTH_TOKEN");
}

// Own process group, so the timeout path below can reap the whole tree.
// `claude` is a node launcher: killing only the direct child can leave the
// interpreter it spawned running, and on a long-lived server repeated
// timeouts would accumulate orphans.
#[cfg(unix)]
{
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

// The prompt rides STDIN, never argv: conversations outgrow ARG_MAX, and a
// positional prompt is also what the variadic flags above would eat.
let mut child = match cmd.spawn() {
    Ok(c) => c,
    Err(e) => return err_out(&format!("claude_code: could not run `{}` ({}). Set CLAUDE_CODE_BIN in runtime/agent/botd.properties to its full path, or install Claude Code.", bin, e)),
};
if let Some(mut si) = child.stdin.take() {
    let _ = si.write_all(convo.as_bytes());
}   // dropped here: stdin closes, which is what makes -p start work

let pid = child.id();
let secs = opt(&meta, "CLAUDE_CODE_TIMEOUT", "600").parse::<u64>().unwrap_or(600);
let (tx, rx) = channel();
thread::spawn(move || { let _ = tx.send(child.wait_with_output()); });
let out = match rx.recv_timeout(Duration::from_secs(secs)) {
    Ok(Ok(o)) => o,
    Ok(Err(e)) => return err_out(&format!("claude_code: reading the answer failed: {}", e)),
    Err(_) => {
        // Nothing else would ever reap it: an agent loop blocked forever is
        // worse than a reported failure.
        // A NEGATIVE pid signals the whole group (see process_group above).
        // `--` is REQUIRED: /bin/kill is not the shell builtin and parses a
        // bare `-1234` as options, silently leaving the tree running.
        #[cfg(unix)]
        let _ = Command::new("kill").arg("-9").arg("--").arg(format!("-{}", pid)).status();
        #[cfg(not(unix))]
        let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
        return err_out(&format!("claude_code: no answer in {}s, killed pid {} (raise CLAUDE_CODE_TIMEOUT)", secs, pid));
    }
};

let body = String::from_utf8_lossy(&out.stdout).to_string();
let errtail: String = String::from_utf8_lossy(&out.stderr).chars().rev().take(600)
    .collect::<String>().chars().rev().collect();
let root = match obj_from_str(&body) {
    Some(r) => r,
    None => {
        // --output-format json failed to produce json at all: a login prompt,
        // a usage-limit notice, or a crash. stderr is where it says which.
        return err_out(&format!("claude_code: `{}` returned no JSON (exit {:?}). stderr: {}",
            bin, out.status.code(),
            if errtail.trim().is_empty() { body.chars().take(600).collect::<String>() } else { errtail }));
    }
};

if root.try_get_boolean("is_error").unwrap_or(false) {
    let sub = root.try_get_string("subtype").unwrap_or_else(|_| "unknown".to_string());
    let detail = root.try_get_string("result").unwrap_or_default();
    return err_out(&format!("claude_code failed ({}): {}", sub,
        if detail.is_empty() { errtail } else { detail }));
}

let result = root.try_get_string("result").unwrap_or_default();
if result.trim().is_empty() {
    let stop = root.try_get_string("stop_reason").unwrap_or_else(|_| "none".to_string());
    return err_out(&format!("claude_code returned no text (stop_reason: {}, subtype: {})", stop,
        root.try_get_string("subtype").unwrap_or_else(|_| "none".to_string())));
}

let mut answer = text_result(&result);
// Additive, and chat_llm passes the envelope through untouched: what a turn
// notionally cost is the number that decides whether this arm is worth
// running at all. On a subscription it is charged to the plan's allowance
// rather than billed, but it still measures what was spent.
if let Ok(c) = root.try_get_float("total_cost_usd") { answer.put_float("cost_usd", c); }
if let Ok(n) = root.try_get_int("num_turns") { answer.put_int("num_turns", n); }
if let Ok(s) = root.try_get_string("session_id") { answer.put_string("session_id", &s); }
answer

}
