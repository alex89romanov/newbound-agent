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
//   * (retired 2026-08-19, harvest H1b) the LLM_CAPTURE_DIR Q/A text
//     files. Capture lives at the chat_llm seam now - LLM_CAPTURE=on
//     records every arm's traffic as message records plus an
//     ID-referencing capture row, these two-message conversations
//     included. Loose prompt/answer text beside a managed message
//     store would be an orphan bank.
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

out
