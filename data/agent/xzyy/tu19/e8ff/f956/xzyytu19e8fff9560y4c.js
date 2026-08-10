// agent — the Agent app's boot (docs/agent-app.md). Runs under the STOCK
// mount, so this is CLASSIC-script code: no import/export. It builds the
// module world the chat control needs — app.modules (store/loader/nb/
// tokens...), dev.chatctx (the context registry), and the agent's own
// modules (agentloop/agentprompt/memory, installed directly — the old
// agentmodules cluster retired with the bench-plugin rework) — then hands
// the loader the union control directory and mounts agent.chat. Same
// pattern as the Development boot, minus vendors and plugins.
(async function() {
  var host = document.querySelector(".ag-boot") || document.body;
  function fail(msg) {
    host.innerHTML = "";
    var p = document.createElement("p");
    p.className = "ag-boot-err";
    p.textContent = "the agent app failed to boot: " + msg;
    host.appendChild(p);
  }
  try {
    // declare the environment BEFORE module installs; filled in place below
    var bench = globalThis.__benchPlatform = {
      lib: "agent", moduleUrls: {}, controls: {}, assetRoots: {} };
    var install = function(l, n, el) {
      return new Promise(function(res) { installControl(el || null, l, n, res); });
    };
    var holder = document.createElement("div");
    holder.hidden = true;
    host.appendChild(holder);
    var clusterEl = function() {
      var d = document.createElement("div");
      holder.appendChild(d);
      return d;
    };
    await Promise.all([
      install("app", "modules", clusterEl()),
      install("dev", "chatctx", clusterEl()),
      install("agent", "agentloop", clusterEl()),
      install("agent", "agentprompt", clusterEl()),
      install("agent", "memory", clusterEl()),
    ]);
    var read = async function(l, id) {
      var r = await fetch("/app/read?lib=" + encodeURIComponent(l) +
                          "&id=" + encodeURIComponent(id));
      var j = await r.json();
      if (j.status !== "ok") throw new Error(j.msg || "app/read failed");
      return j;
    };
    var libs = ["agent", "app", "dev"];
    for (var i = 0; i < libs.length; i++) {
      try {
        var idx = await read(libs[i], "controls");
        var list = idx.data.list || [];
        for (var k = 0; k < list.length; k++) {
          if (!bench.controls[list[k].name]) {
            bench.controls[list[k].name] = { lib: libs[i], id: list[k].id };
          }
        }
      } catch (e) { /* an absent library is fine */ }
    }
    for (var n in NB_MODULES) {
      if (NB_MODULES[n].url) bench.moduleUrls["assets/" + n + ".js"] = NB_MODULES[n].url;
    }
    var loader = await requireModule("loader");
    host.innerHTML = "";
    await loader.mountControl("chat", host);
  } catch (e) {
    fail(e && e.message ? e.message : String(e));
  }
})();
