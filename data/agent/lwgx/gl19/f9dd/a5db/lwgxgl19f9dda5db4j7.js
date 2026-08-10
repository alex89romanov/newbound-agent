// agentprompt.js — the OWNER'S system-prompt addendum. Whatever ADDENDUM
// contains is appended to the agent's system prompt on every ask (under an
// "OWNER ADDENDUM" heading), so prompt experiments happen HERE — in the
// bench, edit the `agentprompt` module control's js facet; every change is
// journaled and takes effect on the next ask, no reinstall. When something
// proves out, promote it into agentloop.js's base prompt.
//
// Keep it a plain template literal. Empty string = no addendum.
//
// LIBRARY control — headless: defines window.NB_AGENTPROMPT once (idempotent across
// installs). Consumers list this control as a hidden data-control child
// div and use the global from their ready.

var me = this;
var ME = document.getElementById(me.UUID);

me.ready = function () {
  if (window.NB_AGENTPROMPT) return;
  window.NB_AGENTPROMPT = (function () {

const ADDENDUM = ``;

    return { ADDENDUM };
  })();
};
