(function () {
  if (!window.CodeMirror || typeof window.CodeMirror.defineMode !== "function") {
    return;
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
  const PUNCT = /[()[\]{},.:]/;

  function isIdentifierStart(ch) {
    return /[A-Za-z_]/.test(ch || "");
  }

  function isIdentifierPart(ch) {
    return /[A-Za-z0-9_]/.test(ch || "");
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

  function classifyIdentifier(source, start, end, word) {
    const nextIndex = nextNonWhitespaceIndex(source, end);
    const prevIndex = prevNonWhitespaceIndex(source, start - 1);
    const nextChar = nextIndex < source.length ? source[nextIndex] : "";
    const prevChar = prevIndex >= 0 ? source[prevIndex] : "";

    if (DECLARATIONS.has(word) && nextChar === "(") return "mei-decl";
    if (KEYWORDS.has(word)) return "mei-keyword";
    if (LITERALS.has(word)) return "mei-literal";
    if (prevChar === ".") return "mei-member";
    if (nextChar === ".") return "mei-namespace";
    if (nextChar === "(") return "mei-call";
    if (nextChar === "=") return "mei-attr";
    return null;
  }

  function consumeString(stream, state) {
    let escaped = false;
    while (!stream.eol()) {
      const ch = stream.next();
      if (state.triple) {
        if (
          ch === state.quote &&
          stream.peek() === state.quote &&
          stream.string.charAt(stream.pos) === state.quote
        ) {
          stream.next();
          stream.next();
          state.inString = false;
          state.triple = false;
          break;
        }
        continue;
      }
      if (!escaped && ch === state.quote) {
        state.inString = false;
        break;
      }
      escaped = !escaped && ch === "\\";
    }
    return "mei-string";
  }

  window.CodeMirror.defineMode("mei", function () {
    return {
      startState: function () {
        return {
          inString: false,
          quote: "",
          triple: false,
        };
      },
      token: function (stream, state) {
        if (state.inString) {
          return consumeString(stream, state);
        }

        if (stream.sol()) {
          state.line = stream.string;
        }

        if (stream.eatSpace()) return null;

        const ch = stream.peek();

        if (ch === "#") {
          stream.skipToEnd();
          return "mei-comment";
        }

        if (ch === '"' || ch === "'") {
          state.quote = ch;
          state.triple =
            stream.string.slice(stream.pos, stream.pos + 3) === ch.repeat(3);
          state.inString = true;
          if (state.triple) {
            stream.next();
            stream.next();
            stream.next();
          } else {
            stream.next();
          }
          return consumeString(stream, state);
        }

        if (/[0-9]/.test(ch)) {
          stream.eatWhile(/[0-9_]/);
          if (stream.peek() === "." && /[0-9]/.test(stream.string.charAt(stream.pos + 1))) {
            stream.next();
            stream.eatWhile(/[0-9_]/);
          }
          return "mei-number";
        }

        if (isIdentifierStart(ch)) {
          const start = stream.pos;
          stream.next();
          stream.eatWhile(isIdentifierPart);
          const end = stream.pos;
          const word = stream.string.slice(start, end);
          return classifyIdentifier(stream.string, start, end, word);
        }

        if (PUNCT.test(ch)) {
          stream.next();
          return "mei-punct";
        }

        if ("=+-*/%<>!".includes(ch)) {
          stream.next();
          if ("=<>*/".includes(stream.peek() || "")) {
            const pair = ch + stream.peek();
            if (pair === "==" || pair === "!=" || pair === "<=" || pair === ">=" || pair === "//" || pair === "**") {
              stream.next();
            }
          }
          return "mei-operator";
        }

        stream.next();
        return null;
      },
    };
  });
})();
