(function () {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (typeof boot.disposeSourceHighlight === "function") {
    try {
      boot.disposeSourceHighlight();
    } catch (_) {}
    boot.disposeSourceHighlight = null;
  }
  const DECLARATIONS = new Set([
    "app",
    "entry",
    "scene",
    "world",
    "flow",
    "frame",
    "panel",
    "component",
    "resource",
    "entity",
    "topology",
    "theme",
    "layout",
    "intent",
    "start",
    "click",
    "timer",
    "tick",
    "rule",
    "cell",
    "subject",
    "outcome",
  ]);

  const KEYWORDS = new Set([
    "def",
    "return",
    "for",
    "in",
    "if",
    "elif",
    "else",
    "and",
    "or",
    "not",
    "lambda",
    "pass",
    "break",
    "continue",
    "load",
  ]);

  const LITERALS = new Set(["True", "False", "None"]);
  const PUNCT = new Set(["(", ")", "[", "]", "{", "}", ",", ".", ":"]);

  function escapeHtml(value) {
    return String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;");
  }

  function wrapToken(type, value) {
    if (!value) return "";
    const text = escapeHtml(value);
    return type ? '<span class="' + type + '">' + text + "</span>" : text;
  }

  function isIdentifierStart(ch) {
    return /[A-Za-z_]/.test(ch || "");
  }

  function isIdentifierPart(ch) {
    return /[A-Za-z0-9_]/.test(ch || "");
  }

  function isDigit(ch) {
    return /[0-9]/.test(ch || "");
  }

  function nextNonWhitespaceIndex(source, start) {
    let index = start;
    while (index < source.length && /\s/.test(source[index])) {
      index += 1;
    }
    return index;
  }

  function prevNonWhitespaceIndex(source, start) {
    let index = start;
    while (index >= 0 && /\s/.test(source[index])) {
      index -= 1;
    }
    return index;
  }

  function readString(source, start) {
    const quote = source[start];
    const triple = source.slice(start, start + 3) === quote.repeat(3);
    let index = start + (triple ? 3 : 1);

    while (index < source.length) {
      if (triple) {
        if (source.slice(index, index + 3) === quote.repeat(3)) {
          index += 3;
          break;
        }
        index += 1;
        continue;
      }

      if (source[index] === "\\") {
        index += 2;
        continue;
      }
      if (source[index] === quote) {
        index += 1;
        break;
      }
      index += 1;
    }

    return {
      end: index,
      value: source.slice(start, index),
    };
  }

  function readNumber(source, start) {
    let index = start;
    while (index < source.length && /[0-9_]/.test(source[index])) {
      index += 1;
    }
    if (source[index] === "." && isDigit(source[index + 1])) {
      index += 1;
      while (index < source.length && /[0-9_]/.test(source[index])) {
        index += 1;
      }
    }
    return {
      end: index,
      value: source.slice(start, index),
    };
  }

  function readOperator(source, start) {
    const pair = source.slice(start, start + 2);
    if (
      pair === "==" ||
      pair === "!=" ||
      pair === "<=" ||
      pair === ">=" ||
      pair === "//" ||
      pair === "**"
    ) {
      return { end: start + 2, value: pair };
    }
    return { end: start + 1, value: source[start] };
  }

  function classifyIdentifier(source, start, end, word) {
    const nextIndex = nextNonWhitespaceIndex(source, end);
    const prevIndex = prevNonWhitespaceIndex(source, start - 1);
    const nextChar = nextIndex < source.length ? source[nextIndex] : "";
    const prevChar = prevIndex >= 0 ? source[prevIndex] : "";

    if (DECLARATIONS.has(word) && nextChar === "(") return "token-decl";
    if (KEYWORDS.has(word)) return "token-keyword";
    if (LITERALS.has(word)) return "token-literal";
    if (prevChar === ".") return "token-member";
    if (nextChar === ".") return "token-namespace";
    if (nextChar === "(") return "token-call";
    if (nextChar === "=") return "token-attr";
    return "";
  }

  function highlightMei(source) {
    let index = 0;
    let html = "";

    while (index < source.length) {
      const ch = source[index];

      if (ch === "#" ) {
        let end = index;
        while (end < source.length && source[end] !== "\n") {
          end += 1;
        }
        html += wrapToken("token-comment", source.slice(index, end));
        index = end;
        continue;
      }

      if (ch === '"' || ch === "'") {
        const token = readString(source, index);
        html += wrapToken("token-string", token.value);
        index = token.end;
        continue;
      }

      if (isDigit(ch)) {
        const token = readNumber(source, index);
        html += wrapToken("token-number", token.value);
        index = token.end;
        continue;
      }

      if (isIdentifierStart(ch)) {
        let end = index + 1;
        while (end < source.length && isIdentifierPart(source[end])) {
          end += 1;
        }
        const word = source.slice(index, end);
        const type = classifyIdentifier(source, index, end, word);
        html += wrapToken(type, word);
        index = end;
        continue;
      }

      if ("=+-*/%<>!".includes(ch)) {
        const token = readOperator(source, index);
        html += wrapToken("token-operator", token.value);
        index = token.end;
        continue;
      }

      if (PUNCT.has(ch)) {
        html += wrapToken("token-punct", ch);
        index += 1;
        continue;
      }

      html += wrapToken("", ch);
      index += 1;
    }

    return html;
  }

  function highlightElement(element) {
    const lang = String(element.dataset.sourceLang || "plain").trim().toLowerCase();
    const source = element.textContent || "";
    if (lang !== "mei") {
      element.textContent = source;
      return;
    }
    element.innerHTML = highlightMei(source);
  }

  function init() {
    document
      .querySelectorAll('[data-source-viewer="1"]')
      .forEach(function (element) {
        highlightElement(element);
      });
  }

  const rerun = function () {
    init();
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", rerun, { once: true });
  } else {
    rerun();
  }
  document.addEventListener("mei:manage-context-change", rerun);

  boot.disposeSourceHighlight = function () {
    document.removeEventListener("mei:manage-context-change", rerun);
  };
})();
