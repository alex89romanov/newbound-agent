#!/bin/sh
# One-command session setup: overlay this agent repo onto a newbound
# checkout, build everything, keep both git trees clean of builder-written
# local state, and probe the result.
#
#   path/to/newbound-agent/tools/setup.sh [path-to-newbound-checkout] [--no-probe]
#
# With no path argument the current directory is used if it is a newbound
# checkout; otherwise a `newbound` directory beside this repo.
#
# Idempotent: on a fully set-up checkout every step short-circuits and the
# whole run takes seconds. On a fresh clone it is the complete first-time
# sequence from the README (overlay -> cmd scaffold -> host build ->
# `newbound rebuild` -> host build -> dylibs), then the app staging the
# overlay otherwise misses (the platform's install_lib step: the server
# serves only apps listed in config.properties, and runtime/agent must
# carry the app shell), followed by the git hygiene that keeps regenerated
# local state out of accidental commits:
#
#   - newbound repo: Cargo.toml (workspace exclude), the generated
#     initializer, and newbound_core/src/api.rs are rewritten by
#     `newbound rebuild` and must never be committed there -> marked
#     skip-worktree (undo: git update-index --no-skip-worktree <file>).
#   - agent repo: the rebuild regenerates each FFI crate's src/api.rs
#     against the libraries present in THIS checkout, deleting stubs for
#     libraries that exist elsewhere (camera, hollis, ...). That churn is
#     environment-induced, not authored, so it is reverted -- unless the
#     file was already modified before setup ran, in which case it is
#     left alone as in-progress work.

set -e

AGENT_DIR=$(cd "$(dirname "$0")/.." && pwd)

NB=""
PROBE=yes
for arg in "$@"; do
  case "$arg" in
    --no-probe) PROBE=no ;;
    *) NB="$arg" ;;
  esac
done
if [ -z "$NB" ]; then
  if [ -f Cargo.toml ] && [ -d data ]; then
    NB=$(pwd)
  elif [ -d "$AGENT_DIR/../newbound/data" ]; then
    NB=$(cd "$AGENT_DIR/../newbound" && pwd)
  else
    echo "error: no newbound checkout found (pass its path, run from inside one, or keep one beside this repo)" >&2
    exit 1
  fi
fi
NB=$(cd "$NB" && pwd)
cd "$NB"
if [ ! -f Cargo.toml ] || [ ! -d data ]; then
  echo "error: $NB is not a newbound checkout (needs Cargo.toml and data/)" >&2
  exit 1
fi

echo "== setup: agent repo $AGENT_DIR onto checkout $NB"

# Remember which churn-prone agent-repo files carry pre-existing edits, so
# the hygiene step never reverts intentional work.
CHURN_FILES="agent/src/api.rs kb/src/api.rs"
DIRTY_BEFORE=$(cd "$AGENT_DIR" && git status --porcelain -- $CHURN_FILES)

# 1. Overlay (idempotent; also sets skip-worktree on the scratch skeleton).
"$AGENT_DIR/tools/overlay.sh" "$AGENT_DIR"

# 2. cmd crate scaffold, if this checkout ships without one.
if [ ! -d cmd/src ]; then
  "$AGENT_DIR/tools/gen-cmd-crate.py" .
fi

# Whether the host binary predates this run decides what can be said
# about MCP attachment at the end (step 10).
HOST_PREBUILT=no
[ -x target/release/newbound ] && HOST_PREBUILT=yes

# 3. Host build (first pass). No-op when already built.
cargo build --release --features=serde_support

# 4. Regenerate the initializer if it doesn't know the overlay crates yet,
#    then rebuild the host with the FFI blocks baked in.
if ! grep -q 'Initialize crate: agent' src/generated_initializer.rs 2>/dev/null \
  || ! grep -q 'Initialize crate: kb' src/generated_initializer.rs \
  || ! grep -q 'Initialize crate: scratch' src/generated_initializer.rs; then
  ./target/release/newbound rebuild
  cargo build --release --features=serde_support
else
  echo "== initializer already carries the overlay crates; skipping rebuild"
fi

# 5. The dylibs (fast no-ops when unchanged; hot-load into a running server).
(cd agent && cargo build --release --features=serde_support,python_runtime)
(cd kb && cargo build --release)
(cd scratch && cargo build --release)

# 6. Stage the agent app — what the platform's install_lib does on a real
#    install, which the overlay never runs. All of it is per-clone local
#    state: runtime/agent is excluded via .git/info/exclude (the tracked
#    .gitignore belongs to the platform and doesn't know the agent), and
#    config.properties is already gitignored.
if [ ! -d runtime/agent ]; then
  cp -r data/agent/_APPS/agent runtime/agent
  echo "== staged data/agent/_APPS/agent -> runtime/agent"
fi
if ! grep -qx '/runtime/agent' .git/info/exclude 2>/dev/null; then
  echo '/runtime/agent' >> .git/info/exclude
  echo "== added /runtime/agent to .git/info/exclude"
fi
if [ ! -f config.properties ]; then
  sed 's/^apps=.*/&,agent/' config.properties_example > config.properties
  echo "== created config.properties from the example, agent app enabled"
elif grep -Eq '^apps=(.*,)?agent(,|$)' config.properties; then
  : # agent already enabled
elif grep -q '^apps=' config.properties; then
  sed -i 's/^apps=.*/&,agent/' config.properties
  echo "== added agent to the apps list in config.properties"
else
  echo 'apps=app,dev,security,peer,agent' >> config.properties
  echo "== added an apps list with agent to config.properties"
fi
if grep -q '^http_port=0$' config.properties; then
  # http_port=0 is never a choice anyone made in a dev checkout — it is
  # the residue of a bare `newbound mcp` run auto-creating the file.
  # Any other value (a deliberate port) is left alone.
  sed -i 's/^http_port=0$/http_port=8080/' config.properties
  echo "== set http_port=8080 in config.properties (was the mcp-run default 0 = no HTTP listener)"
fi

# 7. Git hygiene, newbound side: builder-written local state stays invisible.
for f in Cargo.toml src/generated_initializer.rs newbound_core/src/api.rs; do
  git update-index --skip-worktree "$f" 2>/dev/null || true
done
echo "== skip-worktree set on the builder-written newbound files (undo: git update-index --no-skip-worktree <file>)"

# 8. Git hygiene, agent side: drop environment-induced api.rs regeneration.
(cd "$AGENT_DIR"
 for f in $CHURN_FILES; do
   case "$DIRTY_BEFORE" in
     *"$f"*) echo "== $f was already modified before setup; leaving it alone" ;;
     *) if [ -n "$(git status --porcelain -- "$f")" ]; then
          git checkout -- "$f"
          echo "== reverted environment-induced regeneration of $f"
        fi ;;
   esac
 done)

# 9. Top up the brain from the primer (docs/one-memory-cycle.md A4).
#    Idempotent (exact-claim dedupe) and best-effort: a fresh clone's brain
#    starts from the frozen kb snapshot, and the primer carries whatever
#    doctrine/process material was curated after the freeze.
if [ -f "$AGENT_DIR/docs/kb-seed.json" ]; then
  if "$AGENT_DIR/tools/nb-call.py" -C "$NB" agent-archivist-bootstrap \
       "{\"path\": \"$AGENT_DIR/docs/kb-seed.json\"}" >/dev/null 2>&1; then
    echo "== brain topped up from docs/kb-seed.json (idempotent)"
  else
    echo "== brain top-up skipped (bootstrap unavailable — run it once the binary serves mcp):"
    echo "==   tools/nb-call.py agent-archivist-bootstrap '{\"path\": \"$AGENT_DIR/docs/kb-seed.json\"}'"
  fi
fi

# 10. Prove it.
if [ "$PROBE" = yes ]; then
  "$AGENT_DIR/tools/overlay-probe.py" "$NB"
fi

echo "== setup complete: $NB serves the store via ./target/release/newbound mcp,"
echo "==   or run ./target/release/newbound for the web UI (agent app at /agent/index.html)"

# 11. Attachment sense. The server side is proven (the probe), but whether a
#     CLIENT holds the native attachment can't be read from here — harnesses
#     spawn .mcp.json servers only at their own startup. Infer what we can:
#     a live server process means some client is attached; none means none is.
MCP_PID=$(pgrep -f 'newbound mcp$' | head -1 || true)
if [ -n "$MCP_PID" ]; then
  echo "== MCP: a live 'newbound mcp' process exists (pid $MCP_PID) — a client is attached"
else
  if [ "$HOST_PREBUILT" = no ]; then
    echo "== MCP: no client is attached — none could be: the newbound binary was built by"
    echo "==   THIS run, and clients spawn .mcp.json servers only when they start."
  else
    echo "== MCP: no live 'newbound mcp' process — no client is currently attached."
  fi
  if [ "${CLAUDE_CODE_REMOTE:-}" = "true" ]; then
    echo "==   This is a Claude Code web session: attachment happens only at session start"
    echo "==   and builds don't persist between sessions, so set the environment's setup"
    echo "==   script to newbound-agent/tools/setup.sh to make every session attach"
    echo "==   natively. Until then, drive the store with tools/nb-call.py (same surface)."
  else
    echo "==   Claude Code (CLI/IDE) reads this checkout's .mcp.json — restart it or"
    echo "==   reconnect via /mcp. Claude Desktop doesn't read .mcp.json; add the server"
    echo "==   to its config (Settings > Developer > Edit Config):"
    echo "==     {\"mcpServers\": {\"newbound\": {\"command\": \"/bin/sh\","
    echo "==       \"args\": [\"-c\", \"cd $NB && exec ./target/release/newbound mcp\"]}}}"
  fi
fi
