// ask_llm IS chat_llm with a two-message conversation and no tools. It used
// to carry its own full copy of the provider resolver — ~120 lines, marked
// "MIRRORED - keep the two in sync", and by the time anyone looked they had
// already drifted: a `stop` sequence here and not there, temperature 0.4 vs
// 0.2, five retries vs four, with nothing recording which differences were
// deliberate. Delegating removes the second copy structurally instead of
// promising to maintain it.
//
// What survives from the old body on purpose:
//   * the "ERROR: " failure prefix. agent.archivist.consolidate KEEPS its
//     queue when a call fails, and it detects failure by that prefix — a
//     clean error string would silently drain the queue instead.
//   * optional prompt/answer capture, now OFF unless LLM_CAPTURE_DIR is set
//     (it used to write every prompt to LLM_RAW/ unconditionally: unbounded,
//     and prompts carry whatever facet source the agent had just read).
//
// What does NOT survive: `stop: ["Observation:"]`. It was a ReAct-era hack,
// tool_loop has used native tool_calls through chat_llm for a long time, and
// it silently truncated any answer containing the word "Observation:".

let system = match system_prompt.is_string() {
    true => system_prompt.string(),
    _ => String::new(),
};

let mut messages = DataArray::new();
if !system.trim().is_empty() {
    let mut m = DataObject::new();
    m.put_string("role", "system");
    m.put_string("content", &system);
    messages.push_object(m);
}
let mut m = DataObject::new();
m.put_string("role", "user");
m.put_string("content", &prompt);
messages.push_object(m);

let res = chat_llm(messages, DataArray::new());

let kind = res.try_get_string("kind").unwrap_or_else(|_| "error".to_string());
let out = match kind.as_str() {
    "text" => res.try_get_string("content").unwrap_or_default(),
    // No tools were offered, so a tool_calls answer means the provider
    // ignored that. Report it rather than returning an empty string.
    "tool_calls" => {
        let c = res.try_get_string("content").unwrap_or_default();
        if c.trim().is_empty() {
            "ERROR: the provider answered with tool calls although no tools were offered".to_string()
        } else { c }
    },
    _ => format!("ERROR: {}", res.try_get_string("content")
        .unwrap_or_else(|_| "LLM call failed".to_string())),
};

// Opt-in capture: LLM_CAPTURE_DIR=<path> in runtime/agent/botd.properties.
// Logs the answer as well as the prompt — the old unconditional version had
// dropped the answer half, so it was write-only prompt capture.
let capture = (|| -> Option<String> {
    let s = DataStore::globals().try_get_object("system").ok()?;
    let a = s.try_get_object("apps").ok()?;
    let g = a.try_get_object("agent").ok()?;
    let r = g.try_get_object("runtime").ok()?;
    match r.try_get_string("LLM_CAPTURE_DIR") {
        Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
        _ => None,
    }
})();
if let Some(dir) = capture {
    let d = PathBuf::from(&dir);
    if std::fs::create_dir_all(&d).is_ok() {
        let stamp = time();
        let _ = std::fs::write(d.join(format!("Q{}.txt", stamp)),
                               format!("{}\n\n{}", &system, &prompt));
        let _ = std::fs::write(d.join(format!("A{}.txt", stamp)), &out);
    }
}

out
