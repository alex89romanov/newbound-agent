// run — the browser smoke battery.
//
//   node tools/smoke/run.mjs                          # everything that needs no instance
//   node tools/smoke/run.mjs --base URL --dir DIR     # + the real-instance passes
//   node tools/smoke/run.mjs flow-editor --base ...   # just one
//
// Exit code is non-zero if any smoke reports a problem, so this is CI-shaped.
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));

const SMOKES = [
  { name: "stage-pools", file: "stage-pools.mjs", needsInstance: false },
  { name: "scene-mock", file: "scene-mock.mjs", needsInstance: false },
  { name: "flow-editor", file: "flow-editor.mjs", needsInstance: true },
  { name: "boot-player", file: "boot-player.mjs", needsInstance: true },
  { name: "wiring", file: "wiring.mjs", needsInstance: true },
  { name: "deletion", file: "deletion.mjs", needsInstance: true },
];

const argv = process.argv.slice(2);
const only = argv.filter((a) => !a.startsWith("--") &&
  !argv[argv.indexOf(a) - 1]?.startsWith("--"));
const flags = argv.filter((a, i) => a.startsWith("--") ||
  argv[i - 1]?.startsWith("--"));
const hasInstance = flags.includes("--base");

const chosen = SMOKES.filter((s) => (only.length ? only.includes(s.name) : true));
const runnable = chosen.filter((s) => !s.needsInstance || hasInstance);
const skipped = chosen.filter((s) => s.needsInstance && !hasInstance);

const run = (file) => new Promise((res) => {
  const p = spawn(process.execPath, [resolve(HERE, file), ...flags], { stdio: "inherit" });
  p.on("exit", (code) => res(code === 0));
});

let failed = 0;
for (const s of runnable) {
  if (!(await run(s.file))) failed++;
}
for (const s of skipped) {
  console.log(`\n── ${s.name} ── SKIPPED (needs --base URL --dir DIR: a disposable instance)`);
}
console.log(`\n${runnable.length - failed}/${runnable.length} smokes green` +
  (skipped.length ? `, ${skipped.length} skipped` : ""));
process.exit(failed ? 1 : 0);
