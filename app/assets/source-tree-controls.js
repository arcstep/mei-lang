(function initSourceTreeControls() {
  const sidebar = document.querySelector(".sidebar.left");
  if (!sidebar) return;

  const setBranchOpenRecursively = (rootDetails, nextOpen) => {
    rootDetails.open = nextOpen;
    const descendants = rootDetails.querySelectorAll(".tree-li-branch > details");
    descendants.forEach((child) => {
      child.open = nextOpen;
    });
  };

  sidebar.addEventListener("dblclick", (event) => {
    if (!(event.target instanceof Element)) return;
    const summary = event.target.closest(".tree-folder-summary");
    if (!summary) return;
    const details = summary.closest("details");
    if (!details) return;
    const nextOpen = !details.open;
    setBranchOpenRecursively(details, nextOpen);
  });
})();
