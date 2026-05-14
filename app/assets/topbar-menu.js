(() => {
  const groups = Array.from(
    document.querySelectorAll("details.app-group[data-topbar-menu-group]"),
  );
  if (!groups.length) return;

  const closeGroup = (group) => {
    if (!group) return;
    group.removeAttribute("open");
  };

  const closeAll = (except) => {
    for (const group of groups) {
      if (group !== except) closeGroup(group);
    }
  };

  for (const group of groups) {
    const summary = group.querySelector("summary.app-group-summary");
    if (!summary) continue;

    summary.addEventListener("click", (event) => {
      event.preventDefault();
      const willOpen = !group.hasAttribute("open");
      closeAll(group);
      if (willOpen) {
        group.setAttribute("open", "");
      } else {
        closeGroup(group);
      }
    });

    group.addEventListener("mouseleave", () => {
      closeGroup(group);
    });

    group.addEventListener("focusout", () => {
      window.setTimeout(() => {
        const active = document.activeElement;
        if (!active || !group.contains(active)) {
          closeGroup(group);
        }
      }, 0);
    });
  }

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (!target.closest(".app-group")) {
      closeAll();
    }
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeAll();
    }
  });
})();
