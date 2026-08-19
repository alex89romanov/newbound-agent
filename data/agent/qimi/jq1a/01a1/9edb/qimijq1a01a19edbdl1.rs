// agent-executive-consolidate_room - acoustic consolidation (harvest
// H4). Executive-side, NOT sensor-side: sensors stay procedural; the
// room's transcripts reach here as messages BY VENUE (never by
// sensor - the layering rule), and inference is the executive's job.
// Runs when conversation SUBSIDES: the quiet gate and the cursor make
// it a cheap no-op at any tick rate, so the drive can try it every
// act tick and only pay the frontier when there is a settled window
// to understand. Products are CLAIMS in kb.environment - inferred,
// hysteresis-guarded via adjudicate, owner-auditable - including the
// voiceprint->person bindings the contract always promised ("I'm
// Marc" heard in the room becomes the claim that binds the print).
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
fn hhmm(t: i64) -> String {
    format!("{:02}:{:02}", (t / 3_600_000) % 24, (t / 60_000) % 60)
}
if min_quiet_s < 0 || window < 1 || budget < 1 {
    return err("min_quiet_s >= 0, window >= 1, budget >= 1".to_string());
}
let store = DataStore::new();
let now = time();

// the cursor: consolidate each utterance once, across restarts
let mut cursor_rec = if store.exists("runtime", "room_consolidation") {
    store.get_data("runtime", "room_consolidation")
} else {
    let mut r = DataObject::new();
    r.put_string("id", "room_consolidation");
    r.put_string("username", "system");
    r.put_array("readers", DataArray::new());
    r.put_array("writers", DataArray::new());
    let mut d = DataObject::new();
    d.put_int("last_t", 0);
    r.put_object("data", d);
    r
};
let mut cd = cursor_rec.get_object("data");
let last_t = if cd.has("last_t") { cd.get_int("last_t") } else { 0 };

let r = recent("room".to_string(), window);
if !r.has("messages") { return err("message index unreadable".to_string()); }
let msgs = r.get_array("messages");
let mut fresh: Vec<DataObject> = Vec::new();
let mut newest = 0i64;
let mut entities: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
for i in 0..msgs.len() {
    if let Ok(m) = msgs.try_get_object(i) {
        let t = if m.has("t") { m.get_int("t") } else { 0 };
        if t > newest { newest = t; }
        if t > last_t {
            if m.has("entity") && !m.get_string("entity").is_empty() {
                entities.insert(m.get_string("entity"));
            }
            fresh.push(m);
        }
    }
}
if fresh.is_empty() {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_string("skipped", "nothing new since the cursor");
    o.put_int("consolidated", 0);
    return o;
}
if now - newest < min_quiet_s * 1000 {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_string("skipped", "the room is still talking");
    o.put_int("quiet_for_s", (now - newest) / 1000);
    o.put_int("consolidated", 0);
    return o;
}

// the window, rendered with time and speaker - observations in, so
// conclusions can come out attributable
let mut transcript = String::new();
for m in &fresh {
    let who = if m.has("entity") && !m.get_string("entity").is_empty() {
        m.get_string("entity") } else { "unknown".to_string() };
    let t = if m.has("t") { m.get_int("t") } else { 0 };
    transcript.push_str(&format!("[{} {}] {}\n", hhmm(t), who,
        m.get_string("content").chars().take(400).collect::<String>()));
}
let subject = format!("household room conversation {}",
    entities.iter().cloned().collect::<Vec<_>>().join(" "));
let ctx = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    crate::agent::context::assemble::assemble("rumination".to_string(), subject.clone(), budget)
})).ok()
    .filter(|c| c.try_get_string("status").ok().as_deref() == Some("ok"))
    .map(|c| c.try_get_string("block").unwrap_or_default())
    .unwrap_or_default();

let prompt = format!(
    "You are an autonomous household agent consolidating what it heard. Below is the recent room transcript (speaker ids are voiceprint entities like vp-3 until a name is learned) and your existing knowledge.\nTRANSCRIPT:\n{}\nKNOWLEDGE (assembled, provenance-tagged):\n{}\nExtract durable CLAIMS: what happened, what was decided, what patterns recur, and any voiceprint-to-person binding revealed by the words themselves (someone addressed by name, or self-introducing). Only claims the transcript actually supports - no speculation. Reply with ONLY a JSON array, no fences, 0 to 6 items:\n[{{\"claim\": \"<one durable standalone sentence>\", \"entity\": \"<vp-id or person name, or empty>\", \"kind\": \"happened|decided|pattern|binding\"}}]",
    transcript, ctx);
let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    ask_llm(prompt, Data::DNull)
})).unwrap_or_else(|_| "ERROR: ask_llm panicked".to_string());
if reply.starts_with("ERROR") {
    return err(format!("the frontier arm failed: {}", reply.chars().take(200).collect::<String>()));
}
let parsed = reply.find('[').and_then(|s0| reply.rfind(']').map(|e0| (s0, e0)))
    .filter(|(s0, e0)| e0 > s0)
    .and_then(|(s0, e0)| {
        DataObject::try_from_string(&format!("{{\"a\":{}}}", &reply[s0..=e0])).ok()
    })
    .and_then(|w| w.try_get_array("a").ok());
let list = match parsed {
    Some(l) => l,
    None => {
        // spent but unparseable: move the cursor anyway - reasking the
        // same window would loop the spend; the words are still in the
        // message store for a smarter pass later
        cd.put_int("last_t", newest);
        cursor_rec.put_object("data", cd);
        cursor_rec.put_int("time", now);
        store.set_data("runtime", "room_consolidation", cursor_rec);
        let mut o = DataObject::new();
        o.put_string("status", "ok");
        o.put_int("consolidated", 0);
        o.put_boolean("unparseable", true);
        return o;
    }
};

let mut deposited = 0i64;
let mut held = 0i64;
for i in 0..list.len() {
    if let Ok(c) = list.try_get_object(i) {
        if !c.has("claim") || c.get_string("claim").trim().is_empty() { continue; }
        let kind = if c.has("kind") { c.get_string("kind") } else { "happened".to_string() };
        let mut entry = DataObject::new();
        entry.put_string("claim", c.get_string("claim").trim());
        entry.put_string("tags", &format!("environment,inferred,{}", kind));
        entry.put_string("confidence", "low");
        if c.has("entity") && !c.get_string("entity").trim().is_empty() {
            entry.put_string("detail", &format!("entity: {}", c.get_string("entity").trim()));
        }
        let adj = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            adjudicate("kb".to_string(), "environment".to_string(),
                       entry.deep_copy(), "consolidate_room".to_string())
        }));
        if let Ok(a) = adj {
            if a.try_get_string("status").ok().as_deref() == Some("ok") { deposited += 1; }
            else { held += 1; }
        } else { held += 1; }
    }
}
cd.put_int("last_t", newest);
cursor_rec.put_object("data", cd);
cursor_rec.put_int("time", now);
store.set_data("runtime", "room_consolidation", cursor_rec);

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("consolidated", fresh.len() as i64);
o.put_int("claims_deposited", deposited);
o.put_int("claims_held", held);
o.put_int("entities", entities.len() as i64);
o.put_string("domain", "kb.environment");
o
