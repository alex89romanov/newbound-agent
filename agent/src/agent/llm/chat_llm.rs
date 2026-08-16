use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use ndata::data::Data;
use std::collections::HashMap;

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
        chat_llm(arg_0, arg_1)
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

pub fn chat_llm(messages: DataArray, tools: DataArray) -> DataObject {
// ── THE provider engine. ask_llm is a thin wrapper over this command, so
// there is no second copy of the resolver to keep in sync.
//
// `LLM` in runtime/agent/botd.properties picks an ARM; each arm names the
// DIALECT it actually speaks on the wire:
//
//   VLLM      -> openai     OpenAI chat-completions (+ vLLM chat_template_kwargs)
//   OPENAI    -> openai     also LM Studio / llama.cpp / Groq / OpenRouter via _URL
//   ANTHROPIC -> anthropic  NATIVE /v1/messages     (see the thinking note below)
//   GEMINI    -> gemini     NATIVE generateContent  (see the safety note below)
//   OLLAMA    -> ollama     NATIVE /api/chat        (keep_alive is not expressible
//                                                    on the compat endpoint)
//   other     -> custom     LLM_CTL=lib:ctl:cmd
//
// The "they all speak OpenAI, so one request path serves them" shortcut this
// replaced was wrong where it counted. Gemini's compat endpoint has nowhere
// to put safetySettings, so its DEFAULT filters applied; a filtered answer
// arrives with no `content`, which the OpenAI parser reported as "unexpected
// JSON structure". An agent that ships code and stack traces all day was
// being silently blocked and then misdiagnosed. The native dialect sends
// safetySettings BLOCK_NONE and NAMES the block when one happens.
//
// Anthropic is that same lesson a second time. The Messages API is not
// chat-completions wearing a different key: the system prompt is a top-level
// param, max_tokens is REQUIRED, tools carry `input_schema` rather than
// `parameters`, and tool traffic is tool_use / tool_result CONTENT BLOCKS
// instead of a role:"tool" message. Pointing the OPENAI arm at it with a
// header override 400s on the first call, and a safety decline arrives as a
// 200 with empty content - the same silent-misdiagnosis shape as Gemini's
// filtered answer. One demand no other dialect makes: with thinking on (the
// DEFAULT on Opus 5) an assistant turn that called a tool is rejected on the
// NEXT request unless its thinking block is replayed verbatim, so those
// blocks ride the assistant_message the caller already keeps.
//
// Everything below normalizes INTO the OpenAI-shaped result the callers
// already speak ({kind, content|tool_calls, assistant_message}), so
// tool_loop and the browser agentloop are dialect-agnostic.

fn err_out(msg: &str) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("kind", "error");
    o.put_string("content", msg);
    o
}
fn assistant_msg(content: &str, replay: Option<DataArray>) -> DataObject {
    let mut a = DataObject::new();
    a.put_string("role", "assistant");
    a.put_string("content", content);
    if let Some(r) = replay { a.put_array("tool_calls", r); }
    a
}
fn text_result(msg: &str) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("kind", "text");
    o.put_string("content", msg);
    o.put_object("assistant_message", assistant_msg(msg, None));
    o
}
fn need(meta: &DataObject, key: &str, arm: &str) -> Result<String, String> {
    match meta.try_get_string(key) {
        Ok(v) if !v.trim().is_empty() => Ok(v.trim().to_string()),
        _ => Err(format!("LLM arm {} needs {} - set it in runtime/agent/botd.properties and restart. (LLM= selects VLLM | OPENAI | ANTHROPIC | GEMINI | OLLAMA; any other value uses LLM_CTL=lib:ctl:cmd.)", arm, key)),
    }
}
fn opt(meta: &DataObject, key: &str, default: &str) -> String {
    match meta.try_get_string(key) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => default.to_string(),
    }
}
// <ARM>_HEADERS: newline- or comma-separated `Name: value` pairs, applied
// AFTER the arm's own auth header so an explicit entry always wins. This is
// the escape hatch for every scheme not special-cased here — Azure OpenAI
// wants `api-key: <key>` rather than Bearer (plus ?api-version= on the URL),
// OpenRouter likes HTTP-Referer/X-Title. Config, not a code change.
// The key may be spelled <ARM>_KEY or <ARM>_API_KEY. Older configs used
// _API_KEY, and there is no reason to make anyone edit a working file.
fn need_key(meta: &DataObject, arm: &str) -> Result<String, String> {
    for k in [format!("{}_KEY", arm), format!("{}_API_KEY", arm)] {
        if let Ok(v) = meta.try_get_string(&k) {
            if !v.trim().is_empty() { return Ok(v.trim().to_string()); }
        }
    }
    Err(format!("LLM arm {} needs {}_KEY (or {}_API_KEY) - set it in runtime/agent/botd.properties and restart. (LLM= selects VLLM | OPENAI | ANTHROPIC | GEMINI | OLLAMA; any other value uses LLM_CTL=lib:ctl:cmd.)", arm, arm, arm))
}
fn opt_key(meta: &DataObject, arm: &str) -> String {
    need_key(meta, arm).unwrap_or_default()
}
fn extra_headers(meta: &DataObject, arm: &str) -> Vec<(String, String)> {
    let raw = opt(meta, &format!("{}_HEADERS", arm), "");
    let mut out: Vec<(String, String)> = Vec::new();
    for line in raw.split(|c| c == '\n' || c == ',') {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(i) = line.find(':') {
            let k = line[..i].trim().to_string();
            let v = line[i + 1..].trim().to_string();
            if !k.is_empty() { out.push((k, v)); }
        }
    }
    out
}
// OpenAI puts tool-call arguments in a JSON *string*; Gemini and Ollama use
// an object. Normalize to the string form the callers already parse.
fn args_to_string(d: &Data) -> String {
    if d.is_string() { d.string() }
    else if d.is_object() { d.object().to_string() }
    else { "{}".to_string() }
}
// ndata's try_from_string is NOT panic-safe. It returns Err only for
// MALFORMED json; for well-formed json that is not an OBJECT — an array, a
// string, a number, a bool, null — it reaches DataObject::from_json, whose
// `.expect("DataObject::from_json requires a JSON object Value")` PANICS.
// A tool answering with a JSON ARRAY (search_commands does) therefore took
// down the whole chat_llm call. Wrapping in {"a": ...} before parsing makes
// it total, and is already the house idiom (see dev.code.remember).
fn parse_json(s: &str) -> Option<Data> {
    DataObject::try_from_string(&format!("{{\"a\":{}}}", s))
        .ok().map(|w| w.get_property("a"))
}
fn obj_from_str(s: &str) -> Option<DataObject> {
    match parse_json(s) {
        Some(d) if d.is_object() => Some(d.object()),
        _ => None,
    }
}
fn args_to_object(s: &str) -> DataObject {
    obj_from_str(s).unwrap_or_else(DataObject::new)
}

// Gemini's Schema is an OpenAPI subset and it REJECTS unknown keys, so an
// MCP inputSchema cannot be forwarded verbatim ($schema and
// additionalProperties are the usual offenders). Keep only what Schema
// defines, recursively.
fn gemini_schema(o: DataObject) -> DataObject {
    let mut out = DataObject::new();
    for (k, v) in o.objects() {
        match k.as_str() {
            "type" | "format" | "description" | "nullable" | "enum" => out.set_property(&k, v.clone()),
            "required" => out.set_property(&k, v.clone()),
            "items" => { if v.is_object() { out.put_object("items", gemini_schema(v.object())); } },
            "properties" => {
                if v.is_object() {
                    let mut props = DataObject::new();
                    for (pk, pv) in v.object().objects() {
                        if pv.is_object() { props.put_object(&pk, gemini_schema(pv.object())); }
                    }
                    out.put_object("properties", props);
                }
            },
            _ => {}   // $schema, additionalProperties, examples, ... dropped
        }
    }
    if !out.has("type") { out.put_string("type", "object"); }
    // Gemini REJECTS an array with no `items` ("...properties[params].items:
    // missing field", INVALID_ARGUMENT) where every OpenAI-compatible server
    // tolerates it. There is nothing to derive the element type FROM:
    // newbound's declared param types are just `JSONArray`, which
    // flowlang's describe.rs maps to a bare {"type":"array"}.
    // `object` is right for the arrays the agent actually uses
    // (upsert_command.params, chat_llm messages/tools) and wrong for the
    // string lists on app.app.newlib / security.setuser
    // (readers/writers/groups) - the per-tool description still documents
    // those, and they are not on the agent's hot path. The real fix is
    // element types in the platform's param declarations; that is a schema
    // change to every command record and the owner's call.
    let is_array = matches!(out.try_get_string("type"), Ok(ref t) if t == "array");
    if is_array && !out.has("items") {
        let mut it = DataObject::new();
        it.put_string("type", "object");
        out.put_object("items", it);
    }
    out
}

fn build_gemini_payload(messages: &DataArray, tools: &DataArray,
                        temperature: f64, max_tokens: i64) -> DataObject {
    let mut payload = DataObject::new();
    let mut contents = DataArray::new();
    let mut system = String::new();
    // Gemini's functionResponse is keyed by the function NAME, not by a call
    // id (it has no call ids at all — we synthesize them on the way out), so
    // walking forward we remember which id belonged to which name.
    let mut id_names: HashMap<String, String> = HashMap::new();

    for i in 0..messages.len() {
        let m = messages.get_object(i);
        let role = m.try_get_string("role").unwrap_or_default();
        let content = m.try_get_string("content").unwrap_or_default();

        if role == "system" {
            if !system.is_empty() { system.push_str("\n\n"); }
            system.push_str(&content);
            continue;
        }
        if role == "tool" {
            let id = m.try_get_string("tool_call_id").unwrap_or_default();
            let name = id_names.get(&id).cloned().unwrap_or_else(|| "tool".to_string());
            // response must be an OBJECT. A tool answering with an ARRAY
            // (search_commands) or with plain text gets WRAPPED rather than
            // dropped — and an array keeps its STRUCTURE instead of being
            // flattened to a string, so the model can still read the rows.
            let inner = match parse_json(&content) {
                Some(d) if d.is_object() => d.object(),
                Some(d) => { let mut w = DataObject::new(); w.set_property("result", d); w },
                None => { let mut w = DataObject::new(); w.put_string("result", &content); w }
            };
            let mut fr = DataObject::new();
            fr.put_string("name", &name);
            fr.put_object("response", inner);
            let mut part = DataObject::new();
            part.put_object("functionResponse", fr);
            let mut parts = DataArray::new();
            parts.push_object(part);
            let mut c = DataObject::new();
            c.put_string("role", "user");
            c.put_array("parts", parts);
            contents.push_object(c);
            continue;
        }

        let mut parts = DataArray::new();
        if !content.is_empty() {
            let mut p = DataObject::new();
            p.put_string("text", &content);
            if role == "assistant" {
                if let Ok(s) = m.try_get_string("thought_signature") {
                    if !s.is_empty() { p.put_string("thoughtSignature", &s); }
                }
            }
            parts.push_object(p);
        }
        if role == "assistant" {
            if let Ok(calls) = m.try_get_array("tool_calls") {
                for c in calls.objects() {
                    let c = c.object();
                    let f = c.get_object("function");
                    let name = f.get_string("name");
                    if c.has("id") { id_names.insert(c.get_string("id"), name.clone()); }
                    let mut fc = DataObject::new();
                    fc.put_string("name", &name);
                    fc.put_object("args", args_to_object(&args_to_string(&f.get_property("arguments"))));
                    let mut p = DataObject::new();
                    p.put_object("functionCall", fc);
                    // Echo the signature Gemini gave us for THIS call, or it
                    // rejects the replayed turn outright.
                    if let Ok(s) = c.try_get_string("thought_signature") {
                        if !s.is_empty() { p.put_string("thoughtSignature", &s); }
                    }
                    parts.push_object(p);
                }
            }
        }
        if parts.len() == 0 { continue; }
        let mut c = DataObject::new();
        c.put_string("role", if role == "assistant" { "model" } else { "user" });
        c.put_array("parts", parts);
        contents.push_object(c);
    }

    if !system.is_empty() {
        let mut t = DataObject::new();
        t.put_string("text", &system);
        let mut parts = DataArray::new();
        parts.push_object(t);
        let mut si = DataObject::new();
        si.put_array("parts", parts);
        payload.put_object("system_instruction", si);
    }
    payload.put_array("contents", contents);

    if tools.len() > 0 {
        let mut decls = DataArray::new();
        for t in tools.objects() {
            let t = t.object();
            let f = if t.has("function") { t.get_object("function") } else { t.clone() };
            if !f.has("name") { continue; }
            let mut d = DataObject::new();
            d.put_string("name", &f.get_string("name"));
            if f.has("description") { d.put_string("description", &f.get_string("description")); }
            if let Ok(p) = f.try_get_object("parameters") { d.put_object("parameters", gemini_schema(p)); }
            decls.push_object(d);
        }
        let mut tool = DataObject::new();
        tool.put_array("functionDeclarations", decls);
        let mut ta = DataArray::new();
        ta.push_object(tool);
        payload.put_array("tools", ta);
    }

    let mut gen = DataObject::new();
    gen.put_float("temperature", temperature);
    gen.put_int("maxOutputTokens", max_tokens);
    payload.put_object("generationConfig", gen);

    // The reason this dialect exists. Without it Gemini's defaults block a
    // working agent's ordinary traffic and the compat layer gives no way to
    // say otherwise.
    let mut safety = DataArray::new();
    for cat in ["HARM_CATEGORY_HARASSMENT", "HARM_CATEGORY_HATE_SPEECH",
                "HARM_CATEGORY_SEXUALLY_EXPLICIT", "HARM_CATEGORY_DANGEROUS_CONTENT"] {
        let mut s = DataObject::new();
        s.put_string("category", cat);
        s.put_string("threshold", "BLOCK_NONE");
        safety.push_object(s);
    }
    payload.put_array("safetySettings", safety);
    payload
}

fn build_ollama_payload(messages: &DataArray, tools: &DataArray, model: &str,
                        temperature: f64, max_tokens: i64, keep_alive: &str) -> DataObject {
    let mut payload = DataObject::new();
    payload.put_string("model", model);

    let mut out = DataArray::new();
    for i in 0..messages.len() {
        let m = messages.get_object(i);
        let role = m.try_get_string("role").unwrap_or_default();
        let mut n = DataObject::new();
        n.put_string("role", &role);
        n.put_string("content", &m.try_get_string("content").unwrap_or_default());
        if role == "assistant" {
            if let Ok(calls) = m.try_get_array("tool_calls") {
                let mut oc = DataArray::new();
                for c in calls.objects() {
                    let c = c.object();
                    let f = c.get_object("function");
                    let mut nf = DataObject::new();
                    nf.put_string("name", &f.get_string("name"));
                    // Ollama wants arguments as an OBJECT, not a JSON string.
                    nf.put_object("arguments", args_to_object(&args_to_string(&f.get_property("arguments"))));
                    let mut w = DataObject::new();
                    w.put_object("function", nf);
                    oc.push_object(w);
                }
                if oc.len() > 0 { n.put_array("tool_calls", oc); }
            }
        }
        out.push_object(n);
    }
    payload.put_array("messages", out);
    payload.put_boolean("stream", false);
    // 0 unloads the model as soon as the call finishes — your original
    // /api/generate code chose that deliberately, and it matters on a shared
    // GPU. Set OLLAMA_KEEP_ALIVE=5m (or any duration Ollama accepts) to keep
    // it resident and pay the load cost only once.
    match keep_alive.parse::<i64>() {
        Ok(n) => payload.put_int("keep_alive", n),
        Err(_) => payload.put_string("keep_alive", keep_alive),
    }
    if tools.len() > 0 { payload.put_array("tools", tools.clone()); }

    let mut options = DataObject::new();
    options.put_float("temperature", temperature);
    options.put_int("num_predict", max_tokens);
    payload.put_object("options", options);
    payload
}

// name + arguments-as-string pairs -> (normalized calls, replay tool_calls)
fn pack_calls(raw: Vec<(String, String, String, String)>) -> (DataArray, DataArray) {
    let mut norm = DataArray::new();
    let mut replay = DataArray::new();
    for (id, name, args, sig) in raw {
        let mut n = DataObject::new();
        n.put_string("id", &id);
        n.put_string("name", &name);
        n.put_string("arguments", &args);
        norm.push_object(n);

        let mut rf = DataObject::new();
        rf.put_string("name", &name);
        rf.put_string("arguments", &args);
        let mut rc = DataObject::new();
        rc.put_string("id", &id);
        rc.put_string("type", "function");
        rc.put_object("function", rf);
        // Gemini 3 returns a thoughtSignature on the part carrying a
        // functionCall and REQUIRES it back when that turn is replayed
        // ("Function call is missing a thought_signature in functionCall
        // parts", INVALID_ARGUMENT). It rides the replay object so the
        // conversation the caller keeps carries it; only the gemini payload
        // builder reads it back out, and only a gemini conversation ever
        // has one.
        if !sig.is_empty() { rc.put_string("thought_signature", &sig); }
        replay.push_object(rc);
    }
    (norm, replay)
}

fn parse_openai(root: &DataObject, arm: &str) -> Result<DataObject, DataObject> {
    if let Ok(choices) = root.try_get_array("choices") {
        if choices.len() > 0 {
            let choice = choices.get_object(0);
            if let Ok(message) = choice.try_get_object("message") {
                let content = message.try_get_string("content").unwrap_or_default();
                if let Ok(calls) = message.try_get_array("tool_calls") {
                    if calls.len() > 0 {
                        let mut raw = Vec::new();
                        for (i, c) in calls.objects().iter().enumerate() {
                            let c = c.object();
                            let f = c.get_object("function");
                            let id = if c.has("id") { c.get_string("id") } else { format!("call_{}", i) };
                            raw.push((id, f.get_string("name"),
                                      args_to_string(&f.get_property("arguments")),
                                      String::new()));
                        }
                        let (norm, replay) = pack_calls(raw);
                        let mut out = DataObject::new();
                        out.put_string("kind", "tool_calls");
                        out.put_array("tool_calls", norm);
                        out.put_object("assistant_message", assistant_msg(&content, Some(replay)));
                        return Ok(out);
                    }
                }
                if message.has("content") { return Ok(text_result(&content)); }
            }
            // A length-capped or filtered choice can carry no message at all.
            if let Ok(fr) = choice.try_get_string("finish_reason") {
                return Err(err_out(&format!("{} returned no content (finish_reason: {})", arm, fr)));
            }
        }
    }
    if let Ok(e) = root.try_get_object("error") {
        return Err(err_out(&format!("{} error: {}", arm,
            e.try_get_string("message").unwrap_or_else(|_| e.to_string()))));
    }
    Err(err_out(&format!("{}: unexpected response shape: {}", arm,
        root.to_string().chars().take(1200).collect::<String>())))
}

fn parse_gemini(root: &DataObject, arm: &str) -> Result<DataObject, DataObject> {
    // Prompt-level block: no candidates at all.
    if let Ok(fb) = root.try_get_object("promptFeedback") {
        if let Ok(reason) = fb.try_get_string("blockReason") {
            return Err(err_out(&format!("{} blocked the PROMPT ({}). safetySettings are already BLOCK_NONE, so this is a non-overridable category.", arm, reason)));
        }
    }
    if let Ok(e) = root.try_get_object("error") {
        return Err(err_out(&format!("{} error: {}", arm,
            e.try_get_string("message").unwrap_or_else(|_| e.to_string()))));
    }
    if let Ok(candidates) = root.try_get_array("candidates") {
        if candidates.len() > 0 {
            let cand = candidates.get_object(0);
            let mut text = String::new();
            let mut text_sig = String::new();
            let mut raw = Vec::new();
            if let Ok(content) = cand.try_get_object("content") {
                if let Ok(parts) = content.try_get_array("parts") {
                    for (i, p) in parts.objects().iter().enumerate() {
                        let p = p.object();
                        // A thinking part is not the answer; it carries
                        // thought:true and must not be concatenated into it.
                        let is_thought = matches!(p.try_get_boolean("thought"), Ok(true));
                        // JSON is camelCase (thoughtSignature); the error text
                        // uses the proto spelling. Accept both.
                        let sig = p.try_get_string("thoughtSignature")
                            .or_else(|_| p.try_get_string("thought_signature"))
                            .unwrap_or_default();
                        if let Ok(t) = p.try_get_string("text") {
                            if !is_thought { text.push_str(&t); }
                            if !sig.is_empty() && text_sig.is_empty() { text_sig = sig.clone(); }
                        }
                        if let Ok(fc) = p.try_get_object("functionCall") {
                            let args = match fc.try_get_object("args") {
                                Ok(a) => a.to_string(), _ => "{}".to_string() };
                            // Gemini has no call ids; synthesize stable ones so
                            // the tool_call_id round trip works.
                            raw.push((format!("call_{}", i), fc.get_string("name"), args, sig));
                        }
                    }
                }
            }
            if !raw.is_empty() {
                let (norm, replay) = pack_calls(raw);
                let mut out = DataObject::new();
                out.put_string("kind", "tool_calls");
                out.put_array("tool_calls", norm);
                out.put_object("assistant_message", assistant_msg(&text, Some(replay)));
                return Ok(out);
            }
            if !text.is_empty() {
                // not `mut`: the signature is written through the handle
                // get_object returns, not through `out` itself.
                let out = text_result(&text);
                if !text_sig.is_empty() {
                    let mut am = out.get_object("assistant_message");
                    am.put_string("thought_signature", &text_sig);
                }
                return Ok(out);
            }
            // Empty candidate: say WHY instead of "unexpected JSON".
            if let Ok(reason) = cand.try_get_string("finishReason") {
                return Err(err_out(&format!("{} returned no content (finishReason: {}). SAFETY here means a category that BLOCK_NONE does not cover; MAX_TOKENS means raise LLM_MAX_TOKENS.", arm, reason)));
            }
        }
    }
    Err(err_out(&format!("{}: unexpected response shape: {}", arm,
        root.to_string().chars().take(1200).collect::<String>())))
}

fn parse_ollama(root: &DataObject, arm: &str) -> Result<DataObject, DataObject> {
    if let Ok(e) = root.try_get_string("error") {
        return Err(err_out(&format!("{} error: {}", arm, e)));
    }
    if let Ok(message) = root.try_get_object("message") {
        let content = message.try_get_string("content").unwrap_or_default();
        if let Ok(calls) = message.try_get_array("tool_calls") {
            if calls.len() > 0 {
                let mut raw = Vec::new();
                for (i, c) in calls.objects().iter().enumerate() {
                    let c = c.object();
                    let f = c.get_object("function");
                    // Ollama emits no ids either.
                    raw.push((format!("call_{}", i), f.get_string("name"),
                              args_to_string(&f.get_property("arguments")),
                              String::new()));
                }
                let (norm, replay) = pack_calls(raw);
                let mut out = DataObject::new();
                out.put_string("kind", "tool_calls");
                out.put_array("tool_calls", norm);
                out.put_object("assistant_message", assistant_msg(&content, Some(replay)));
                return Ok(out);
            }
        }
        if message.has("content") { return Ok(text_result(&content)); }
    }
    Err(err_out(&format!("{}: unexpected response shape: {}", arm,
        root.to_string().chars().take(1200).collect::<String>())))
}

fn build_anthropic_payload(messages: &DataArray, tools: &DataArray, model: &str,
                           max_tokens: i64, effort: &str, thinking: &str,
                           cache: bool) -> DataObject {
    // Tool results are their own USER turn here, and every tool_use in the
    // preceding assistant turn must be answered inside ONE of them. tool_loop
    // emits one role:"tool" message per call, so consecutive ones are merged
    // instead of sent as separate turns - splitting them is what teaches a
    // model to stop calling tools in parallel.
    fn flush(pending: &mut Vec<DataObject>, out: &mut DataArray) {
        if pending.is_empty() { return; }
        let mut arr = DataArray::new();
        for b in pending.drain(..) { arr.push_object(b); }
        let mut m = DataObject::new();
        m.put_string("role", "user");
        m.put_array("content", arr);
        out.push_object(m);
    }

    let mut payload = DataObject::new();
    payload.put_string("model", model);
    // REQUIRED - the Messages API has no server-side default. It caps THINKING
    // plus visible text together, so on a thinking-by-default model a tight
    // LLM_MAX_TOKENS truncates the answer rather than the reasoning.
    payload.put_int("max_tokens", max_tokens);

    let mut system = String::new();
    let mut out = DataArray::new();
    let mut pending: Vec<DataObject> = Vec::new();

    for i in 0..messages.len() {
        let m = messages.get_object(i);
        let role = m.try_get_string("role").unwrap_or_default();
        let content = m.try_get_string("content").unwrap_or_default();

        // Not a message at all on this wire: a top-level parameter.
        if role == "system" {
            if !system.is_empty() { system.push_str("\n\n"); }
            system.push_str(&content);
            continue;
        }
        if role == "tool" {
            let body = if content.is_empty() { "(no output)".to_string() } else { content.clone() };
            let mut tr = DataObject::new();
            tr.put_string("type", "tool_result");
            tr.put_string("tool_use_id", &m.try_get_string("tool_call_id").unwrap_or_default());
            tr.put_string("content", &body);
            pending.push(tr);
            continue;
        }
        flush(&mut pending, &mut out);

        let mut blocks = DataArray::new();
        if role == "assistant" {
            // Replayed VERBATIM and FIRST. With thinking on - the default on
            // Opus 5 - an assistant turn that called a tool is rejected on the
            // next request unless its thinking block comes back untouched.
            // They ride the assistant_message the caller already keeps,
            // exactly as Gemini's thought_signature does.
            if let Ok(tb) = m.try_get_array("thinking_blocks") {
                for b in tb.objects() { blocks.push_object(b.object()); }
            }
        }
        if !content.is_empty() {
            let mut t = DataObject::new();
            t.put_string("type", "text");
            t.put_string("text", &content);
            blocks.push_object(t);
        }
        if role == "assistant" {
            if let Ok(calls) = m.try_get_array("tool_calls") {
                for c in calls.objects() {
                    let c = c.object();
                    let f = c.get_object("function");
                    let mut tu = DataObject::new();
                    tu.put_string("type", "tool_use");
                    tu.put_string("id", &c.get_string("id"));
                    tu.put_string("name", &f.get_string("name"));
                    // input is an OBJECT here, not OpenAI's JSON string.
                    tu.put_object("input", args_to_object(&args_to_string(&f.get_property("arguments"))));
                    blocks.push_object(tu);
                }
            }
        }
        // An empty content array is rejected outright, so a contentless turn
        // is dropped rather than sent.
        if blocks.len() == 0 { continue; }
        let mut n = DataObject::new();
        n.put_string("role", if role == "assistant" { "assistant" } else { "user" });
        n.put_array("content", blocks);
        out.push_object(n);
    }
    flush(&mut pending, &mut out);
    payload.put_array("messages", out);
    if !system.is_empty() { payload.put_string("system", &system); }

    if tools.len() > 0 {
        let mut ts = DataArray::new();
        for t in tools.objects() {
            let t = t.object();
            let f = if t.has("function") { t.get_object("function") } else { t.clone() };
            if !f.has("name") { continue; }
            let mut d = DataObject::new();
            d.put_string("name", &f.get_string("name"));
            if f.has("description") { d.put_string("description", &f.get_string("description")); }
            // `input_schema`, not `parameters`, and it must be an object
            // schema. Copied rather than edited in place: the caller's tools
            // array is shared, and a builder has no business mutating it.
            let mut schema = DataObject::new();
            if let Ok(p) = f.try_get_object("parameters") {
                for (k, v) in p.objects() { schema.set_property(&k, v.clone()); }
            }
            if !schema.has("type") { schema.put_string("type", "object"); }
            d.put_object("input_schema", schema);
            ts.push_object(d);
        }
        if ts.len() > 0 { payload.put_array("tools", ts); }
    }

    // NO temperature: Opus 5 and the 4.7/4.8 family reject temperature, top_p
    // and top_k outright (400). Depth is output_config.effort instead, which
    // is why LLM_TEMPERATURE is not threaded into this builder at all.
    if !effort.is_empty() {
        let mut oc = DataObject::new();
        oc.put_string("effort", effort);
        payload.put_object("output_config", oc);
    }
    // Left unset by default: omitting it runs adaptive thinking on Opus 5 but
    // means NO thinking on 4.8/4.7, and `disabled` is a 400 above effort
    // `high`. Set ANTHROPIC_THINKING=adaptive to force it on an older model.
    if !thinking.is_empty() {
        let mut th = DataObject::new();
        th.put_string("type", thinking);
        payload.put_object("thinking", th);
    }
    // Top-level cache_control auto-places the breakpoint on the last cacheable
    // block, which for a growing conversation is the newest turn - so every
    // request after the first reads the whole prior prefix at ~0.1x input
    // price. The agent loop resends its entire history every call; this is the
    // difference between that being cheap and being the bill.
    if cache {
        let mut cc = DataObject::new();
        cc.put_string("type", "ephemeral");
        payload.put_object("cache_control", cc);
    }
    payload
}

fn parse_anthropic(root: &DataObject, arm: &str) -> Result<DataObject, DataObject> {
    if let Ok(e) = root.try_get_object("error") {
        return Err(err_out(&format!("{} error: {}", arm,
            e.try_get_string("message").unwrap_or_else(|_| e.to_string()))));
    }
    let stop = root.try_get_string("stop_reason").unwrap_or_default();
    // A safety decline is a 200 with EMPTY content, not an HTTP error. Read it
    // before walking content or it surfaces as "unexpected response shape" -
    // the exact misdiagnosis the native dialects exist to prevent.
    if stop == "refusal" {
        let detail = match root.try_get_object("stop_details") {
            Ok(d) => format!(" ({}{})",
                d.try_get_string("category").unwrap_or_else(|_| "unspecified".to_string()),
                match d.try_get_string("explanation") {
                    Ok(x) if !x.is_empty() => format!(": {}", x),
                    _ => String::new(),
                }),
            _ => String::new(),
        };
        return Err(err_out(&format!("{} declined the request{}", arm, detail)));
    }
    if let Ok(content) = root.try_get_array("content") {
        let mut text = String::new();
        let mut raw = Vec::new();
        let mut tb = DataArray::new();
        for item in content.objects() {
            let b = item.object();
            match b.try_get_string("type").unwrap_or_default().as_str() {
                "text" => text.push_str(&b.try_get_string("text").unwrap_or_default()),
                "tool_use" => {
                    let args = match b.try_get_object("input") {
                        Ok(a) => a.to_string(), _ => "{}".to_string() };
                    raw.push((b.try_get_string("id").unwrap_or_default(),
                              b.try_get_string("name").unwrap_or_default(),
                              args, String::new()));
                },
                // Reasoning, not the answer: never concatenated into it, but
                // kept whole so the next turn can replay it.
                "thinking" | "redacted_thinking" => tb.push_object(b),
                _ => {}
            }
        }
        if !raw.is_empty() {
            let (norm, replay) = pack_calls(raw);
            let mut out = DataObject::new();
            out.put_string("kind", "tool_calls");
            out.put_array("tool_calls", norm);
            out.put_object("assistant_message", assistant_msg(&text, Some(replay)));
            if tb.len() > 0 {
                let mut am = out.get_object("assistant_message");
                am.put_array("thinking_blocks", tb);
            }
            return Ok(out);
        }
        if !text.is_empty() {
            let out = text_result(&text);
            if tb.len() > 0 {
                let mut am = out.get_object("assistant_message");
                am.put_array("thinking_blocks", tb);
            }
            return Ok(out);
        }
        if stop == "max_tokens" {
            return Err(err_out(&format!("{} returned no content (stop_reason: max_tokens). max_tokens caps THINKING plus text together, so a thinking model can spend the whole budget before answering - raise LLM_MAX_TOKENS.", arm)));
        }
        if !stop.is_empty() {
            return Err(err_out(&format!("{} returned no content (stop_reason: {})", arm, stop)));
        }
    }
    Err(err_out(&format!("{}: unexpected response shape: {}", arm,
        root.to_string().chars().take(1200).collect::<String>())))
}

// ── resolve ──────────────────────────────────────────────────────────────
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
let arm = match meta.try_get_string("LLM") {
    Ok(v) if !v.trim().is_empty() => v.trim().to_uppercase(),
    _ => "VLLM".to_string(),
};

// (dialect, url, model, headers)
let resolved: Option<(String, String, String, Vec<(String, String)>)> = match arm.as_str() {
    "VLLM" => {
        let url = match need(&meta, "VLLM_URL", &arm) { Ok(v) => v, Err(e) => return err_out(&e) };
        let model = match need(&meta, "VLLM_MODEL", &arm) { Ok(v) => v, Err(e) => return err_out(&e) };
        let mut h = vec![("Content-Type".to_string(), "application/json".to_string())];
        // vLLM served with --api-key needs this; a bare local server does not,
        // which is why it is optional rather than required.
        let k = opt_key(&meta, "VLLM");
        if !k.is_empty() { h.push(("Authorization".to_string(), format!("Bearer {}", k))); }
        Some(("openai".to_string(), url, model, h))
    },
    "OPENAI" => {
        let custom_url = opt(&meta, "OPENAI_URL", "");
        let url = if custom_url.is_empty() { "https://api.openai.com/v1/chat/completions".to_string() } else { custom_url.clone() };
        let model = match need(&meta, "OPENAI_MODEL", &arm) { Ok(v) => v, Err(e) => return err_out(&e) };
        let mut h = vec![("Content-Type".to_string(), "application/json".to_string())];
        // api.openai.com always needs a key; a self-hosted compatible server
        // reached through OPENAI_URL may be keyless.
        let k = if custom_url.is_empty() {
            match need_key(&meta, "OPENAI") { Ok(v) => v, Err(e) => return err_out(&e) }
        } else { opt_key(&meta, "OPENAI") };
        if !k.is_empty() { h.push(("Authorization".to_string(), format!("Bearer {}", k))); }
        Some(("openai".to_string(), url, model, h))
    },
    "ANTHROPIC" => {
        let model = match need(&meta, "ANTHROPIC_MODEL", &arm) { Ok(v) => v, Err(e) => return err_out(&e) };
        let key = match need_key(&meta, "ANTHROPIC") { Ok(v) => v, Err(e) => return err_out(&e) };
        let url = opt(&meta, "ANTHROPIC_URL", "https://api.anthropic.com/v1/messages");
        // The key rides x-api-key, NOT Authorization: Bearer - and
        // anthropic-version is required on every request. Neither is
        // expressible by pointing the OPENAI arm at this URL, and the body
        // differs anyway (see the dialect note at the top).
        let h = vec![("Content-Type".to_string(), "application/json".to_string()),
                     ("x-api-key".to_string(), key),
                     ("anthropic-version".to_string(),
                      opt(&meta, "ANTHROPIC_VERSION", "2023-06-01"))];
        Some(("anthropic".to_string(), url, model, h))
    },
    "GEMINI" => {
        let model = match need(&meta, "GEMINI_MODEL", &arm) { Ok(v) => v, Err(e) => return err_out(&e) };
        let key = match need_key(&meta, "GEMINI") { Ok(v) => v, Err(e) => return err_out(&e) };
        // The key rides a HEADER, not `?key=` — a URL-embedded secret ends up
        // in every log line that prints the endpoint.
        let url = opt(&meta, "GEMINI_URL",
            &format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent", model));
        let h = vec![("Content-Type".to_string(), "application/json".to_string()),
                     ("x-goog-api-key".to_string(), key)];
        Some(("gemini".to_string(), url, model, h))
    },
    "OLLAMA" => {
        // /api/chat, NOT /api/generate: generate is single-prompt with no
        // conversation and no tools, so it cannot carry the agent loop.
        let url = opt(&meta, "OLLAMA_URL", "http://127.0.0.1:11434/api/chat");
        let model = match need(&meta, "OLLAMA_MODEL", &arm) { Ok(v) => v, Err(e) => return err_out(&e) };
        let mut h = vec![("Content-Type".to_string(), "application/json".to_string())];
        let k = opt_key(&meta, "OLLAMA");       // hosted Ollama needs one
        if !k.is_empty() { h.push(("Authorization".to_string(), format!("Bearer {}", k))); }
        Some(("ollama".to_string(), url, model, h))
    },
    _ => None,
};

if resolved.is_none() {
    // LLM_CTL dispatch (the CUSTOM arm). TWO command shapes are honored,
    // selected by the command's DECLARED params:
    //   (messages, tools) — tool-capable: the command gets this call's
    //     messages/tools verbatim and answers in chat_llm's own result shape,
    //     passed through, so a custom provider can drive the full agent loop.
    //   (prompt, system_prompt) — legacy text: conversation flattened, no
    //     tools forwarded, text back.
    let whole = match need(&meta, "LLM_CTL", &arm) { Ok(v) => v, Err(e) => return err_out(&e) };
    let parts: Vec<&str> = whole.split(':').collect();
    if parts.len() < 3 { return err_out("LLM_CTL is not in the format 'lib:ctl:cmd'"); }
    let custom_cmd = Command::lookup(parts[0], parts[1], parts[2]);
    if custom_cmd.params.iter().any(|(n, _)| n == "messages") {
        let mut params = DataObject::new();
        params.put_array("messages", messages.clone());
        params.put_array("tools", tools.clone());
        match custom_cmd.execute(params) {
            Ok(res) => {
                let res = match res.try_get_object("a") { Ok(inner) => inner, _ => res };
                if res.try_get_string("kind").is_ok() { return res; }
                let msg = if let Ok(m) = res.try_get_string("msg") { m }
                    else if let Ok(m) = res.try_get_string("content") { m }
                    else { res.to_string() };
                return text_result(&msg);
            },
            Err(e) => return err_out(&format!("LLM_CTL {} failed: {:?}", whole, e)),
        }
    }
    let mut system = String::new();
    let mut turns: Vec<(String, String)> = Vec::new();
    for i in 0..messages.len() {
        let m = messages.get_object(i);
        let role = m.try_get_string("role").unwrap_or_default();
        let content = m.try_get_string("content").unwrap_or_default();
        if content.is_empty() { continue; }
        if role == "system" && system.is_empty() { system = content; }
        else { turns.push((role, content)); }
    }
    // A single user turn is a plain ASK, so it goes through verbatim — that
    // is exactly what ask_llm used to hand a legacy LLM_CTL command, and
    // prefixing it with "USER: " would have silently changed the contract
    // for every custom text provider. Only a real multi-turn conversation
    // gets role labels, because otherwise it is unreadable.
    let convo = if turns.len() == 1 && turns[0].0 == "user" {
        turns[0].1.clone()
    } else {
        turns.iter().map(|(r, c)| format!("{}: {}\n\n", r.to_uppercase(), c))
             .collect::<String>()
    };
    let mut params = DataObject::new();
    params.put_string("prompt", convo.trim());
    params.put_string("system_prompt", &system);
    match custom_cmd.execute(params) {
        Ok(res) => {
            let msg = if let Ok(m) = res.try_get_string("msg") { m }
                else if let Ok(m) = res.try_get_string("a") { m }
                else if let Ok(o) = res.try_get_object("a") {
                    o.try_get_string("msg").unwrap_or_else(|_| o.to_string()) }
                else { res.to_string() };
            return text_result(&msg);
        },
        Err(e) => return err_out(&format!("LLM_CTL {} failed: {:?}", whole, e)),
    }
}
let (dialect, url, model, mut headers) = resolved.unwrap();
for kv in extra_headers(&meta, &arm) {
    headers.retain(|(k, _)| !k.eq_ignore_ascii_case(&kv.0));
    headers.push(kv);
}

let temperature = opt(&meta, "LLM_TEMPERATURE", "0.2").parse::<f64>().unwrap_or(0.2);
let max_tokens = opt(&meta, "LLM_MAX_TOKENS", "8192").parse::<i64>().unwrap_or(8192);

let payload = match dialect.as_str() {
    "anthropic" => build_anthropic_payload(&messages, &tools, &model, max_tokens,
                                           &opt(&meta, "ANTHROPIC_EFFORT", ""),
                                           &opt(&meta, "ANTHROPIC_THINKING", ""),
                                           opt(&meta, "ANTHROPIC_CACHE", "on") != "off"),
    "gemini" => build_gemini_payload(&messages, &tools, temperature, max_tokens),
    "ollama" => build_ollama_payload(&messages, &tools, &model, temperature, max_tokens,
                                     &opt(&meta, "OLLAMA_KEEP_ALIVE", "0")),
    _ => {
        let mut p = DataObject::new();
        p.put_string("model", &model);
        p.put_array("messages", messages.clone());
        if tools.len() > 0 {
            p.put_array("tools", tools.clone());
            p.put_string("tool_choice", "auto");
        }
        // vLLM-only: no thinking for tool work. Other OpenAI-compatible
        // servers reject or ignore unknown fields, so this is gated on the arm.
        if arm == "VLLM" {
            let mut chat_kwargs = DataObject::new();
            chat_kwargs.put_boolean("enable_thinking", false);
            p.put_object("chat_template_kwargs", chat_kwargs);
        }
        p.put_float("temperature", temperature);
        p.put_int("max_tokens", max_tokens);
        p
    }
};

let http = ureq::AgentBuilder::new()
    .timeout_read(std::time::Duration::from_secs(600))
    .timeout_write(std::time::Duration::from_secs(600))
    .build();

println!("chat_llm[{}/{}]: {} messages, {} tools", arm, dialect, messages.len(), tools.len());

let mut out = err_out(&format!("{}: no response", arm));
let attempts = 4u32;
for attempt in 0..attempts {
    let mut req = http.post(&url);
    for (k, v) in &headers { req = req.set(k, v); }

    // RETRY ONLY WHAT IS RETRYABLE: transport, 408, 429, 5xx. A 400/401/403/404
    // is a configuration answer, and retrying it only delays the report — the
    // vLLM "System message must be at the beginning." 400 used to burn five
    // attempts and 31s of sleeps before surfacing.
    let (retryable, retry_after) = match req.send_json(payload.to_json()) {
        Ok(resp) => {
            match resp.into_string() {
                Ok(body) => {
                    match obj_from_str(&body) {
                        Some(root) => match match dialect.as_str() {
                            "anthropic" => parse_anthropic(&root, &arm),
                            "gemini" => parse_gemini(&root, &arm),
                            "ollama" => parse_ollama(&root, &arm),
                            _ => parse_openai(&root, &arm),
                        } {
                            Ok(good) => return good,
                            Err(e) => { out = e; (false, 0u64) }
                        },
                        None => {
                            out = err_out(&format!("{}: response was not a JSON object: {}", arm,
                                body.chars().take(600).collect::<String>()));
                            (false, 0u64)
                        }
                    }
                },
                Err(e) => {
                    out = err_out(&format!("{}: reading the response body failed: {}", arm, e));
                    (true, 0u64)
                }
            }
        },
        Err(ureq::Error::Status(code, resp)) => {
            let ra = resp.header("Retry-After")
                .and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0).min(60);
            let body = resp.into_string().unwrap_or_default();
            out = err_out(&format!("{} API status {}: {}", arm, code,
                body.chars().take(1200).collect::<String>()));
            (code == 408 || code == 429 || code >= 500, ra)
        },
        Err(ureq::Error::Transport(e)) => {
            out = err_out(&format!("{}: network error to {}: {}", arm, url, e));
            (true, 0u64)
        }
    };

    if !retryable || attempt + 1 == attempts { break; }
    let backoff = if retry_after > 0 { retry_after } else { 2u64.pow(attempt) };
    println!("chat_llm[{}] attempt {} failed, retrying in {}s: {}",
             arm, attempt + 1, backoff, out.get_string("content"));
    std::thread::sleep(std::time::Duration::from_secs(backoff));
}

out

}
