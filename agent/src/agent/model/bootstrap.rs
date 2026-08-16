use ndata::dataobject::DataObject;
use flowlang::datastore::DataStore;
use ndata::dataarray::DataArray;
use flowlang::flowlang::system::system_call::system_call;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        bootstrap()
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

pub fn bootstrap() -> DataObject {
// bootstrap: the agent builds its own nanochat server (owner directive,
// 2026-08-16). Idempotent and settings-driven - no seams, no runbook
// steps: SALIENCE=on in botd.properties turns the subsystem on, and the
// executive fires this once per start whenever the service isn't
// answering. Modeled on the owner's oneshot-installer idiom: donefile-
// guarded stages, everything under the instance's own runtime/, nothing
// at a hardcoded absolute path.
//
//   MODEL_CHECKPOINT    stub (default) | path to a nanochat checkpoint
//   MODEL_SERVICE_PORT  8077 (default)
//   NANOCHAT_REPO       https://github.com/karpathy/nanochat.git
//
// Stub mode installs nothing (the service is stdlib python); the
// nanochat environment (clone + venv + deps) is only built when a real
// checkpoint is configured - that is the GPU box's path. The service
// script ships as a library asset compiled into this dylib
// (data/agent/_ASSETS/service.py): the platform carries its own server
// and rewrites the on-disk copy whenever the shipped one differs.
fn prop(key: &str, dflt: &str) -> String {
    // Settings live in runtime/agent/botd.properties like everything else.
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

let checkpoint = prop("MODEL_CHECKPOINT", "stub");
let port = prop("MODEL_SERVICE_PORT", "8077");
let repo = prop("NANOCHAT_REPO", "https://github.com/karpathy/nanochat.git");

let root = DataStore::new().root;
let root = match root.canonicalize() {
    Ok(r) => r,
    Err(e) => { return err(format!("store root: {}", e)); }
};
let root = match root.parent() {
    Some(p) => p.to_path_buf(),
    None => { return err("store root has no parent".to_string()); }
};
let modeldir = root.join("runtime").join("model");
let deps = modeldir.join("deps");
let _ = std::fs::create_dir_all(&deps);

let mut o = DataObject::new();
o.put_string("status", "ok");

// stage 1: the nanochat environment - only for a real checkpoint
let mut nanochat_env = "not_needed".to_string();
if checkpoint != "stub" {
    let nc = deps.join("nanochat");
    let donefile = nc.join("oneshot_ready");
    if donefile.exists() && nc.join("venv").exists() {
        nanochat_env = "ready".to_string();
    } else {
        if nc.exists() { let _ = std::fs::remove_dir_all(&nc); }
        let mut cmd = "set -e; cd ".to_string();
        cmd += &deps.display().to_string();
        cmd += &format!("; git clone {} nanochat", repo);
        cmd += "; cd nanochat; python3 -m venv venv; source venv/bin/activate";
        cmd += "; pip install --upgrade pip setuptools wheel; pip install -e .";
        let mut x = DataArray::new();
        x.push_string("bash");
        x.push_string("-c");
        x.push_string(&cmd);
        let r = system_call(x);
        println!("BOOTSTRAP NANOCHAT ENV {}", r.to_string());
        if nc.join("venv").join("bin").join("python").exists() {
            let _ = std::fs::write(&donefile, "x");
            nanochat_env = "installed".to_string();
        } else {
            nanochat_env = "install_failed".to_string();
            o.put_string("status", "err");
        }
    }
}
o.put_string("nanochat_env", &nanochat_env);

// stage 2: the service script, from the compiled-in asset
let svc = modeldir.join("service.py");
let asset = include_str!("../../../../data/agent/_ASSETS/service.py");
let current = std::fs::read_to_string(&svc).unwrap_or_default();
if current != asset {
    if let Err(e) = std::fs::write(&svc, asset) {
        return err(format!("could not write {}: {}", svc.display(), e));
    }
    o.put_boolean("script_written", true);
} else {
    o.put_boolean("script_written", false);
}

// stage 3: launch if it isn't answering
let status_url = format!("{}/status", service_url());
let probe = || ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(800))
    .build()
    .get(&status_url)
    .call()
    .is_ok();
let mut service = "already_running".to_string();
if !probe() {
    let py = if checkpoint == "stub" {
        "python3".to_string()
    } else {
        deps.join("nanochat").join("venv").join("bin").join("python").display().to_string()
    };
    let mut cmd = "cd ".to_string();
    cmd += &root.display().to_string();
    cmd += &format!(
        "; nohup '{}' runtime/model/service.py --data-dir runtime/model --port {} --checkpoint '{}' >> runtime/model/service.log 2>&1 &",
        py, port, checkpoint);
    let mut x = DataArray::new();
    x.push_string("bash");
    x.push_string("-c");
    x.push_string(&cmd);
    let r = system_call(x);
    println!("BOOTSTRAP LAUNCH MODEL SERVICE {}", r.to_string());
    std::thread::sleep(std::time::Duration::from_millis(900));
    if probe() {
        service = "launched".to_string();
    } else {
        service = "launch_failed".to_string();
        o.put_string("status", "err");
    }
}
o.put_string("service", &service);
o.put_string("checkpoint", &checkpoint);
o.put_string("port", &port);
o.put_string("path", &modeldir.display().to_string());
o

}
