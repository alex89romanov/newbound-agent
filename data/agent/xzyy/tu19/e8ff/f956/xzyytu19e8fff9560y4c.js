// agent — the Agent app's boot (docs/agent-app.md). Runs under the STOCK
// mount, so this is CLASSIC-script code: no import/export. There is no
// module world to assemble — agent.chat's own html declares its library
// controls (agentloop/agentprompt/memory/viewctx/tokens) as hidden child
// divs, the platform's nested-composition idiom. The boot just mounts it.
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
    host.innerHTML = "";
    var el = document.createElement("div");
    el.style.height = "100%";
    host.appendChild(el);
    await new Promise(function(res) { installControl(el, "agent", "chat", res); });
  } catch (e) {
    fail(e && e.message ? e.message : String(e));
  }
})();
