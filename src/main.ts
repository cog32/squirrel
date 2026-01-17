import "./style.css";
import { open } from "@tauri-apps/api/dialog";
import { invoke } from "@tauri-apps/api/tauri";

type Diagnostic = {
  line: number;
  column: number;
  message: string;
};

type ParseResponse = {
  ok: boolean;
  diagnostics: Diagnostic[];
};

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) {
  throw new Error("#app not found");
}

function escapeText(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function render(status: { filePath?: string; output: string; busy: boolean }): void {
  app.innerHTML = `
    <div class="container">
      <h1>Squirrel COVID</h1>
      <p class="muted">Select a <code>.transactions</code> file to parse it and see diagnostics.</p>

      <div class="card">
        <div class="row">
          <button id="addFile" ${status.busy ? "disabled" : ""}>Add File</button>
          <span class="muted">${status.filePath ? escapeText(status.filePath) : "No file selected"}</span>
        </div>

        <div style="margin-top: 12px;">
          <pre>${escapeText(status.output)}</pre>
        </div>
      </div>
    </div>
  `;

  const btn = document.querySelector<HTMLButtonElement>("#addFile");
  if (!btn) return;

  btn.addEventListener("click", async () => {
    state.busy = true;
    state.output = "Opening file picker...";
    render(state);

    const selected = await open({
      multiple: false,
      filters: [{ name: "Transactions", extensions: ["transactions"] }],
    });

    if (!selected || Array.isArray(selected)) {
      state.busy = false;
      state.output = "No file selected.";
      render(state);
      return;
    }

    state.filePath = selected;
    state.output = `Parsing ${selected}...`;
    render(state);

    try {
      const response = await invoke<ParseResponse>("parse_transactions_file", { path: selected });
      if (response.ok) {
        state.output = "OK";
      } else {
        state.output = response.diagnostics
          .map((d) => `line ${d.line}, column ${d.column}: ${d.message}`)
          .join("\n");
      }
    } catch (err) {
      state.output = `Error: ${String(err)}`;
    } finally {
      state.busy = false;
      render(state);
    }
  });
}

const state: { filePath?: string; output: string; busy: boolean } = {
  output: "Pick a file to begin.",
  busy: false,
};

render(state);
