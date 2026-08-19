// start (docs/perception-contract.md section 5): the journal tailer, the
// contract's reference implementation - purely procedural end to end.
// Explicit and killable like the executive. Every sweep stats each
// control's _patches journal (mtime gate - journals parse only when they
// actually moved), turns each new entry into one store_change envelope,
// binds the claims whose source pointers name the changed facet (stale by
// hash compare), and hands it to agent-executive-perceive. Because acts
// go through platform commands, the executive's own writes return here:
// the self-model, for free.
// Sensor runtime state under one globals key (the executive's pattern).
// The cursor is runtime state for now - it resets to `now` on start, so a
// restart never replays history; a persisted cursor record arrives with
// the sensor-state work the contract reserves for it.
fn ensure_sensor_state(g: &mut DataObject) -> DataObject {
    if !g.has("AGENT_SENSOR_STORE") {
        let mut st = DataObject::new();
        st.put_boolean("running", false);
        st.put_int("cursor", 0);
        st.put_int("emitted_total", 0);
        st.put_int("started", 0);
        st.put_string("last_label", "");
        st.put_int("last_bound", 0);
        g.put_object("AGENT_SENSOR_STORE", st);
    }
    g.get_object("AGENT_SENSOR_STORE")
}

fn seg(id: &str, i: usize) -> String {
    // The store's path law: 4-char segments of the id, '_'-padded short.
    let mut s: String = id.chars().skip(i).take(4).collect();
    while s.len() < 4 { s.push('_'); }
    s
}
fn content_hash(s: &str) -> String {
    // FNV-1a over the \r-normalized source - MUST stay in sync with
    // remember/recall/read_control_facet/patch_control_facet.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}
// Claims whose source pointers name the changed lib.ctl facet, stale by
// comparing the stamped hash to the referent's current content hash.
fn bind_claims(store: &DataStore, chg_lib: &str, chg_ctl: &str, chg_facet: &str) -> DataArray {
    let mut out = DataArray::new();
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
            let id = item.get_string("id");
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
                if !e.has("claim") || !e.has("source") { continue; }
                let src = match e.try_get_object("source") { Ok(s) => s, Err(_) => continue };
                if !src.has("lib") || !src.has("ctl") || !src.has("facet") || !src.has("hash") { continue; }
                if src.get_string("lib") != chg_lib || src.get_string("ctl") != chg_ctl { continue; }
                if !chg_facet.is_empty() && src.get_string("facet") != chg_facet { continue; }
                // referent's current content vs the stamped hash
                let mut stale = true;
                let sfacet = src.get_string("facet");
                let mut sid = String::new();
                if store.exists(chg_lib, "controls") {
                    let cl = store.get_data(chg_lib, "controls").get_object("data").get_array("list");
                    for k in 0..cl.len() {
                        let c = cl.get_object(k);
                        if c.has("name") && c.get_string("name") == chg_ctl { sid = c.get_string("id"); break; }
                    }
                }
                if !sid.is_empty() && store.exists(chg_lib, &sid) {
                    let sdata = store.get_data(chg_lib, &sid).get_object("data");
                    if sdata.has(&sfacet) {
                        let content = sdata.get_string(&sfacet).replace("\r", "");
                        stale = content_hash(&content) != src.get_string("hash");
                    }
                }
                let mut c = DataObject::new();
                c.put_string("lib", &lib);
                c.put_string("ctl", &item.get_string("name"));
                c.put_string("claim", &e.get_string("claim"));
                c.put_boolean("stale", stale);
                out.push_object(c);
            }
        }
    }
    out
}

let mut g = DataStore::globals();
let mut st = ensure_sensor_state(&mut g);
if st.get_boolean("running") {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_boolean("already_running", true);
    return o;
}
st.put_boolean("running", true);
st.put_int("cursor", time());
st.put_int("started", time());

std::thread::spawn(move || {
    let g = DataStore::globals();
    loop {
        let st = g.get_object("AGENT_SENSOR_STORE");
        if !st.get_boolean("running") { break; }
        let mut st = st;
        let cursor = st.get_int("cursor");
        let t0 = time();
        let store = DataStore::new();
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
                let jid = format!("{}_patches", id);
                // mtime gate: parse a journal only when its file moved
                let jpath = store.root.join(&lib)
                    .join(seg(&jid, 0)).join(seg(&jid, 4)).join(seg(&jid, 8)).join(seg(&jid, 12))
                    .join(&jid);
                let mt = match std::fs::metadata(&jpath) {
                    Ok(m) => match m.modified() {
                        Ok(t) => match t.duration_since(std::time::UNIX_EPOCH) {
                            Ok(d) => d.as_millis() as i64,
                            Err(_) => continue,
                        },
                        Err(_) => continue,
                    },
                    Err(_) => continue,
                };
                if mt <= cursor { continue; }
                if !store.exists(&lib, &jid) { continue; }
                let jd = store.get_data(&lib, &jid).get_object("data");
                if !jd.has("list") { continue; }
                let jlist = jd.get_array("list");
                for j in 0..jlist.len() {
                    let e = match jlist.try_get_object(j) { Ok(e) => e, Err(_) => continue };
                    let et = if e.has("time") { e.get_int("time") } else { 0 };
                    if et <= cursor || et > t0 { continue; }
                    let facet = if e.has("facet") { e.get_string("facet") } else { String::new() };
                    let mut payload = DataObject::new();
                    payload.put_string("lib", &lib);
                    payload.put_string("ctl", &name);
                    payload.put_string("id", &id);
                    if !facet.is_empty() { payload.put_string("facet", &facet); }
                    let mut patch = DataObject::new();
                    if e.has("patch_id") { patch.put_string("id", &e.get_string("patch_id")); }
                    if e.has("label") { patch.put_string("label", &e.get_string("label")); }
                    if e.has("author") { patch.put_string("author", &e.get_string("author")); }
                    payload.put_object("patch", patch);
                    let claims = bind_claims(&store, &lib, &name, &facet);
                    let mut env = DataObject::new();
                    env.put_int("v", 1);
                    env.put_string("kind", "store_change");
                    env.put_int("time", et);
                    env.put_string("sensor", "store");
                    env.put_object("payload", payload);
                    let nbound = claims.len() as i64;
                    env.put_array("claims", claims);
                    let delivered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let cmd = Command::lookup("agent", "executive", "perceive");
                        let mut args = DataObject::new();
                        args.put_object("perception", env);
                        cmd.execute(args)
                    }));
                    if let Ok(Ok(_)) = delivered {
                        st.put_int("emitted_total", st.get_int("emitted_total") + 1);
                        let label = if e.has("label") { e.get_string("label") } else { format!("{}.{}", lib, name) };
                        st.put_string("last_label", &label);
                        st.put_int("last_bound", nbound);
                    }
                }
            }
        }
        st.put_int("cursor", t0);
        // the system sensor rides the same loop (H4): one sweep every
        // 15 ticks (~30s) - one sensor family, one loop, no second
        // scheduler. The sweep itself coalesces (band crossings only),
        // so this cadence sets latency, not volume.
        let ticks = if st.has("sys_ticks") { st.get_int("sys_ticks") } else { 0 } + 1;
        st.put_int("sys_ticks", ticks);
        if ticks % 15 == 0 {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::agent::sensor::system_sense::system_sense()
            }));
        }
        std::thread::sleep(std::time::Duration::from_millis(2000));
    }
});

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_boolean("already_running", false);
o
