(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.statusBarMounted) return;
  boot.statusBarMounted = true;

  function readMeta(name) {
    const node = document.querySelector('meta[name="' + name + '"]');
    return node ? String(node.getAttribute("content") || "").trim() : "";
  }

  function els() {
    return {
      compliance: document.getElementById("mei-status-compliance"),
      hostVersion: document.getElementById("mei-status-host-version"),
    };
  }

  function setChip(node, text, tone, title) {
    if (!node) return;
    node.textContent = text;
    node.title = title || text;
    node.dataset.tone = tone || "neutral";
  }

  function applyComplianceChip() {
    const nodes = els();
    if (!nodes.compliance) return;
    const parts = [
      readMeta("mei-host-icp-record"),
      readMeta("mei-host-psb-record"),
      readMeta("mei-host-copyright"),
    ].filter(Boolean);
    if (!parts.length) {
      nodes.compliance.hidden = true;
      nodes.compliance.textContent = "";
      return;
    }
    nodes.compliance.hidden = false;
    const text = parts.join(" · ");
    setChip(nodes.compliance, text, "neutral", text);
  }

  function applyHostVersionChip() {
    const nodes = els();
    const label = readMeta("mei-host-version-label");
    const version = readMeta("mei-host-version");
    const text = label || (version ? "Mei " + version : "");
    if (!text) return;
    setChip(nodes.hostVersion, text, "neutral", text);
  }

  function start() {
    applyComplianceChip();
    applyHostVersionChip();
  }

  boot.refreshStatusBarChips = function refreshStatusBarChips() {
    applyComplianceChip();
    applyHostVersionChip();
  };

  boot.disposeStatusBar = function () {
    boot.statusBarMounted = false;
  };
  start();
})();
