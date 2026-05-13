class MeiDocMarkdown extends HTMLElement {
  connectedCallback() {
    const props = parseProps(this);
    const content = props.content || props.resource?.document || `无法渲染文档：缺少 content。`;
    this.attachShadow({ mode: "open" });
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        article { display: grid; gap: 12px; padding: 16px; border-radius: 14px; background: rgba(15,23,42,.72); border: 1px solid rgba(148,163,184,.18); color: #e2e8f0; }
        h1, h2, h3 { margin: 0; color: #f8fafc; }
        p { margin: 0; line-height: 1.65; color: #cbd5e1; }
        code { padding: 2px 6px; border-radius: 6px; background: rgba(30,41,59,.92); color: #bfdbfe; }
      </style>
      <article>${renderMarkdown(content)}</article>
    `;
  }
}

function renderMarkdown(content) {
  return content
    .split(/\n{2,}/)
    .map((block) => {
      const trimmed = block.trim();
      if (trimmed.startsWith("### ")) return `<h3>${escapeHtml(trimmed.slice(4))}</h3>`;
      if (trimmed.startsWith("## ")) return `<h2>${escapeHtml(trimmed.slice(3))}</h2>`;
      if (trimmed.startsWith("# ")) return `<h1>${escapeHtml(trimmed.slice(2))}</h1>`;
      return `<p>${escapeHtml(trimmed).replace(/\n/g, "<br/>")}</p>`;
    })
    .join("");
}

function parseProps(element) {
  try {
    return JSON.parse(element.dataset.props || "{}");
  } catch {
    return {};
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

customElements.define("mei-doc-markdown", MeiDocMarkdown);
