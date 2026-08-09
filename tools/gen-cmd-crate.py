#!/usr/bin/env python3
"""Write the empty `cmd` crate scaffold into a newbound checkout.

`/cmd` is gitignored in the newbound repo BECAUSE it is generated —
`newbound rebuild` emits it for whichever store libraries declare
`"root": "cmd"`. A checkout that redacts (or simply lacks) such a
library still needs the crate to exist so Cargo.toml's
`cmd = { path = "./cmd" }` resolves. This writes the empty scaffold the
builder would produce; a checkout that has the libraries regenerates
over it.

Usage: gen-cmd-crate.py <newbound-checkout-dir>
Refuses to touch an existing cmd/src (never overwrites real content).
"""
import os
import sys

root = sys.argv[1] if len(sys.argv) > 1 else "."
cmd = os.path.join(root, "cmd")
if not os.path.isfile(os.path.join(root, "Cargo.toml")):
    sys.exit("error: %s is not a newbound checkout (no Cargo.toml)" % root)
if os.path.isdir(os.path.join(cmd, "src")):
    print("cmd/src already exists — leaving it alone")
    sys.exit(0)

os.makedirs(os.path.join(cmd, "src"), exist_ok=True)
with open(os.path.join(cmd, "Cargo.toml"), "w") as f:
    f.write("""[package]
name = "cmd"
version = "0.1.0"
edition = "2021"

[dependencies]
flowlang = "0.3"
ndata = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[features]
serde_support = []
python_runtime = []
javascript_runtime = []
reload = []
default = []
""")
with open(os.path.join(cmd, "src", "lib.rs"), "w") as f:
    f.write("""// Empty generated scaffold: no store library roots here in this
// checkout. Exists only so Cargo.toml's `cmd = { path = "./cmd" }`
// resolves, and so the committed generated_initializer's
// `cmd::cmdinit(&mut cmds)` call links. `newbound rebuild` regenerates
// it where the libraries exist.
use flowlang::rustcmd::Transform;

pub fn cmdinit(_cmds: &mut Vec<(String, Transform, String)>) {}
""")
print("wrote empty cmd crate scaffold at", cmd)
