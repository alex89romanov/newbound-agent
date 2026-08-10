// describebtn — the agent add-on's desc drafter, grafted into the
// workbench by the registry (dev.plugins: target dev.workbench, selector
// .wb-plugins). Classic and lineal: module dependencies are child divs
// (activated before me.ready; registration is idempotent), and the
// listener sits on THIS control's host — the mount slot's parent, the
// workbench root — so nothing outside this workbench instance is
// touched, and a remount gets a fresh listener with the fresh DOM. The
// workbench announces each command-meta panel with a bubbling
// nb-command-meta event; this control injects the generate button and
// drives agent.plugin.describe_command.
var me = this;
var ME = $('#' + me.UUID)[0];

me.ready = async function () {
  var host = ME.parentElement;
  if (!host) return;
  const loop = window.NB_AGENTLOOP;
  const invokeP = (l2, c2, m2, a2) => new Promise((res2) => invokeCommand(l2, c2, m2, a2, res2));
  const readCommand = async (l2, c2, m2) => {
    const r2 = await invokeP("dev", "code", "read_command", { lib: l2, ctl: c2, cmd: m2 });
    if (r2.status !== "ok") return r2;
    return { status: "ok", ...(r2.data && typeof r2.data === "object" ? r2.data : {}) };
  };
  host.addEventListener("nb-command-meta", (ev) => {
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
      const rc = await readCommand(lib, ctl, cmd);
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
};
