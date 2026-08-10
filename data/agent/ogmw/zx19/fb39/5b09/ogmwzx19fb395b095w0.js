// agentmodules js — the graft arm of the one-entry plugin. The session
// mounts its notebook lazily, so the chat surface can't be a static
// child here: this script watches for the notebook's plugin slot
// (.ss-plugins) and installs agent.askrow into it whenever an unclaimed
// one appears — including remounts. The marker it sets is the same one
// the loader's registry pass uses, so the two mechanisms can coexist
// without double-mounting on an instance that carries extra entries.
(function () {
  if (document.__nbAgentAskGraft) return;
  document.__nbAgentAskGraft = true;
  if (typeof installControl !== "function") return;
  var graft = function () {
    var slot = document.querySelector(".ss-plugins:not([data-nb-plugin])");
    if (!slot) return;
    slot.dataset.nbPlugin = "agent-askrow";
    installControl(slot, "agent", "askrow", null);
  };
  graft();
  new MutationObserver(graft).observe(document.body, { childList: true, subtree: true });
})();