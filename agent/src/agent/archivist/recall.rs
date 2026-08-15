use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["query", "domains", "limit"] {
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
        let arg_0: String = o.get_string("query");
        let arg_1: String = o.get_string("domains");
        let arg_2: i64 = o.get_int("limit");
        recall(arg_0, arg_1, arg_2)
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

pub fn recall(query: String, domains: String, limit: i64) -> DataObject {
// recall (understandingloop.md Phase 1): claims by topic across the whole
// federation - the brain and every library's manuals - with staleness
// computed from the source hashes remember stamps at write time. This is
// the orient contract: callers depend on the RESULT SHAPE ({claim, detail,
// tags, confidence, home, time, age_days, stale, stale_checked, promoted});
// the internals are free to change behind it. Read-only: recall never
// writes, so it is safe from any thread at any tick rate.
fn content_hash(s: &str) -> String {
    // FNV-1a over the \r-normalized source - MUST stay in sync with
    // remember/read_control_facet/patch_control_facet.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}
fn ctl_id(store: &DataStore, lib: &str, name: &str) -> String {
    if !store.exists(lib, "controls") { return String::new(); }
    let rec = store.get_data(lib, "controls").get_object("data");
    if !rec.has("list") { return String::new(); }
    for c in rec.get_array("list").objects() {
        let c = c.object();
        if c.has("name") && c.get_string("name") == name { return c.get_string("id"); }
    }
    String::new()
}

let store = DataStore::new();
let q = query.to_lowercase();
// Function words carry no topic; without this filter a conversational
// query ("does the executive...") matches most of the store and only
// ranking saves the caller.
const STOP: &[&str] = &["the", "and", "for", "with", "that", "this", "from",
    "are", "was", "were", "has", "have", "had", "does", "did", "not", "its",
    "you", "your", "all", "any", "can", "will", "into", "out", "about",
    "what", "when", "how", "why", "who", "against"];
let tokens: Vec<String> = q.split(|c: char| !c.is_alphanumeric())
    .filter(|t| t.len() >= 3 && !STOP.contains(t))
    .map(|t| t.to_string())
    .collect();
if tokens.is_empty() {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", "query yielded no usable tokens (3+ characters)");
    return o;
}
let domain_filter: Vec<String> = domains.split(',')
    .map(|d| d.trim().to_string())
    .filter(|d| !d.is_empty())
    .collect();
let cap = (if limit < 1 { 1 } else if limit > 50 { 50 } else { limit }) as usize;

// The federation walk (consolidate's, read-only): every control of every
// library that carries a memory facet is a recall source - the brain and
// the shipped manuals answer through one door.
let mut libs: Vec<String> = Vec::new();
if let Ok(rd) = std::fs::read_dir(&store.root) {
    for e in rd.flatten() {
        if e.path().is_dir() {
            if let Ok(n) = e.file_name().into_string() { libs.push(n); }
        }
    }
}
libs.sort();
let now = time();
let mut considered: i64 = 0;
let mut hits: Vec<(i64, i64, DataObject)> = Vec::new();
for lib in libs {
    if !store.exists(&lib, "controls") { continue; }
    let list = store.get_data(&lib, "controls").get_object("data").get_array("list");
    for i in 0..list.len() {
        let item = list.get_object(i);
        if !item.has("name") || !item.has("id") { continue; }
        let name = item.get_string("name");
        let id = item.get_string("id");
        let home = format!("{}.{}", lib, name);
        if !domain_filter.is_empty() && !domain_filter.contains(&home) { continue; }
        if !store.exists(&lib, &id) { continue; }
        let dd = store.get_data(&lib, &id).get_object("data");
        if !dd.has("memory") { continue; }
        let w = match DataObject::try_from_string(&format!("{{\"a\":{}}}", dd.get_string("memory"))) {
            Ok(w) => w,
            Err(_) => continue,
        };
        let a = match w.try_get_array("a") { Ok(a) => a, Err(_) => continue };
        for j in 0..a.len() {
            let e = match a.try_get_object(j) { Ok(e) => e, Err(_) => continue };
            if !e.has("claim") { continue; }
            // Superseded claims are history, not belief (Phase 3): they
            // stay in the facet for the audit trail but never answer.
            if e.has("superseded") { continue; }
            considered += 1;
            let claim = e.get_string("claim");
            let detail = if e.has("detail") { e.get_string("detail") } else { String::new() };
            let tags = if e.has("tags") { e.get_string("tags") } else { String::new() };
            let cl = claim.to_lowercase();
            let dl = detail.to_lowercase();
            let tl = tags.to_lowercase();
            let mut score: i64 = 0;
            for t in &tokens {
                if cl.contains(t.as_str()) { score += 3; }
                if tl.contains(t.as_str()) { score += 2; }
                if dl.contains(t.as_str()) { score += 1; }
            }
            if score == 0 { continue; }
            let mut out = DataObject::new();
            out.put_string("claim", &claim);
            if !detail.is_empty() { out.put_string("detail", &detail); }
            if !tags.is_empty() { out.put_string("tags", &tags); }
            if e.has("confidence") { out.put_string("confidence", &e.get_string("confidence")); }
            out.put_string("home", &home);
            let t0 = if e.has("time") { e.get_int("time") } else { 0 };
            out.put_int("time", t0);
            out.put_int("age_days", if t0 > 0 { (now - t0) / 86_400_000 } else { -1 });
            out.put_boolean("promoted", e.has("promoted"));
            // Staleness is drift, not age: a facet-pointer source whose
            // referent no longer hashes to the stamped value marks the
            // claim stale; a vanished referent is stale too. Claims with
            // no checkable source report stale_checked=false.
            let mut stale = false;
            let mut checked = false;
            if e.has("source") {
                if let Ok(src) = e.try_get_object("source") {
                    if src.has("lib") && src.has("ctl") && src.has("facet") && src.has("hash") {
                        checked = true;
                        let slib = src.get_string("lib");
                        let sid = ctl_id(&store, &slib, &src.get_string("ctl"));
                        if !sid.is_empty() && store.exists(&slib, &sid) {
                            let sdata = store.get_data(&slib, &sid).get_object("data");
                            let sfacet = src.get_string("facet");
                            if sdata.has(&sfacet) {
                                let content = sdata.get_string(&sfacet).replace("\r", "");
                                stale = content_hash(&content) != src.get_string("hash");
                            } else {
                                stale = true;
                            }
                        } else {
                            stale = true;
                        }
                    }
                }
            }
            out.put_boolean("stale", stale);
            out.put_boolean("stale_checked", checked);
            hits.push((score, t0, out));
        }
    }
}
hits.sort_by(|x, y| y.0.cmp(&x.0).then(y.1.cmp(&x.1)));
let mut claims = DataArray::new();
for (score, _, c) in hits.iter().take(cap) {
    let mut c = c.clone();
    c.put_int("score", *score);
    claims.push_object(c);
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("considered", considered);
o.put_int("matched", hits.len() as i64);
o.put_array("claims", claims);
o

}
