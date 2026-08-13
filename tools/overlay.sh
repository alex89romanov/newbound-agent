#!/bin/sh
# Overlay this agent repo onto a newbound checkout via symlinks.
#
# Run from the newbound checkout root:
#   path/to/newbound-agent/tools/overlay.sh path/to/newbound-agent
#
# Idempotent: existing correct links are left alone; a real directory in
# the way is an error (never deleted).

set -e

AGENT_DIR="$1"
if [ -z "$AGENT_DIR" ]; then
  echo "usage: overlay.sh <path-to-newbound-agent>" >&2
  exit 1
fi
AGENT_DIR=$(cd "$AGENT_DIR" && pwd)

if [ ! -d "data" ] || [ ! -f "Cargo.toml" ]; then
  echo "error: run from a newbound checkout root (needs data/ and Cargo.toml)" >&2
  exit 1
fi

link() { # link <checkout-relative-path> <agent-repo-relative-path>
  src="$AGENT_DIR/$2"
  dst="$1"
  if [ -L "$dst" ]; then
    ln -sfn "$src" "$dst"
  elif [ -e "$dst" ]; then
    echo "error: $dst exists and is not a symlink — refusing to touch it" >&2
    exit 1
  else
    ln -s "$src" "$dst"
  fi
  echo "  $dst -> $src"
}

echo "overlaying $AGENT_DIR:"
link data/agent   data/agent
link data/kb      data/kb
link data/scratch data/scratch
link agent        agent
link kb           kb
link scratch      scratch

# Silence runtime mutations of the tracked scratch skeleton (per-clone).
(cd "$AGENT_DIR" && git ls-files -z data/scratch scratch \
  | xargs -0 -r git update-index --skip-worktree \
  && echo "  skip-worktree set on the scratch skeleton")

# The platform checkout's generated files absorb overlay knowledge on
# every rebuild (api.rs grows agent/kb/... modules; the initializer
# learns the FFI crates). They are tracked upstream but their overlay
# state must never ride a commit - hide the churn per-clone, the same
# way the scratch skeleton is handled.
git update-index --skip-worktree newbound_core/src/api.rs src/generated_initializer.rs 2>/dev/null \
  && echo "  skip-worktree set on newbound_core/src/api.rs and src/generated_initializer.rs"

# Same story for the workspace manifest: its `exclude = [...]` line names
# the overlay and FFI crates, `newbound rebuild` writes it when absent,
# and the platform repo's manifest must never reference anything outside
# newbound_core. Nothing there is worth preserving across a pull.
git update-index --skip-worktree Cargo.toml 2>/dev/null \
  && echo "  skip-worktree set on Cargo.toml"

# Untracked residue can't be hidden with skip-worktree, and .gitignore is
# the platform repo's tracked file - not ours to grow for overlay-local
# convenience. Per-clone excludes instead, so `git status` stays honest
# and a dirty tree keeps meaning "you have work to commit".
GITDIR=$(git rev-parse --git-dir)
EXCLUDE="$GITDIR/info/exclude"
mkdir -p "$GITDIR/info"
exclude() { # exclude <pattern>
  grep -qxF "$1" "$EXCLUDE" 2>/dev/null && return 0
  echo "$1" >> "$EXCLUDE"
  echo "  excluded $1 (per-clone)"
}
exclude /server.pid
# dev.github.import clones an FFI library to repositories/<lib> and links
# its crate at the checkout root. .gitignore covers the store and runtime
# halves of that import (/data/<lib>, /runtime/<lib>) but not the link.
for entry in *; do
  [ -L "$entry" ] || continue
  case "$(readlink "$entry")" in
    */repositories/*) exclude "/$entry" ;;
  esac
done

echo "done. Build the host, then the agent/kb dylibs (see README)."
