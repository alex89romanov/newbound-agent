use ndata::dataobject::DataObject;
use flowlang::command::Command;
use flowlang::datastore::DataStore;
use crate::agent::llm::chat_llm::chat_llm;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["prompt"] {
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
        let arg_0: String = o.get_string("prompt");
        tool_loop(arg_0)
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

pub fn tool_loop(prompt: String) -> DataObject {
// INITIALIZE AGENT MEMORY
{
  let mut g = DataStore::globals();
  if !g.has("AGENT_MEMORY") { g.put_object("AGENT_MEMORY", DataObject::new()); }
}

// --- helpers ----------------------------------------------------------------
fn extract_compile_errors(raw: &str) -> String {
  // Compile failures arrive as {"status":"err","kind":"compile_error","msg":"error[E0382]: ..."}
  // serialized to JSON, so newlines inside msg are escaped literals. Unescape, then keep
  // everything from the first rustc error marker onward %E2%80%94 dev.dev.compile already
  // pre-filters stderr to error blocks, so nothing after that point is noise.
  let expanded = raw.replace("\\n", "\n").replace("\\\"", "\"");
  match expanded.find("error[").or_else(|| expanded.find("error:")) {
    Some(i) => expanded[i..].to_string(),
    None => expanded
  }
}

fn clamp(s: &str, budget: usize) -> String {
  if s.len() <= budget { return s.to_string(); }
  // Head-biased: rustc's FIRST error is the root cause; later ones usually cascade.
  let head_len = budget * 2 / 3;
  let tail_len = budget - head_len;
  let mut h = head_len; while !s.is_char_boundary(h) { h -= 1; }
  let mut t = s.len() - tail_len; while !s.is_char_boundary(t) { t += 1; }
  format!("{}\n...[{} bytes omitted]...\n{}", &s[..h], t - h, &s[t..])
}
// -----------------------------------------------------------------------------

let sys_prompt = {
  // The PLATFORM-KNOWLEDGE CORE is store-resident (agent.prompts `prompt`
  // facet - docs/prompting.md in the bench repo): ONE journaled source of
  // truth shared with the bench notebook's agentloop. A missing core
  // FAILS LOUD - a silently thin prompt regresses the confabulation
  // lesson - and code + data ride the same repo, so a pull cannot
  // desync them.
  let pstore = DataStore::new();
  // Resolve the control id from the library index directly. An api-struct
  // accessor bakes in whichever library layout api.rs was last generated
  // from (that is the E0609 the dev.code move produced), and a cross-library
  // command call would tie this to dev.editcontrol surviving the cruft
  // audit. The index is the thing that actually holds the answer.
  let pid = {
    let list = pstore.get_data("agent", "controls").get_object("data").get_array("list");
    let mut found = String::new();
    for i in 0..list.len() {
      let it = list.get_object(i);
      if it.get_string("name") == "prompts" { found = it.get_string("id"); break; }
    }
    found
  };
  let missing = !pstore.exists("agent", &pid) || {
    let d = pstore.get_data("agent", &pid).get_object("data");
    !d.has("prompt") || d.get_string("prompt").trim().is_empty()
  };
  if missing {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", "agent.prompts `prompt` facet unavailable - the curriculum is store-resident (docs/prompting.md); pull canon");
    return o;
  }
  let core = pstore.get_data("agent", &pid).get_object("data").get_string("prompt");
  let shell = r#"

The Newbound Commands in the `code` control of the `dev` library are available to you as native tools. Call them through the tool-calling interface - do not describe tool calls in text. Tool names are in the format library-control-command (e.g. dev-code-read_command).

Sub-Agent Recursion
Inside an evaluate_rust script you can call the LLM itself, for map-reduce over data too large for this conversation:
```Rust
use flowlang::datastore::DataStore;
use crate::agent::llm::ask_llm::ask_llm;
use ndata::data::Data;
let store = DataStore::new();
let prompt = format!("Summarize this data: {}", store.get_data("runtime","exported_data").get_object("data").to_string());
let summary = ask_llm(prompt, Data::DString("Answer with the summary only.".to_string()));
let _ = std::fs::remove_file(store.get_data_file("runtime", "exported_data"));
summary
```
Use this to build multi-step pipelines, perform map-reduce tasks, or spin up sub-agents, returning only the final, concise result.
"#;
  core + shell
};
// MEMORY INDEX (docs/memory.md, federated per docs/one-memory-cycle.md A1):
// every library's controls, any control carrying a memory facet is a domain,
// labeled lib.ctl - the control's manual, one read away. kb's controls are
// domains by definition and always listed. One-shot loop, so no staleness
// machinery: the curriculum says to verify source hashes on read.
let mut sys_prompt = sys_prompt;
{
  let mstore = DataStore::new();
  let mut idx = String::from("\n\nMEMORY INDEX (recall with dev-code-read_control_facet lib:<lib> ctl:<ctl> facet:\"memory\"):\n");
  let mut any = false;
  let mut libs: Vec<String> = Vec::new();
  if let Ok(rd) = std::fs::read_dir(&mstore.root) {
    for e in rd.flatten() {
      if e.path().is_dir() {
        if let Ok(n) = e.file_name().into_string() { libs.push(n); }
      }
    }
  }
  libs.sort();
  for lib in libs {
    if !mstore.exists(&lib, "controls") { continue; }
    let list = mstore.get_data(&lib, "controls").get_object("data").get_array("list");
    let is_brain = lib == "kb";
    for i in 0..list.len() {
      let item = list.get_object(i);
      if !item.has("name") || !item.has("id") { continue; }
      let name = item.get_string("name");
      let id = item.get_string("id");
      if !mstore.exists(&lib, &id) { continue; }
      let dd = mstore.get_data(&lib, &id).get_object("data");
      if !dd.has("memory") && !is_brain { continue; }
      let desc = if dd.has("desc") { dd.get_string("desc") } else { String::new() };
      let tags = if dd.has("tags") { dd.get_string("tags") } else { String::new() };
      let n: i64 = if dd.has("memory") {
        match ndata::dataobject::DataObject::try_from_string(&format!("{{\"a\":{}}}", dd.get_string("memory"))) {
          Ok(w) => match w.try_get_array("a") { Ok(a) => a.len() as i64, Err(_) => -1 },
          Err(_) => -1,
        }
      } else { 0 };
      let count = if n >= 0 { format!(" ({})", n) } else { String::new() };
      let tagpart = if tags.is_empty() { String::new() } else { format!(" [{}]", tags) };
      idx.push_str(&format!("- {}.{}{}{} - {}\n", lib, name, count, tagpart, desc));
      any = true;
    }
  }
  if any { sys_prompt.push_str(&idx); }
}

// Build the OpenAI-format tools array from the MCP descriptors
// The write API lives at dev.code (CONTRACT 6.6). Look the command up by
// NAME: an api-struct accessor bakes in whichever library layout api.rs
// was last generated from, so it breaks on regeneration after a move;
// Command::lookup resolves against the store at runtime and compiles
// against any generation of api.rs.
let mcp_tools = {
    let mut d = DataObject::new();
    d.put_string("lib", "dev");
    d.put_string("ctl", "code");
    Command::lookup("dev", "code", "list_commands").execute(d)
        .expect("dev.code.list_commands failed").get_array("a")
};
let mut tools = DataArray::new();
for t in mcp_tools.objects() {
  let t = t.object();
  if !t.has("name") { continue; }
  let mut f = DataObject::new();
  f.put_string("name", &t.get_string("name"));
  if t.has("description") { f.put_string("description", &t.get_string("description")); }
  if t.has("inputSchema") { f.put_object("parameters", t.get_object("inputSchema")); }
  else if t.has("input_schema") { f.put_object("parameters", t.get_object("input_schema")); }
  else {
    let mut s = DataObject::new();
    s.put_string("type", "object");
    s.put_object("properties", DataObject::new());
    f.put_object("parameters", s);
  }
  let mut w = DataObject::new();
  w.put_string("type", "function");
  w.put_object("function", f);
  tools.push_object(w);
}

// Seed the conversation
let mut messages = DataArray::new();
let mut m = DataObject::new();
m.put_string("role", "system");
m.put_string("content", &sys_prompt);
messages.push_object(m);
let mut m = DataObject::new();
m.put_string("role", "user");
m.put_string("content", &prompt);
messages.push_object(m);

let mut final_answer = String::new();
let mut iterations = 0;
let max_iterations = 24;
let mut dirty = String::new();    // "lib-ctl-cmd" modified but not yet executed
let mut nudged = false;           // one verify-nudge per dirty cycle
let mut last_sig = String::new(); // repeated-identical-call detection

let mut consecutive_failures = 0;
let mut repeat_count = 0;
let mut last_err_head = String::new();

loop {
    if iterations >= max_iterations {
        final_answer = "Error: Maximum tool loop iterations exceeded.".to_string();
        break;
    }
    iterations += 1;

    let resp = chat_llm(messages.clone(), tools.clone());
    let kind = resp.get_string("kind");
    println!("TL[{}] kind={}", iterations, kind);
  
    if kind == "error" {
        final_answer = format!("LLM backend error: {}", resp.get_string("content"));
        break;
    }

    if kind == "text" {
        let content = resp.get_string("content");
        println!("TL[{}] text {} chars; dirty='{}' nudged={}", iterations, content.len(), dirty, nudged);
        // Verify nudge: the workflow's EXECUTE step, enforced once per modification cycle.
        if !dirty.is_empty() && !nudged {
            messages.push_object(resp.get_object("assistant_message"));
            let mut m = DataObject::new();
            m.put_string("role", "user");
            m.put_string("content", &format!("You modified `{}` but never executed it. Follow your workflow: verify with dev-code-invoke_command (or dev-code-evaluate_rust for a test harness) before giving your final answer. If you already verified it another way, state how and answer.", dirty));
            messages.push_object(m);
            nudged = true;
            continue;
        }
        final_answer = content;
        break;
    }

    // kind == "tool_calls"
    messages.push_object(resp.get_object("assistant_message"));
    let calls = resp.get_array("tool_calls");
    for c in calls.objects() {
        let c = c.object();
        let name = c.get_string("name");
        let args_str = c.get_string("arguments");
        let call_id = c.get_string("id");
        let sig = format!("{}::{}", &name, &args_str);
        let args_head: String = args_str.chars().take(200).collect();
        println!("TL[{}] -> {} args: {}", iterations, name, args_head.replace('\n', " "));

        let observation = if sig == last_sig {
            repeat_count += 1;
            if repeat_count >= 3 {
                final_answer = format!("Aborting: the model repeated the identical call to {} {} times despite being told the result will not change. Last real result from this tool is above in the transcript. This usually means the underlying operation is failing and the model has no viable alternative.", name, repeat_count + 1);
                break;
            }
            "You already called this exact tool with these exact arguments; the result will not change. Adjust the arguments or take a different approach.".to_string()
        } else {
            repeat_count = 0;
            last_sig = sig;
            let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                DataObject::from_string(&args_str)
            }));
            match parsed {
                Err(_) => format!("Error: tool arguments were not a valid JSON object: {}", &args_str),
                Ok(arguments) => {
                    let parts: Vec<&str> = name.split('-').collect();
                    if parts.len() != 3 {
                        format!("Error: Invalid tool name '{}'. Must be in the format 'library-control-command'", name)
                    } else {
                        let execution_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let cmd = Command::lookup(parts[0], parts[1], parts[2]);
                            cmd.cast_params(arguments.clone());
                            cmd.execute(arguments)
                        }));
                        match execution_result {
                            Ok(Ok(v)) => {
                                // Unwrap the wrapper's {"a": ...} packaging when that's the
                                // whole payload, so the model sees the actual result.
                                match v.try_get_object("a") {
                                    Ok(inner) => inner.to_string(),
                                    _ => v.to_string()
                                }
                            },
                            Ok(Err(e)) => format!("Tool Error: {:?}", e),
                            Err(panic_err) => {
                                let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                                    s.to_string()
                                } else if let Some(s) = panic_err.downcast_ref::<String>() {
                                    s.clone()
                                } else {
                                    "Unknown internal panic".to_string()
                                };
                                // Honest diagnosis: compile errors are compile errors,
                                // not "check your formatting".
                                if msg.contains("error[") || msg.trim_start().starts_with("error") {
                                    format!("Compilation failed. Fix the code and try again:\n{}", msg)
                                } else {
                                    format!("Tool panicked: '{}'. This is a runtime failure in the tool, not a formatting problem. Reconsider the arguments or approach before retrying.", msg)
                                }
                            }
                        }
                    }
                }
            }
        };

        // Dirty-tracking for the verify nudge
        if name.ends_with("upsert_command") || name.ends_with("patch_command_body") {
            nudged = false;
            dirty = name.clone();
            if let Ok(a) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| DataObject::from_string(&args_str))) {
                if a.has("lib") && a.has("ctl") && a.has("cmd") {
                    dirty = format!("{}-{}-{}", a.get_string("lib"), a.get_string("ctl"), a.get_string("cmd"));
                }
            }
        }
        if name.ends_with("invoke_command") || name.ends_with("evaluate_rust") || name.ends_with("delete_command") {
            dirty = String::new();
            nudged = false;
        }

        // Budget the observation: rustc-bearing tools get error-extraction @ 8KB, others 6KB
        let obs = if name.ends_with("upsert_command") || name.ends_with("patch_command_body") {
            clamp(&extract_compile_errors(&observation), 8000)
        } else {
            clamp(&observation, 6000)
        };
      
        let obs_head: String = obs.chars().take(300).collect();
        println!("TL[{}] <- {} [{}B] {}", iterations, name, obs.len(), obs_head.replace('\n', " | "));

        let mut toolmsg = DataObject::new();
        toolmsg.put_string("role", "tool");
        toolmsg.put_string("tool_call_id", &call_id);
        toolmsg.put_string("content", &obs);
        messages.push_object(toolmsg);
      
        let is_failure = obs.contains("was not registered")
            || obs.contains("\"status\":\"err\"")
            || obs.starts_with("error")
            || obs.starts_with("Tool Error")
            || obs.starts_with("Tool panicked")
            || obs.starts_with("Compilation failed")
            || obs.contains("You already called this exact tool");
        if is_failure {
            let err_head: String = obs.chars().take(120).collect();
            if err_head == last_err_head {
                consecutive_failures += 1;
            } else {
                consecutive_failures = 1;
                last_err_head = err_head;
            }
        } else {
            consecutive_failures = 0;
            last_err_head = String::new();
        }
        if consecutive_failures >= 3 {
            final_answer = format!("Aborting: the same failure occurred {} times in a row from {} with no change in outcome: {}", consecutive_failures, name, obs.chars().take(500).collect::<String>());
            break;
        }
    }
    if !final_answer.is_empty() { break; }
}

// the archivist's intake (docs/memory.md): a completed loop is a turn.
// Panic-shielded fire-and-forget - logging must never cost the answer.
let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let logcmd = Command::lookup("agent", "archivist", "log_turn");
    let mut la = DataObject::new();
    la.put_string("venue", "tool_loop");
    la.put_string("ask", &prompt.chars().take(4000).collect::<String>());
    la.put_string("reply", &final_answer.chars().take(4000).collect::<String>());
    la.put_string("tools", "");
    la.put_string("author", "tool_loop");
    let _ = logcmd.execute(la);
}));

let mut response = DataObject::new();
response.put_string("msg", &final_answer);
response.put_array("messages", messages);
response
}
