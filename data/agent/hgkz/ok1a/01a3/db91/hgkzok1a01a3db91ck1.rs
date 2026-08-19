// agent-model-harvest_report - the gauge cluster, pointed at the
// harvest (harvest H6). One read-only sweep of everything the cycle
// grows: claims by domain and author in the window, the banks' row
// counts, capture volume, the message universe, notions pending the
// owner's audit, wonderings, the garden's act traces, and the
// assembled-context token trend - the syspack-shrinkage gauge, the
// number the whole flywheel exists to push DOWN. What gets MEASURED
// gets grown; this is the measuring.
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
if window_days < 1 || window_days > 90 {
    return err("window_days must be 1..=90".to_string());
}
let store = DataStore::new();
let now = time();
let cutoff = now - window_days * 86_400_000;
let root = match store.root.canonicalize().ok()
        .and_then(|r| r.parent().map(|p| p.to_path_buf())) {
    Some(r) => r,
    None => { return err("cannot resolve the checkout root".to_string()); }
};

// ── claims + traces, swept from every library's memory domains ──────
let mut libs: Vec<String> = Vec::new();
if let Ok(rd) = std::fs::read_dir(&store.root) {
    for e in rd.flatten() {
        if e.path().is_dir() {
            if let Ok(n) = e.file_name().into_string() { libs.push(n); }
        }
    }
}
libs.sort();
let mut by_domain = DataObject::new();
let mut by_author = DataObject::new();
let mut claims_in_window = 0i64;
let mut acts = DataObject::new();
let mut notions_pending = 0i64;
for lib in &libs {
    if !store.exists(lib, "controls") { continue; }
    let list = store.get_data(lib, "controls").get_object("data").get_array("list");
    for i in 0..list.len() {
        let item = match list.try_get_object(i) { Ok(x) => x, Err(_) => continue };
        if !item.has("name") || !item.has("id") { continue; }
        let (name, id) = (item.get_string("name"), item.get_string("id"));
        if !store.exists(lib, &id) { continue; }
        let dd = store.get_data(lib, &id).get_object("data");
        let home = format!("{}.{}", lib, name);
        if dd.has("memory") {
            if let Ok(w) = DataObject::try_from_string(&format!("{{\"a\":{}}}", dd.get_string("memory").replace('\r', ""))) {
                if let Ok(a) = w.try_get_array("a") {
                    for j in 0..a.len() {
                        if let Ok(e) = a.try_get_object(j) {
                            let superseded = e.has("superseded");
                            if lib == "kb" && name == "notions" && !superseded {
                                notions_pending += 1;
                            }
                            let t = if e.has("time") { e.get_int("time") } else { 0 };
                            if t >= cutoff && !superseded {
                                claims_in_window += 1;
                                let c = if by_domain.has(&home) { by_domain.get_int(&home) } else { 0 };
                                by_domain.put_int(&home, c + 1);
                            }
                        }
                    }
                }
            }
        }
        // authorship lives in the journals, not the entries: count
        // memory-facet patches by author within the window
        let jid = format!("{}_patches", id);
        if store.exists(lib, &jid) {
            let jd = store.get_data(lib, &jid).get_object("data");
            if jd.has("list") {
                let jl = jd.get_array("list");
                for j in 0..jl.len() {
                    if let Ok(pe) = jl.try_get_object(j) {
                        let f = if pe.has("facet") { pe.get_string("facet") } else { String::new() };
                        let tt = if pe.has("time") { pe.get_int("time") } else { 0 };
                        if f == "memory" && tt >= cutoff {
                            let author = if pe.has("author") && !pe.get_string("author").is_empty() {
                                pe.get_string("author") } else { "unknown".to_string() };
                            let c = if by_author.has(&author) { by_author.get_int(&author) } else { 0 };
                            by_author.put_int(&author, c + 1);
                        }
                    }
                }
            }
        }
        if dd.has("traces") {
            if let Ok(w) = DataObject::try_from_string(&format!("{{\"a\":{}}}", dd.get_string("traces").replace('\r', ""))) {
                if let Ok(a) = w.try_get_array("a") {
                    for j in 0..a.len() {
                        if let Ok(t) = a.try_get_object(j) {
                            let tt = if t.has("time") { t.get_int("time") } else { 0 };
                            if tt >= cutoff {
                                let author = if t.has("author") { t.get_string("author") } else { "unknown".to_string() };
                                let c = if acts.has(&author) { acts.get_int(&author) } else { 0 };
                                acts.put_int(&author, c + 1);
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── the banks ───────────────────────────────────────────────────────
let mut banks = DataArray::new();
if store.exists("runtime", "datasets") {
    let d = store.get_data("runtime", "datasets").get_object("data");
    if d.has("list") {
        let list = d.get_array("list");
        for i in 0..list.len() {
            if let Ok(m) = list.try_get_object(i) {
                let mut b = DataObject::new();
                b.put_string("name", &m.get_string("name"));
                b.put_string("kind", &(if m.has("kind") { m.get_string("kind") } else { String::new() }));
                b.put_int("rows", if m.has("rows") { m.get_int("rows") } else { 0 });
                banks.push_object(b);
            }
        }
    }
}

// ── capture volume + the message universe ───────────────────────────
let mut capture_rows = 0i64;
let cap_dir = root.join("runtime").join("agent").join("model").join("capture");
if let Ok(rd) = std::fs::read_dir(&cap_dir) {
    for e in rd.flatten() {
        if let Ok(text) = std::fs::read_to_string(e.path()) {
            capture_rows += text.lines().filter(|l| !l.trim().is_empty()).count() as i64;
        }
    }
}
let mut msg_total = 0i64;
let mut msg_by_venue = DataObject::new();
if let Ok(text) = std::fs::read_to_string(root.join("runtime").join("agent").join("msg").join("index.jsonl")) {
    for ln in text.lines() {
        if ln.trim().is_empty() { continue; }
        msg_total += 1;
        if let Ok(r) = DataObject::try_from_string(ln) {
            if r.has("venue") {
                let v = r.get_string("venue");
                let c = if msg_by_venue.has(&v) { msg_by_venue.get_int(&v) } else { 0 };
                msg_by_venue.put_int(&v, c + 1);
            }
        }
    }
}

// ── wonderings ──────────────────────────────────────────────────────
let mut wonderings = DataArray::new();
if store.exists("runtime", "wonderings") {
    let d = store.get_data("runtime", "wonderings").get_object("data");
    if d.has("list") {
        let list = d.get_array("list");
        for i in 0..list.len() {
            if let Ok(w) = list.try_get_object(i) {
                if w.has("q") { wonderings.push_string(&w.get_string("q")); }
            }
        }
    }
}

// ── the syspack gauge: assembled-context tokens per purpose ─────────
let mut ctx_calls = 0i64;
let mut ctx_by_purpose = DataObject::new();
if let Ok(text) = std::fs::read_to_string(root.join("runtime").join("agent").join("model").join("metrics.jsonl")) {
    for ln in text.lines() {
        if !ln.contains("\"kind\":\"context\"") { continue; }
        if let Ok(r) = DataObject::try_from_string(ln) {
            let t = if r.has("t") { r.get_int("t") } else { 0 };
            if t < cutoff { continue; }
            ctx_calls += 1;
            let p = if r.has("purpose") { r.get_string("purpose") } else { "?".to_string() };
            let tok = if r.has("tokens") { r.get_int("tokens") } else { 0 };
            let mut row = if ctx_by_purpose.has(&p) { ctx_by_purpose.get_object(&p) } else {
                let mut x = DataObject::new();
                x.put_int("calls", 0);
                x.put_int("tokens_sum", 0);
                x
            };
            row.put_int("calls", row.get_int("calls") + 1);
            row.put_int("tokens_sum", row.get_int("tokens_sum") + tok);
            ctx_by_purpose.put_object(&p, row);
        }
    }
}
// averages, not sums: the gauge is tokens-per-good-answer
for p in ctx_by_purpose.clone().keys() {
    let mut row = ctx_by_purpose.get_object(&p);
    let calls = row.get_int("calls");
    if calls > 0 { row.put_int("tokens_avg", row.get_int("tokens_sum") / calls); }
    ctx_by_purpose.put_object(&p, row);
}

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("window_days", window_days);
o.put_int("claims", claims_in_window);
o.put_object("claims_by_domain", by_domain);
o.put_object("memory_writes_by_author", by_author);
o.put_object("acts", acts);
o.put_int("notions_pending", notions_pending);
o.put_array("wonderings", wonderings);
o.put_array("banks", banks);
o.put_int("capture_rows", capture_rows);
o.put_int("messages", msg_total);
o.put_object("messages_by_venue", msg_by_venue);
o.put_int("context_calls", ctx_calls);
o.put_object("context_by_purpose", ctx_by_purpose);
o
