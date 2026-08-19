use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use flowlang::datastore::DataStore;
use crate::agent::llm::ask_llm::ask_llm;
use crate::agent::archivist::recall::recall;
use crate::agent::archivist::adjudicate::adjudicate;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["subject"] {
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
        let arg_0: String = o.get_string("subject");
        connect(arg_0)
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

pub fn connect(subject: String) -> DataObject {
// agent-archivist-connect - the mind's synthesis act (harvest H5).
// Picks a claim neighborhood by recall and asks for ONE inference
// that FOLLOWS from the claims but is stated in none of them. The
// product is a NOTION, not knowledge: it lands in kb.notions at low
// confidence, tagged inferred, through adjudicate's hysteresis - and
// notions never self-promote; the owner's memory-tab audit
// (bless / edit / forget) is the only gate from notion to knowledge.
// An empty subject seeds itself from the newest room/session activity
// so the drive tick can call it without choosing.
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
let mut subject_t = subject.trim().to_string();
if subject_t.is_empty() {
    // self-seed: the most recent perception context or a rotating pick
    // from the memory index would be richer; the newest claim's tags
    // are the cheap honest seed
    let g = DataStore::globals();
    if g.has("AGENT_EXECUTIVE") {
        let ex = g.get_object("AGENT_EXECUTIVE");
        if ex.has("last_kind") && !ex.get_string("last_kind").is_empty() {
            subject_t = ex.get_string("last_kind");
        }
    }
    if subject_t.is_empty() { subject_t = "the agent's own recent work".to_string(); }
}
let r = recall(subject_t.clone(), String::new(), 8);
if !r.has("claims") { return err("recall failed".to_string()); }
let claims = r.get_array("claims");
if claims.len() < 3 {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_string("skipped", &format!("only {} claims recalled for '{}' - a neighborhood needs 3", claims.len(), subject_t));
    o.put_int("deposited", 0);
    return o;
}
let mut listing = String::new();
for i in 0..claims.len() {
    if let Ok(c) = claims.try_get_object(i) {
        listing.push_str(&format!("{}. [{}] {}\n", i + 1,
            if c.has("home") { c.get_string("home") } else { String::new() },
            c.get_string("claim").chars().take(300).collect::<String>()));
    }
}
let prompt = format!(
    "You are the agent reflecting on its own knowledge. Below are related claims it holds about '{}'.\nCLAIMS:\n{}\nState ONE inference that FOLLOWS from these claims together but is stated in none of them - a connection, a consequence, or a tension worth noticing. It must be non-obvious and checkable. If nothing genuinely follows, say so.\nReply with ONLY one JSON object, no fences:\n{{\"notion\": \"<one standalone sentence, or empty if nothing follows>\", \"from\": [<claim numbers used>], \"why\": \"<one sentence>\"}}",
    subject_t, listing);
let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    ask_llm(prompt, Data::DNull)
})).unwrap_or_else(|_| "ERROR: ask_llm panicked".to_string());
if reply.starts_with("ERROR") {
    return err(format!("the frontier arm failed: {}", reply.chars().take(200).collect::<String>()));
}
let fd = reply.find('{').and_then(|s0| reply.rfind('}').map(|e0| (s0, e0)))
    .filter(|(s0, e0)| e0 > s0)
    .and_then(|(s0, e0)| DataObject::try_from_string(&reply[s0..=e0]).ok());
let fd = match fd {
    Some(f) => f,
    None => {
        let mut o = DataObject::new();
        o.put_string("status", "ok");
        o.put_boolean("unparseable", true);
        o.put_int("deposited", 0);
        return o;
    }
};
let notion = if fd.has("notion") { fd.get_string("notion") } else { String::new() };
if notion.trim().is_empty() {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_string("skipped", "nothing genuinely follows - an honest empty is a good answer");
    o.put_int("deposited", 0);
    return o;
}
let why = if fd.has("why") { fd.get_string("why") } else { String::new() };
let mut used = String::new();
if fd.has("from") {
    if let Ok(fr) = fd.try_get_array("from") {
        let mut parts: Vec<String> = Vec::new();
        for i in 0..fr.len() {
            if let Ok(n) = fr.try_get_int(i) {
                if n >= 1 && (n as usize) <= claims.len() as usize {
                    if let Ok(c) = claims.try_get_object((n - 1) as usize) {
                        parts.push(c.get_string("claim").chars().take(80).collect::<String>());
                    }
                }
            }
        }
        used = parts.join(" | ");
    }
}
let mut entry = DataObject::new();
entry.put_string("claim", notion.trim());
entry.put_string("detail", &format!("{} [follows from: {}]", why, used));
entry.put_string("tags", "inferred,connect");
entry.put_string("confidence", "low");
let adj = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    adjudicate("kb".to_string(), "notions".to_string(), entry.deep_copy(), "connect".to_string())
}));
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("subject", &subject_t);
o.put_string("notion", notion.trim());
o.put_int("deposited", if adj.is_ok() { 1 } else { 0 });
o

}
