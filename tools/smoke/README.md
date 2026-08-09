# Browser smokes

The half of the verification harness that needs a browser. The other half —
`tools/flow3d-check.mjs` and `tools/scene-check.mjs` — is pure and runs in
node with no server at all; run those first, they are faster and catch more.

```bash
node tools/smoke/run.mjs                                   # no instance needed
node tools/smoke/run.mjs --base http://localhost:33199 \
                         --dir /path/to/instance           # + real-instance passes
node tools/smoke/run.mjs flow-editor --base ... --dir ...  # just one
```

Exit code is non-zero if anything fails, so this is CI-shaped.

| smoke | needs | covers |
|---|---|---|
| `stage-pools` | nothing | the shared nb_three batching core: the §7.2 draw-call budget (framed, which is where op bodies used to cost two calls each) and that pooling stays invisible to callers — pick, move, resize, emphasis, hide, remove |
| `scene-mock` | nothing | the scene stack in mock mode: `each` collections, links, taps carrying instance locals, keyed diffing, and the editor's tolerance of collection docs |
| `flow-editor` | a disposable instance | the 3D flow editor end to end: the deep loop (open → edit → ⌘S → journal → run ▸ → execute), untangle as one undoable mutation, keyboard wiring, and the journal diff view |
| `boot-player` | a disposable instance | the published bench's boot chain (stock-mount stub → MODMAP → module graph → frame → shelf, all from facets), the `#/player` route and its postMessage bridge, and `installControl(el, 'bench', 'player', …)` mounting **inside another app's page** — the peer app's own path |
| `wiring` | a disposable instance | the workbench timers/events panel over the replace-only setters: set, replace-by-name (Q7), the store record, the `timer` journal facet, and two-click removal |

## Why these were rewritten rather than copied

They lived in a session scratchpad and died with it. Three things had to
change before they could live here:

- **Record ids are per-instance.** The scratchpad copies hardcoded command
  and control ids, so they only ever worked on the machine that produced
  them. Everything now resolves by NAME (`harness.agentApi`), and
  `flow-editor` builds its own fixture library through the API rather than
  assuming one exists.
- **Sessions expire** (15 minutes) and a disposable instance restarts often,
  so each run logs in for itself from `<dir>/users/admin.properties`.
- **Paths were absolute.** Playwright and chromium are resolved from flags,
  the environment, or common locations, with a readable error when missing.

## Notes for writing more

- `t.page(...)` collects console and page errors as failures. It seeds a
  live connection only when a session is passed — a pure-stage smoke loads
  `index.html` off a static server and imports a module directly, and
  seeding live mode there makes the bench boot and 404.
- The editor's keybinds are host-scoped: after a popover commit re-renders
  the button that held focus, focus must go back to the pane before `⌘S`.
  A user clicks the canvas naturally; automation has to do it deliberately
  (`focusPane` in `flow-editor.mjs`).
- The stage exposes no world→screen helper, so smokes that need to click a
  specific object put it at the origin under a head-on camera and click the
  canvas centre.
- Mutating smokes target a `smoketest` library. They are safe to re-run:
  fixtures are rewritten unguarded each time.

## The simulator is retired

`boot-player` and `wiring` replace four sim-backed smokes
(`smoke_stockplayer`, `smoke_player`, `smoke_wiring`, `smoke_boot`) that
needed a ~650-line fake platform. Running them against a real instance
deleted that dependency and made them strictly stronger — a simulator
answers however it was written to answer, so it can only ever confirm the
author's assumptions.

Two things a simulator could not have caught, both found during this port:

- **The `player` control had never been committed to the store.** Every
  simulated run passed because the sim served it from repo files. On a real
  instance, `installControl(el, 'bench', 'player', …)` from the peer app is
  the exact call that would have failed on a clean deploy. `boot-player`
  now covers it.
- **A missing declared param aborts the whole server.** Calling
  `remove_timer` without `author` panics through the FFI boundary and takes
  the instance down — not just the request. The bench's own `store.js`
  wrappers always fill `author`, which is why the UI never hits it. Pass
  every declared param from a smoke.
