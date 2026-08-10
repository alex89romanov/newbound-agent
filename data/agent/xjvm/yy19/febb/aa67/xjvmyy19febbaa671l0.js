// ask — the agent library's adapter for the bench's ASK-PROVIDER SOCKET.
// dev.session and dev.workbench import the optional module `ask` when the
// page's module registry carries it; they name no providing library. This
// module is that provider: a thin facade over agentloop (the loop, the
// prompts, the tool machinery, and this library's own wire calls) and
// agentprompt (the owner addendum). Installed headless by the agentmodules
// cluster — one plugin-registry entry activates the whole add-on.
export * from "./agentloop.js";
export { ADDENDUM } from "./agentprompt.js";
