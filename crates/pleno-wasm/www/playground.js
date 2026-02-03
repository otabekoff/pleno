import init, { run_source } from "./pkg/pleno_wasm.js";

const output = document.getElementById("output");
const runBtn = document.getElementById("run-btn");
const clearBtn = document.getElementById("clear-btn");
const status = document.getElementById("status");

let editor;

// Configure Monaco Loader
require.config({
  paths: {
    vs: "https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs",
  },
});

async function setupEditor() {
  return new Promise((resolve) => {
    require(["vs/editor/editor.main"], function () {
      // Define Pleno language
      monaco.languages.register({ id: "pleno" });

      monaco.languages.setMonarchTokensProvider("pleno", {
        keywords: [
          "let",
          "const",
          "mut",
          "fn",
          "return",
          "if",
          "else",
          "for",
          "while",
          "match",
          "class",
          "interface",
          "enum",
          "import",
          "export",
          "async",
          "await",
          "try",
          "catch",
          "finally",
          "throw",
          "abstract",
          "implements",
          "extends",
          "constructor",
          "property",
          "get",
          "set",
          "static",
          "override",
          "case",
          "do",
          "label",
          "break",
          "continue",
          "typeof",
          "module",
          "macro",
          "spawn",
          "operator",
          "new",
          "lazy",
          "yield",
          "defer",
          "public",
          "private",
          "protected",
          "internal",
          "deep",
          "immutable",
          "readonly",
          "out",
          "in",
          "is",
          "as",
          "not",
          "and",
          "or",
        ],
        typeKeywords: [
          "Int",
          "Float",
          "String",
          "Bool",
          "Char",
          "Void",
          "Any",
          "Null",
          "Array",
          "Map",
          "Set",
          "Promise",
          "Result",
          "Option",
          "Function",
          "Object",
          "Class",
          "Enum",
          "Type",
        ],
        operators: [
          "=",
          ">",
          "<",
          "!",
          "~",
          "?",
          ":",
          "==",
          "<=",
          ">=",
          "!=",
          "&&",
          "||",
          "++",
          "--",
          "+",
          "-",
          "*",
          "/",
          "&",
          "|",
          "^",
          "%",
          "<<",
          ">>",
          ">>>",
          "+=",
          "-=",
          "*=",
          "/=",
          "&=",
          "|=",
          "^=",
          "%=",
          "<<=",
          ">>=",
          ">>>=",
          "=>",
          "->",
          "|>",
          "??",
          "?.",
          "...",
          "..",
        ],
        symbols: /[=><!~?:&|+\-*\/\^%]+/,
        escapes:
          /\\(?:[abfnrtv\\"']|x[0-9A-Fa-f]{1,4}|u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8})/,
        tokenizer: {
          root: [
            [
              /[a-z_$][\w$]*/,
              {
                cases: {
                  "@typeKeywords": "keyword.type",
                  "@keywords": "keyword",
                  "@default": "identifier",
                },
              },
            ],
            [/[A-Z][\w$]*/, "type.identifier"],
            { include: "@whitespace" },
            [/[{}()\[\]]/, "@brackets"],
            [/[<>](?!@symbols)/, "@brackets"],
            [
              /@symbols/,
              {
                cases: {
                  "@operators": "operator",
                  "@default": "",
                },
              },
            ],
            [/\d*\.\d+([eE][\-+]?\d+)?/, "number.float"],
            [/0[xX][0-9a-fA-F]+/, "number.hex"],
            [/\d+/, "number"],
            [/[;,.]/, "delimiter"],
            [/"([^"\\]|\\.)*$/, "string.invalid"],
            [/"/, { token: "string.quote", bracket: "@open", next: "@string" }],
            [/'[^\\']'/, "string"],
            [/(')(@escapes)(')/, ["string", "string.escape", "string"]],
            [/'/, "string.invalid"],
          ],
          comment: [
            [/[^\/*]+/, "comment"],
            [/\/\*/, "comment", "@push"],
            ["\\*/", "comment", "@pop"],
            [/[\/*]/, "comment"],
          ],
          string: [
            [/[^\\"]+/, "string"],
            [/@escapes/, "string.escape"],
            [/\\./, "string.escape.invalid"],
            [/"/, { token: "string.quote", bracket: "@close", next: "@pop" }],
          ],
          whitespace: [
            [/[ \t\r\n]+/, "white"],
            [/\/\*/, "comment", "@comment"],
            [/\/\/.*$/, "comment"],
          ],
        },
      });

      // Define Theme
      monaco.editor.defineTheme("pleno-dark", {
        base: "vs-dark",
        inherit: true,
        rules: [
          { token: "keyword", foreground: "C586C0" },
          { token: "keyword.type", foreground: "569CD6" },
          { token: "type.identifier", foreground: "4EC9B0" },
          { token: "string", foreground: "CE9178" },
          { token: "number", foreground: "B5CEA8" },
          { token: "comment", foreground: "6A9955" },
          { token: "operator", foreground: "D4D4D4" },
        ],
        colors: {
          "editor.background": "#0f1117",
        },
      });

      editor = monaco.editor.create(
        document.getElementById("editor-container"),
        {
          value: `// Welcome to Pleno Playground!
fn greet(name) {
    print("Hello, " + name + "!");
}

greet("WebAssembly");

let mut sum = 0;
for (i in 1..=10) {
    sum = sum + i;
}
print("Sum of 1 to 10 is: " + sum);`,
          language: "pleno",
          theme: "pleno-dark",
          automaticLayout: true,
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: 15,
          lineHeight: 24,
          minimap: { enabled: false },
          scrollbar: {
            vertical: "hidden",
            horizontal: "hidden",
          },
          overviewRulerLanes: 0,
          hideCursorInOverviewRuler: true,
          renderLineHighlight: "none",
          scrollBeyondLastLine: false,
        },
      );

      resolve();
    });
  });
}

async function run() {
  status.innerText = "Initializing WASM...";
  await init();
  await setupEditor();
  status.innerText = "Ready";

  runBtn.addEventListener("click", () => {
    const source = editor.getValue();
    status.innerText = "Running...";

    try {
      const result = run_source(source);
      output.innerHTML = "";
      const pre = document.createElement("pre");
      pre.textContent = result;
      output.appendChild(pre);
      status.innerText = "Execution finished";
    } catch (e) {
      output.innerHTML = `<div style="color: #bf616a;">Error: ${e}</div>`;
      status.innerText = "Error";
    }
  });

  clearBtn.addEventListener("click", () => {
    output.innerHTML =
      "<div class=\"system-msg\">Press 'Run' to execute code.</div>";
    status.innerText = "Cleared";
  });
}

run().catch(console.error);
