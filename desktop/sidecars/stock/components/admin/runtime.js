function parseProps(element) {
  const raw = element.getAttribute("data-props");
  if (!raw) return {};
  try {
    return JSON.parse(raw);
  } catch (_error) {
    return {};
  }
}

function element(tag, options = {}) {
  const node = document.createElement(tag);
  if (options.className) node.className = options.className;
  if (options.text != null) node.textContent = String(options.text);
  if (options.type) node.type = options.type;
  return node;
}

class AdminBrick extends HTMLElement {
  connectedCallback() {
    this.render(parseProps(this));
  }

  reset(title) {
    const root = element("section", { className: "mei-admin-brick" });
    if (title) root.append(element("h2", { text: title }));
    this.replaceChildren(root);
    return root;
  }
}

class FormCard extends AdminBrick {
  render(props) {
    const root = this.reset(props.title || "Form");
    const form = element("form", { className: "mei-admin-form-card" });
    const payload = props.payload && typeof props.payload === "object" ? props.payload : {};
    Object.entries(payload).forEach(([name, value]) => {
      const label = element("label", { className: "mei-admin-field" });
      label.append(element("span", { text: name }));
      const input = element("input");
      input.name = name;
      input.value = value == null ? "" : String(value);
      label.append(input);
      form.append(label);
    });
    const save = element("button", { text: props.submit_label || "保存", type: "submit" });
    form.append(save);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const detail = Object.fromEntries(new FormData(form).entries());
      this.dispatchEvent(
        new CustomEvent("mei:admin-submit", { bubbles: true, composed: true, detail }),
      );
    });
    root.append(form);
  }
}

class DataGrid extends AdminBrick {
  render(props) {
    const root = this.reset(props.title || "Data");
    const rows = Array.isArray(props.rows) ? props.rows : [];
    const columns = Array.isArray(props.columns)
      ? props.columns
      : [...new Set(rows.flatMap((row) => Object.keys(row || {})))];
    const table = element("table", { className: "mei-admin-data-grid" });
    const head = element("thead");
    const headRow = element("tr");
    columns.forEach((column) => headRow.append(element("th", { text: column })));
    head.append(headRow);
    table.append(head);
    const body = element("tbody");
    rows.forEach((row) => {
      const tr = element("tr");
      columns.forEach((column) => tr.append(element("td", { text: row?.[column] ?? "" })));
      body.append(tr);
    });
    table.append(body);
    root.append(table);
  }
}

class AssetSlot extends AdminBrick {
  render(props) {
    const root = this.reset(props.title || props.slot_id || "Asset");
    root.append(element("p", { text: props.status || "missing" }));
    const input = element("input");
    input.type = "file";
    input.addEventListener("change", () => {
      const file = input.files?.[0];
      if (file) {
        this.dispatchEvent(
          new CustomEvent("mei:admin-asset-selected", {
            bubbles: true,
            composed: true,
            detail: { slotId: props.slot_id, file },
          }),
        );
      }
    });
    root.append(input);
  }
}

class ActionStrip extends AdminBrick {
  render(props) {
    const root = this.reset();
    root.classList.add("mei-admin-action-strip");
    (Array.isArray(props.actions) ? props.actions : []).forEach((action) => {
      const button = element("button", { text: action.label || action.id, type: "button" });
      button.dataset.action = String(action.id || "");
      button.addEventListener("click", () => {
        this.dispatchEvent(
          new CustomEvent("mei:admin-action", {
            bubbles: true,
            composed: true,
            detail: { action: action.id },
          }),
        );
      });
      root.append(button);
    });
  }
}

class JobStatus extends AdminBrick {
  render(props) {
    const root = this.reset(props.title || "Job");
    root.dataset.status = String(props.status || "unknown");
    root.append(element("strong", { text: props.status || "unknown" }));
    if (props.message) root.append(element("p", { text: props.message }));
  }
}

class Navigator extends AdminBrick {
  render(props) {
    const root = this.reset();
    const nav = element("nav");
    (Array.isArray(props.items) ? props.items : []).forEach((item) => {
      const link = element("a", { text: item.label || item.id });
      link.href = String(item.href || "#");
      if (item.active) link.setAttribute("aria-current", "page");
      nav.append(link);
    });
    root.append(nav);
  }
}

[
  ["mei-admin-form-card", FormCard],
  ["mei-admin-data-grid", DataGrid],
  ["mei-admin-asset-slot", AssetSlot],
  ["mei-admin-action-strip", ActionStrip],
  ["mei-admin-job-status", JobStatus],
  ["mei-admin-navigator", Navigator],
].forEach(([tag, constructor]) => {
  if (!customElements.get(tag)) customElements.define(tag, constructor);
});

