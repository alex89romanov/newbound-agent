// describebtn — the agent add-on's desc drafter, grafted onto the
// workbench's command-meta panel. The workbench knows nothing about this
// control: it renders a .cm-ext slot in each panel and announces it with
// a bubbling nb-command-meta event; this document-level listener injects
// the generate button and drives agent.plugin.describe_command. Mounted
// (headless) through the plugin registry; the listener registers once
// per page across workbench remounts.
(async () => {
  if (document.__nbAgentDescribe) return;
  document.__nbAgentDescribe = true;
  // requireModule, not moduleUrls: as a cluster child this installs at
  // boot, BEFORE the boot publishes moduleUrls — the registry resolves
  // whenever the modules land, order-free.
  if (typeof requireModule !== "function") return;
  const [{ store }, loop] = await Promise.all([
    requireModule("store", "describebtn"),
    requireModule("agentloop", "describebtn"),
  ]);
  document.addEventListener("nb-command-meta", (ev) => {
    const { lib, ctl, cmd, groups, descInput, note } = ev.detail ?? {};
    const slotEl = ev.target;
    if (!lib || !slotEl || slotEl.querySelector(".cm-gen")) return;
    const gen = document.createElement("button");
    gen.className = "cm-gen";
    gen.title = "Draft a description from the command's code (agent.plugin.describe_command)";
    gen.textContent = "generate ▸";
    gen.addEventListener("click", async () => {
      gen.disabled = true;
      note.textContent = "generating…";
      const rc = await store.readCommand(lib, ctl, cmd);
      if (rc.status !== "ok") {
        note.textContent = `could not read the command: ${rc.msg}`;
        gen.disabled = false;
        return;
      }
      const lang = rc.type ?? "rust";
      const ext = lang === "rust" ? "rs" : lang;
      const r = await loop.describeCommand({
        command_name: cmd,
        lang,
        returntype: rc.returntype ?? "",
        groups: groups ?? "",
        params: rc.params ?? [],
        imports: rc.import ?? "",
        code: typeof rc[ext] === "string" ? rc[ext] : "",
        current_description: descInput.value.trim(),
      });
      gen.disabled = false;
      if (r.status !== "ok") {
        note.textContent = `generate failed: ${r.msg}`;
        return;
      }
      descInput.value = (r.msg ?? "").trim();
      note.textContent = "drafted — review, then set";
    });
    slotEl.appendChild(gen);
  });
})();
