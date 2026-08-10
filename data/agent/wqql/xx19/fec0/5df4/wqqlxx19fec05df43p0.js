// bench — the agent add-on's ONE entry point. A single registry line
// (runtime/dev/plugins.json: target dev.dev, selector "" — the boot
// parks headless payloads in its hidden holder) installs this control
// at boot, and it grafts everything else itself.
//
// The knowledge flows ONE way (the owner's rule, 2026-08-10): the
// plugin is allowed to know things about what it's grafting onto —
// never the other way around. So this control knows the bench: the
// notebook drawer carries a .ss-plugins slot and the workbench a
// .wb-plugins slot, both mounted lazily and remounted fresh. It checks
// both spots on a heartbeat and fills any unclaimed one; the marker it
// stamps is the same data-nb-plugin attribute the loader's registry
// pass uses, so the mechanisms coexist without double-mounting.
var me = this;

me.ready = function () {
  if (globalThis.__nbAgentBench) return;
  globalThis.__nbAgentBench = true;
  if (typeof installControl !== "function") return;
  var check = function () {
    var s = document.querySelector(".ss-plugins:not([data-nb-plugin])");
    if (s) {
      s.dataset.nbPlugin = "agent-askrow";
      installControl(s, "agent", "askrow", null);
    }
    var w = document.querySelector(".wb-plugins:not([data-nb-plugin])");
    if (w) {
      w.dataset.nbPlugin = "agent-describebtn";
      installControl(w, "agent", "describebtn", null);
    }
  };
  check();
  setInterval(check, 250);
};
