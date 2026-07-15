(function initSourceTreeControls() {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  function mountSourceTreeControls() {
    if (typeof boot.disposeSourceTreeControls === "function") {
      try {
        boot.disposeSourceTreeControls();
      } catch (_) {}
      boot.disposeSourceTreeControls = null;
    }
    const sidebar = document.querySelector(".sidebar.left");
    if (!sidebar) return;

  const setBranchOpenRecursively = (rootDetails, nextOpen) => {
    rootDetails.open = nextOpen;
    const descendants = rootDetails.querySelectorAll(".tree-li-branch > details");
    descendants.forEach((child) => {
      child.open = nextOpen;
    });
  };

  const onDblClick = (event) => {
    if (!(event.target instanceof Element)) return;
    const summary = event.target.closest(".tree-folder-summary, .upload-folder-summary-row");
    if (!summary) return;
    const details = summary.closest("details");
    if (!details) return;
    const nextOpen = !details.open;
    setBranchOpenRecursively(details, nextOpen);
  };

  sidebar.addEventListener("dblclick", onDblClick);

  boot.disposeSourceTreeControls = function () {
    sidebar.removeEventListener("dblclick", onDblClick);
    boot.disposeSourceTreeControls = null;
  };
  }

  boot.mountSourceTreeControls = mountSourceTreeControls;
  mountSourceTreeControls();
})();
