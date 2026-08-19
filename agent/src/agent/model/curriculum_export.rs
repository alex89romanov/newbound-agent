use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
use crate::agent::model::dataset_feed::dataset_feed;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["path"] {
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
        let arg_0: String = o.get_string("path");
        curriculum_export(arg_0)
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

pub fn curriculum_export(path: String) -> DataObject {
// curriculum_export (understandingloop.md commitment 5): turn the
// accumulated feedstock into one JSONL batch at `path` - typically the
// service's ingest directory (runtime/model/ingest/...), where the
// trainer drains it. Three sample kinds, one line each:
//   salience_pair  - every escalation/audit row from the runtime
//                    salience log: (input, local, frontier, disagree)
//   curation_trace - every adjudication trace on every domain's traces
//                    facet: (claim, relation, action, before/after,
//                    reasoning)
//   claim          - every live (non-superseded) claim in the
//                    federation, with home and confidence
// Raw logs never ride: these are the adjudicated, structured residues
// the flywheel doctrine names. Export is deliberate and explicit, like
// seed_export - nothing writes training data on its own.
fn prop(key: &str, dflt: &str) -> String {
    (|| -> Option<String> {
        let s = DataStore::globals().try_get_object("system").ok()?;
        let a = s.try_get_object("apps").ok()?;
        let g = a.try_get_object("agent").ok()?;
        let r = g.try_get_object("runtime").ok()?;
        match r.try_get_string(key) {
            Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
            _ => None,
        }
    })().unwrap_or_else(|| dflt.to_string())
}
fn service_url() -> String {
    format!("http://127.0.0.1:{}", prop("MODEL_SERVICE_PORT", "8077"))
}
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}

let _ = service_url; // shared helper block; export writes a file, no HTTP
let store = DataStore::new();
let mut lines: Vec<String> = Vec::new();
let mut n_pairs = 0i64;
let mut n_traces = 0i64;
let mut n_claims = 0i64;

fn esc_json(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// 1. salience pairs
if store.exists("runtime", "salience_log") {
    let d = store.get_data("runtime", "salience_log").get_object("data");
    if d.has("rows") {
        let rows = d.get_array("rows");
        for i in 0..rows.len() {
            if let Ok(r) = rows.try_get_object(i) {
                lines.push(format!(
                    "{{\"kind\": \"salience_pair\", \"row\": {}}}", r.to_string()));
                n_pairs += 1;
            }
        }
    }
}

// 2 + 3. the federation walk: traces and live claims
let mut libs: Vec<String> = Vec::new();
if let Ok(rd) = std::fs::read_dir(&store.root) {
    for e in rd.flatten() {
        if e.path().is_dir() {
            if let Ok(n) = e.file_name().into_string() { libs.push(n); }
        }
    }
}
libs.sort();
for lib in libs {
    if !store.exists(&lib, "controls") { continue; }
    let list = store.get_data(&lib, "controls").get_object("data").get_array("list");
    for i in 0..list.len() {
        let item = list.get_object(i);
        if !item.has("name") || !item.has("id") { continue; }
        let name = item.get_string("name");
        let id = item.get_string("id");
        if !store.exists(&lib, &id) { continue; }
        let dd = store.get_data(&lib, &id).get_object("data");
        let home = format!("{}.{}", lib, name);
        if dd.has("traces") {
            if let Ok(w) = DataObject::try_from_string(&format!("{{\"a\":{}}}", dd.get_string("traces"))) {
                if let Ok(a) = w.try_get_array("a") {
                    for j in 0..a.len() {
                        if let Ok(t) = a.try_get_object(j) {
                            lines.push(format!(
                                "{{\"kind\": \"curation_trace\", \"home\": \"{}\", \"trace\": {}}}",
                                esc_json(&home), t.to_string()));
                            n_traces += 1;
                        }
                    }
                }
            }
        }
        if dd.has("memory") {
            if let Ok(w) = DataObject::try_from_string(&format!("{{\"a\":{}}}", dd.get_string("memory"))) {
                if let Ok(a) = w.try_get_array("a") {
                    for j in 0..a.len() {
                        if let Ok(e) = a.try_get_object(j) {
                            if !e.has("claim") || e.has("superseded") { continue; }
                            lines.push(format!(
                                "{{\"kind\": \"claim\", \"home\": \"{}\", \"entry\": {}}}",
                                esc_json(&home), e.to_string()));
                            n_claims += 1;
                        }
                    }
                }
            }
        }
    }
}

if let Some(parent) = std::path::Path::new(&path).parent() {
    let _ = std::fs::create_dir_all(parent);
}
let content = lines.join("\n") + "\n";
if let Err(e) = std::fs::write(&path, &content) {
    return err(format!("could not write {}: {}", path, e));
}

// ── the feed contract (spectrum S2, factored H1c): the sweep hands its
// lines to dataset_feed, which owns dedup, the append, registration and
// the registry re-render - one feeder for every channel, so the banks
// cannot drift apart. The legacy batch file above is untouched (R1);
// the trainer keeps draining ingest until the pools take over.
let mut appended_pairs = 0i64;
let mut appended_memory = 0i64;
{
    let mut pair_lines = String::new();
    let mut mem_lines = String::new();
    for ln in &lines {
        if ln.starts_with("{\"kind\": \"salience_pair\"") {
            pair_lines.push_str(ln); pair_lines.push('\n');
        } else {
            mem_lines.push_str(ln); mem_lines.push('\n');
        }
    }
    let r = dataset_feed("salience-pairs".to_string(), "cpt".to_string(),
        pair_lines, "swept:salience_log".to_string(),
        "curriculum_export".to_string(), 10);
    if r.try_get_string("status").ok().as_deref() == Some("ok") {
        appended_pairs = r.try_get_int("appended").unwrap_or(0);
    }
    let r = dataset_feed("memory".to_string(), "cpt".to_string(),
        mem_lines, "swept:federation".to_string(),
        "curriculum_export".to_string(), 10);
    if r.try_get_string("status").ok().as_deref() == Some("ok") {
        appended_memory = r.try_get_int("appended").unwrap_or(0);
    }
}

// ── the chat kind (harvest H1c): capture rows are ID indexes; this is
// where they become trainable text. Each row's msg_ids + reply_id
// resolve through the message store and render as one backend-neutral
// conversation row ({"messages": [...]}), fed into the REGISTERED
// chat-bank stream (kind sft) - sft_run's feedstock, never the CPT
// trainer's. holdout_every=5 matches the persona split and makes ten
// banked conversations exactly the 8/2 corpus sft_run demands. The
// arm rides along as a provenance tag on each row - a tag, never a
// branch: every arm's traffic renders identically.
let mut chat_appended = 0i64;
{
    fn msg_text(store: &DataStore, oid: &str) -> Option<(String, String)> {
        if !store.exists("runtime", oid) { return None; }
        let d = store.get_data("runtime", oid).get_object("data");
        let role = if d.has("role") { d.get_string("role") } else { return None; };
        let cid = if d.has("content_id") { d.get_string("content_id") } else { return None; };
        if !store.exists("runtime", &cid) { return None; }
        let cd = store.get_data("runtime", &cid).get_object("data");
        if !cd.has("text") { return None; }
        Some((role, cd.get_string("text")))
    }
    let capdir = store.root.canonicalize().ok()
        .and_then(|r| r.parent().map(|p| p.to_path_buf()))
        .map(|r| r.join("runtime").join("agent").join("model").join("capture"));
    if let Some(capdir) = capdir {
        let mut chat_lines = String::new();
        if let Ok(entries) = std::fs::read_dir(&capdir) {
            let mut files: Vec<std::path::PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|x| x == "jsonl").unwrap_or(false))
                .collect();
            files.sort();
            for f in files {
                let text = std::fs::read_to_string(&f).unwrap_or_default();
                for ln in text.lines() {
                    let ln = ln.trim();
                    if ln.is_empty() { continue; }
                    let row = match DataObject::try_from_string(ln) { Ok(r) => r, Err(_) => continue };
                    // text replies only: a tool_calls transcript is not a
                    // conversation the SFT loss can learn from yet
                    if row.try_get_string("kind").ok().as_deref() != Some("text") { continue; }
                    let reply_id = row.try_get_string("reply_id").unwrap_or_default();
                    if reply_id.is_empty() { continue; }
                    let ids = match row.try_get_array("msg_ids") { Ok(a) => a, Err(_) => continue };
                    let mut msgs = DataArray::new();
                    let mut whole = true;
                    for i in 0..ids.len() {
                        let oid = ids.get_string(i);
                        match msg_text(&store, &oid) {
                            Some((role, text2)) => {
                                let mut m = DataObject::new();
                                m.put_string("role", &role);
                                m.put_string("content", &text2);
                                msgs.push_object(m);
                            },
                            None => { whole = false; break; }
                        }
                    }
                    if !whole || msgs.len() == 0 { continue; }
                    match msg_text(&store, &reply_id) {
                        Some((_role, text2)) => {
                            let mut m = DataObject::new();
                            m.put_string("role", "assistant");
                            m.put_string("content", &text2);
                            msgs.push_object(m);
                        },
                        None => { continue; }
                    }
                    let mut conv = DataObject::new();
                    conv.put_array("messages", msgs);
                    let mut prov = DataObject::new();
                    prov.put_string("arm", &row.try_get_string("arm").unwrap_or_default());
                    prov.put_string("model", &row.try_get_string("model").unwrap_or_default());
                    prov.put_int("t", row.try_get_int("t").unwrap_or(0));
                    prov.put_string("reply_id", &reply_id);
                    conv.put_object("provenance", prov);
                    chat_lines.push_str(&conv.to_string().replace('\n', " "));
                    chat_lines.push('\n');
                }
            }
        }
        if !chat_lines.is_empty() {
            let r = dataset_feed("chat-bank".to_string(), "sft".to_string(),
                chat_lines, "swept:capture".to_string(),
                "curriculum_export".to_string(), 5);
            if r.try_get_string("status").ok().as_deref() == Some("ok") {
                chat_appended = r.try_get_int("appended").unwrap_or(0);
            }
        }
    }
}

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("path", &path);
o.put_int("salience_pairs", n_pairs);
o.put_int("curation_traces", n_traces);
o.put_int("claims", n_claims);
o.put_int("total", n_pairs + n_traces + n_claims);
o.put_int("stream_pairs_appended", appended_pairs);
o.put_int("stream_memory_appended", appended_memory);
o.put_int("chat_appended", chat_appended);
o

}
