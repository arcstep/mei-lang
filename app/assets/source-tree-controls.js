(function initSourceTreeControls() {
  const expandBtn = document.querySelector("[data-tree-expand]");
  const collapseBtn = document.querySelector("[data-tree-collapse]");
  const sidebar = document.querySelector(".sidebar.left");
  if (!sidebar) return;
  const detailsList = () => Array.from(sidebar.querySelectorAll(".tree-li-branch > details"));

  if (expandBtn) {
    expandBtn.addEventListener("click", function () {
      detailsList().forEach((el) => { el.open = true; });
    });
  }

  if (collapseBtn) {
    collapseBtn.addEventListener("click", function () {
      detailsList().forEach((el) => { el.open = false; });
    });
  }
})();
