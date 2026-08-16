// metrics: the mind tab's trends, read from the service's metrics
// journal (runtime/agent/model/metrics.jsonl - every served verdict,
// loss samples, every gate). Returns a downsampled loss series, the
// recent gate history with a pass count, and a 10-bucket histogram of
// served verdicts. Read-only; the journal is instance-owned and capped
// by the service itself.
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
let root = DataStore::new().root;
let root = match root.canonicalize() {
    Ok(r) => r,
    Err(e) => { return err(format!("store root: {}", e)); }
};
let root = match root.parent() {
    Some(p) => p.to_path_buf(),
    None => { return err("store root has no parent".to_string()); }
};
let path = root.join("runtime").join("agent").join("model").join("metrics.jsonl");
let mut o = DataObject::new();
o.put_string("status", "ok");
let text = std::fs::read_to_string(&path).unwrap_or_default();
let lines: Vec<&str> = text.lines().collect();
let start = lines.len().saturating_sub(4000);

let mut loss_rows: Vec<(i64, f64)> = Vec::new();
let mut gates = DataArray::new();
let mut gate_total = 0i64;
let mut gate_pass = 0i64;
let mut hist = [0i64; 10];
let mut verdict_total = 0i64;
let mut first_t: i64 = 0;
let mut last_t: i64 = 0;

fn as_f(o: &DataObject, k: &str) -> Option<f64> {
    if !o.has(k) { return None; }
    match o.get_property(k) {
        ndata::data::Data::DFloat(f) => Some(f),
        ndata::data::Data::DInt(i) => Some(i as f64),
        _ => None,
    }
}

for l in &lines[start..] {
    let d = match DataObject::try_from_string(l) { Ok(d) => d, Err(_) => continue };
    let kind = if d.has("kind") { d.get_string("kind") } else { continue };
    let t = as_f(&d, "t").unwrap_or(0.0) as i64;
    if first_t == 0 { first_t = t; }
    if t > last_t { last_t = t; }
    if kind == "loss" {
        if let (Some(s), Some(v)) = (as_f(&d, "step"), as_f(&d, "loss")) {
            loss_rows.push((s as i64, v));
        }
    } else if kind == "gate" {
        gate_total += 1;
        let v = if d.has("verdict") { d.get_string("verdict") } else { String::new() };
        if v == "promote" { gate_pass += 1; }
        if gates.len() >= 20 { gates.remove_property(0); }
        gates.push_object(d.deep_copy());
    } else if kind == "verdict" {
        if let Some(s) = as_f(&d, "sal") {
            let b = ((s * 10.0).floor() as usize).min(9);
            hist[b] += 1;
            verdict_total += 1;
        }
    }
}

// downsample loss to <=100 points, preserving order
let mut loss = DataArray::new();
let stride = (loss_rows.len() / 100).max(1);
for (i, (s, v)) in loss_rows.iter().enumerate() {
    if i % stride == 0 || i == loss_rows.len() - 1 {
        let mut r = DataObject::new();
        r.put_int("step", *s);
        r.put_float("loss", *v);
        loss.push_object(r);
    }
}
let mut ha = DataArray::new();
for h in hist.iter() { ha.push_int(*h); }
o.put_array("loss", loss);
o.put_array("gates", gates);
o.put_int("gate_total", gate_total);
o.put_int("gate_pass", gate_pass);
o.put_array("verdict_hist", ha);
o.put_int("verdict_total", verdict_total);
o.put_int("span_ms", if last_t > first_t { last_t - first_t } else { 0 });
o
