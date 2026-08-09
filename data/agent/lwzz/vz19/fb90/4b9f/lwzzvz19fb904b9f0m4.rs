// The archivist (docs/memory.md): drains the runtime turn queue, asks the
// LLM what is durable and NOT already known, and files survivors through
// dev.code.remember - validated, journaled, author "archivist", tagged
// unreviewed. The queue clears only after the LLM answers: an LLM failure
// leaves it intact for the next sweep. Fired by this control's 30-minute
// timer; runnable by hand any time. Takes NO params - timers fire with
// empty args.
let store = DataStore::new();
if !store.exists("runtime", "archivist_queue") {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_int("swept", 0);
    o.put_int("filed", 0);
    return o;
}
let mut qrec = store.get_data("runtime", "archivist_queue");
let mut qd = qrec.get_object("data");
let turns = if qd.has("turns") { qd.get_array("turns") } else { DataArray::new() };
if turns.len() == 0 {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_int("swept", 0);
    o.put_int("filed", 0);
    return o;
}

// The extraction prompt is this control's own `prompt` facet - journaled,
// tunable without a rebuild. Missing = fail loud, never a silent no-op.
let api = crate::api::new();
let selfid = api.dev.editcontrol.lookup_id("agent".to_string(), "archivist".to_string());
let sp = store.get_data("agent", &selfid).get_object("data");
if !sp.has("prompt") || sp.get_string("prompt").trim().is_empty() {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", "agent.archivist has no `prompt` facet - the extraction prompt is store-resident (docs/memory.md)");
    return o;
}
let extraction = sp.get_string("prompt");

// What is already known: the kb index plus every existing claim, so the
// LLM can skip restatements.
let mut known = String::new();
if store.exists("kb", "controls") {
    let list = store.get_data("kb", "controls").get_object("data").get_array("list");
    for i in 0..list.len() {
        let item = list.get_object(i);
        let name = item.get_string("name");
        let id = item.get_string("id");
        let dd = store.get_data("kb", &id).get_object("data");
        let desc = if dd.has("desc") { dd.get_string("desc") } else { String::new() };
        known.push_str(&format!("\nDOMAIN kb.{} - {}\n", name, desc));
        if dd.has("memory") {
            if let Ok(w) = DataObject::try_from_string(&format!("{{\"a\":{}}}", dd.get_string("memory"))) {
                if let Ok(a) = w.try_get_array("a") {
                    for j in 0..a.len() {
                        if let Ok(e) = a.try_get_object(j) {
                            if e.has("claim") {
                                known.push_str(&format!("- {}\n", e.get_string("claim")));
                            }
                        }
                    }
                }
            }
        }
    }
}

let turns_json = turns.to_string();
let user = format!(
    "EXISTING MEMORY (do not re-file anything here, restated or rephrased):\n{}\n\nRECENT TURNS:\n{}\n\nReply with ONLY a JSON array (at most 5 items; [] is the correct answer for most sweeps) of {{\"domain\": \"<existing kb domain, or a month bucket like m2026-08>\", \"entry\": {{\"claim\": \"...\", \"detail\": \"...\", \"tags\": \"a,b\", \"confidence\": \"high|medium|low\"}}}}.",
    known, turns_json);
let resp = ask_llm(user, Data::DString(extraction));

// A FAILED LLM call must not consume the queue. ask_llm's contract: every
// terminal failure is a string starting with "ERROR: " (provider config,
// LLM_CTL dispatch, or the wire loop's exhausted retries alike).
if resp.starts_with("ERROR:") {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &resp.chars().take(400).collect::<String>());
    return o;
}

// The LLM answered - this sweep consumes the turns regardless of yield.
let swept = turns.len() as i64;
qd.put_array("turns", DataArray::new());
qrec.put_object("data", qd);
qrec.put_int("time", time());
store.set_data("runtime", "archivist_queue", qrec);

// Parse the first [...] block; unparsable output files nothing.
let mut filed: i64 = 0;
let mut skipped: i64 = 0;
if let (Some(i0), Some(i1)) = (resp.find('['), resp.rfind(']')) {
    if i1 > i0 {
        if let Ok(w) = DataObject::try_from_string(&format!("{{\"a\":{}}}", &resp[i0..=i1])) {
            if let Ok(items) = w.try_get_array("a") {
                let cmd = Command::lookup("dev", "code", "remember");
                for i in 0..items.len() {
                    if filed >= 5 {
                        skipped += (items.len() - i) as i64;
                        break;
                    }
                    let it = match items.try_get_object(i) {
                        Ok(x) => x,
                        Err(_) => { skipped += 1; continue; }
                    };
                    if !it.has("domain") || !it.has("entry") { skipped += 1; continue; }
                    let mut entry = match it.try_get_object("entry") {
                        Ok(e) => e,
                        Err(_) => { skipped += 1; continue; }
                    };
                    // provenance: machine-filed memories are marked; the
                    // owner's audit clears the tag or reverts the entry
                    let tags = if entry.has("tags") { entry.get_string("tags") } else { String::new() };
                    if !tags.split(',').any(|x| x.trim() == "unreviewed") {
                        let nt = if tags.trim().is_empty() { "unreviewed".to_string() }
                                 else { format!("{},unreviewed", tags) };
                        entry.put_string("tags", &nt);
                    }
                    let mut args = DataObject::new();
                    args.put_string("lib", "kb");
                    args.put_string("domain", &it.get_string("domain"));
                    args.put_object("entry", entry);
                    args.put_string("author", "archivist");
                    match cmd.execute(args) {
                        Ok(r) => {
                            if r.has("a") && r.get_object("a").has("status")
                                && r.get_object("a").get_string("status") == "ok" {
                                filed += 1;
                            } else {
                                skipped += 1;
                            }
                        }
                        Err(_) => { skipped += 1; }
                    }
                }
            }
        }
    }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("swept", swept);
o.put_int("filed", filed);
o.put_int("skipped", skipped);
o
