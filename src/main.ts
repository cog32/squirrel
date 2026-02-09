import "./style.css";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

type Diagnostic = {
  line: number;
  column: number;
  message: string;
};

type Posting = {
  account: string;
  amount: number;
  amount_text?: string;
  commodity: string;
  remainder?: string | null;
};

type Transaction = {
  date: string;
  datetime: string;
  status?: string | null;
  payee?: string | null;
  narration?: string | null;
  meta?: string | null;
  postings: Posting[];
};

type CommodityAmount = {
  commodity: string;
  amount: number;
};

type AccountBalance = {
  account: string;
  totals: CommodityAmount[];
};

type ParseResponse = {
  ok: boolean;
  diagnostics: Diagnostic[];
  transactions: Transaction[];
  balances: AccountBalance[];
};

type ImportStats = {
  imported: number;
  skipped_duplicates: number;
  archived: number;
};

type ImportResponse = {
  stats: ImportStats;
  parse: ParseResponse;
};

type ManualPostingInput = {
  account: string;
  amount: string;
  commodity: string;
  remainder?: string | null;
};

type ManualTransactionInput = {
  datetime: string;
  status?: string | null;
  payee: string;
  narration: string;
  postings: ManualPostingInput[];
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

function formatAmount(amount: number, commodity: string): string {
  const decimals = commodity === "USD" ? 2 : 6;
  return amount.toFixed(decimals);
}

function pickDisplayTotal(totals: CommodityAmount[]): CommodityAmount | undefined {
  return totals.find((t) => t.commodity === "USD") ?? totals[0];
}

function nowYYYYMM(): string {
  const now = new Date();
  return `${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, "0")}`;
}

function nowYYYYMMDD(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(
    now.getDate(),
  ).padStart(2, "0")}`;
}

function applyParse(state: AppState, response: ParseResponse): void {
  state.parse = response;

  const nextBalances = response.balances ?? [];
  const groupNames = Array.from(new Set(nextBalances.map((b) => b.account.split(":")[0] ?? "other")));
  for (const group of groupNames) {
    if (state.expandedGroups[group] === undefined) state.expandedGroups[group] = true;
  }

  if (!state.selectedAccount) {
    state.selectedAccount =
      nextBalances.find((b) => b.account === "assets:cash:usd")?.account ?? nextBalances[0]?.account;
  }
}

async function loadGeneratedLedger(state: AppState): Promise<void> {
  state.busy = true;
  state.status = "Loading ledger...";
  render(state);

  try {
    const response = await invoke<ParseResponse>("load_generated_ledger", { nowYyyymm: nowYYYYMM() });
    applyParse(state, response);
    state.status = undefined;
  } catch (err) {
    state.status = `Error: ${String(err)}`;
    state.parse = undefined;
  } finally {
    state.busy = false;
    render(state);
  }
}

async function importSources(state: AppState, paths: string[]): Promise<void> {
  state.busy = true;
  state.status = "Importing...";
  render(state);

  try {
    const response = await invoke<ImportResponse>("import_generated_sources", {
      nowYyyymm: nowYYYYMM(),
      paths,
    });
    applyParse(state, response.parse);
    const parsedCount = response.parse.transactions.length;
    state.status = `Parsed (${parsedCount} transactions) — imported ${response.stats.imported}, archived ${response.stats.archived}, skipped ${response.stats.skipped_duplicates}`;
  } catch (err) {
    state.status = `Error: ${String(err)}`;
  } finally {
    state.busy = false;
    render(state);
  }
}

function parsePostingLines(text: string): ManualPostingInput[] {
  const lines = text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);

  return lines.map((line) => {
    const parts = line.split(/\s+/);
    if (parts.length < 3) {
      throw new Error(`Invalid posting line: ${line}`);
    }
    const [account, amount, commodity, ...rest] = parts;
    const remainder = rest.length ? rest.join(" ") : null;
    return { account, amount, commodity, remainder };
  });
}

async function addManualTransaction(state: AppState): Promise<void> {
  const draft = state.manualDraft;
  if (!draft) return;

  const input: ManualTransactionInput = {
    datetime: draft.datetime.trim(),
    status: "*",
    payee: draft.payee.trim(),
    narration: draft.narration.trim(),
    postings: parsePostingLines(draft.postingsText),
  };

  if (!input.datetime || !input.payee || !input.narration) {
    throw new Error("Date, payee, and narration are required.");
  }
  if (input.postings.length < 2) {
    throw new Error("At least two postings are required.");
  }

  state.busy = true;
  state.status = "Adding transaction...";
  render(state);

  try {
    const response = await invoke<ParseResponse>("add_manual_to_generated_ledger", {
      nowYyyymm: nowYYYYMM(),
      input,
    });
    applyParse(state, response);
    const parsedCount = response.transactions.length;
    state.status = `Parsed (${parsedCount} transactions) — added manual transaction`;
    state.manualDraft = undefined;
  } finally {
    state.busy = false;
    render(state);
  }
}

async function addAccountDeclaration(state: AppState): Promise<void> {
  const draft = state.addAccountDraft;
  if (!draft) return;

  const accountName = draft.accountName.trim();
  if (!accountName) {
    throw new Error("Account name is required.");
  }

  const currency = draft.currency.trim() || null;
  const openingBalance = draft.openingBalance.trim() || null;

  state.busy = true;
  state.status = "Adding account...";
  render(state);

  try {
    const response = await invoke<ParseResponse>("add_account_to_generated_ledger", {
      accountName,
      currency,
      openingBalance,
    });
    applyParse(state, response);
    state.status = `Added account "${accountName}"`;
    state.addAccountDraft = undefined;
  } finally {
    state.busy = false;
    render(state);
  }
}

function render(state: AppState): void {
  const balances = state.parse?.balances ?? [];
  const groups = new Map<string, AccountBalance[]>();
  for (const balance of balances) {
    const group = balance.account.split(":")[0] ?? "other";
    const list = groups.get(group) ?? [];
    list.push(balance);
    groups.set(group, list);
  }

  const groupEntries = Array.from(groups.entries()).sort(([a], [b]) => a.localeCompare(b));

  const selectedAccount =
    state.selectedAccount ??
    balances.find((b) => b.account === "assets:cash:usd")?.account ??
    balances[0]?.account;

  const selectedBalance = selectedAccount
    ? balances.find((b) => b.account === selectedAccount)
    : undefined;
  const selectedTotal = selectedBalance ? pickDisplayTotal(selectedBalance.totals) : undefined;

  const allTransactions = state.parse?.transactions ?? [];
  const visibleTransactions = selectedAccount
    ? allTransactions.filter((t) => t.postings.some((p) => p.account === selectedAccount))
    : allTransactions;

  const searchQuery = state.search.trim().toLowerCase();
  const filteredTransactions =
    searchQuery.length === 0
      ? visibleTransactions
      : visibleTransactions.filter((t) => {
          const haystack = `${t.payee ?? ""} ${t.narration ?? ""} ${t.meta ?? ""}`.toLowerCase();
          return haystack.includes(searchQuery);
        });

  const uncategorisedCount = selectedAccount
    ? filteredTransactions.filter((t) => {
        const other = t.postings.find((p) => p.account !== selectedAccount);
        return !other;
      }).length
    : 0;

  const diagnostics = state.parse?.diagnostics ?? [];
  const statusText =
    state.status ??
    (state.parse
      ? state.parse.ok
        ? `Parsed (${allTransactions.length} transactions)`
        : `Parsed with ${diagnostics.length} diagnostics`
      : "Pick a file to begin.");

  app.innerHTML = `
    <div class="layout" data-testid="app-ready">
      <aside class="sidebar">
        <div class="sidebar__top">
          <div class="brand">Squirrel</div>
          <div class="brandSub">Plain Text Accounting</div>
        </div>

        <div class="sidebar__section">
          <div class="sidebar__title">Accounts</div>
          <div class="accounts" role="list">
            ${groupEntries
              .map(([group, items]) => {
                const open = state.expandedGroups[group] ?? true;
                const chevronClass = open ? "group__chevron group__chevron--open" : "group__chevron";
                const children = items
                  .sort((a, b) => a.account.localeCompare(b.account))
                  .map((b) => {
                    const total = pickDisplayTotal(b.totals);
                    const amount = total ? formatAmount(total.amount, total.commodity) : "0.00";
                    const active = b.account === selectedAccount;
                    const displayName = b.account.startsWith(`${group}:`)
                      ? b.account.slice(group.length + 1)
                      : b.account;
                    return `
                      <button
                        class="account ${active ? "account--active" : ""}"
                        type="button"
                        data-testid="account-item"
                        data-account="${escapeText(b.account)}"
                      >
                        <span class="account__name">${escapeText(displayName)}</span>
                        <span class="account__amt">${escapeText(amount)}</span>
                      </button>
                    `;
                  })
                  .join("");

                return `
                  <div class="group" data-testid="account-group" data-group="${escapeText(group)}">
                    <button
                      class="group__toggle"
                      type="button"
                      data-testid="account-group-toggle"
                      data-group="${escapeText(group)}"
                      aria-expanded="${open ? "true" : "false"}"
                    >
                      <span class="${chevronClass}">▸</span>
                      <span class="group__name">${escapeText(group)}</span>
                    </button>
                    ${open ? `<div class="group__children">${children}</div>` : ""}
                  </div>
                `;
              })
              .join("")}
          </div>
        </div>
        <div class="sidebar__bottom">
          <button
            id="addAccountBtn"
            class="btn btn--secondary btn--full"
            type="button"
            data-testid="add-account-btn"
            ${state.busy ? "disabled" : ""}
          >+ Add Account</button>
        </div>
      </aside>

      <main class="main">
        <header class="topbar">
          <div class="topbar__left">
            <div class="accountHeader">
              <div class="accountHeader__name" data-testid="selected-account">
                ${selectedAccount ? escapeText(selectedAccount) : "No account"}
                <span class="accountHeader__caret" aria-hidden="true">▾</span>
              </div>
              <div class="accountHeader__balance">${
                selectedTotal
                  ? `${escapeText(formatAmount(selectedTotal.amount, selectedTotal.commodity))}`
                  : ""
              }</div>
            </div>
          </div>

          <div class="topbar__right">
            <div class="statusLine">
              <span class="status" data-testid="parse-status">${escapeText(statusText)}</span>
              ${
                uncategorisedCount > 0
                  ? `<span class="pill">${uncategorisedCount} uncategorised</span>`
                  : ""
              }
            </div>
            <input
              id="search"
              class="search"
              type="search"
              placeholder="Search"
              value="${escapeText(state.search)}"
              ${state.busy ? "disabled" : ""}
            />
          </div>
        </header>

        <section class="actionBar">
          <button id="importFile" class="actionBtn" ${state.busy ? "disabled" : ""}>
            <span class="actionBtn__icon" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor">
                <path d="M10 2a1 1 0 0 1 1 1v7.59l2.3-2.3a1 1 0 1 1 1.4 1.42l-4 4a1 1 0 0 1-1.4 0l-4-4a1 1 0 0 1 1.4-1.42l2.3 2.3V3a1 1 0 0 1 1-1Z" />
                <path d="M3 14a1 1 0 0 1 1 1v1h12v-1a1 1 0 1 1 2 0v2a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1v-2a1 1 0 0 1 1-1Z" />
              </svg>
            </span>
            Import
          </button>
          <button id="addNew" class="actionBtn" ${state.busy ? "disabled" : ""}>
            <span class="actionBtn__icon" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor">
                <path d="M10 2a1 1 0 0 1 1 1v6h6a1 1 0 1 1 0 2h-6v6a1 1 0 1 1-2 0v-6H3a1 1 0 1 1 0-2h6V3a1 1 0 0 1 1-1Z" />
              </svg>
            </span>
            Add New
          </button>
          <button id="filterBtn" class="actionBtn actionBtn--ghost" ${
            state.busy ? "disabled" : ""
          }>
            <span class="actionBtn__icon" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor">
                <path d="M3 4a1 1 0 0 1 1-1h12a1 1 0 0 1 .78 1.63L12 10.5V16a1 1 0 0 1-1.45.9l-3-1.5A1 1 0 0 1 7 14.5v-4L3.22 5.63A1 1 0 0 1 3 5V4Z" />
              </svg>
            </span>
            Filter
          </button>
        </section>

        ${
          import.meta.env.VITE_E2E === "1"
            ? `
              <section class="e2ePanel">
                <div class="e2ePanel__label">E2E</div>
                <input id="e2e-parse-path" class="e2ePanel__input" placeholder="/abs/path/to/file.transactions" />
                <button id="e2e-parse-run" class="btn btn--secondary" ${state.busy ? "disabled" : ""}>Parse path</button>
              </section>
            `
            : ""
        }

        <section class="content">
          <div class="tableCard">
            <table class="txTable">
              <thead>
                <tr>
                  <th>Date</th>
                  <th>Payee</th>
                  <th>Notes</th>
                  <th>Category</th>
                  <th class="num">Payment</th>
                  <th class="num">Deposit</th>
                </tr>
              </thead>
              <tbody>
                ${
                  filteredTransactions.length === 0
                    ? `<tr><td colspan="6" class="empty">No transactions.</td></tr>`
                    : filteredTransactions
                        .map((t) => {
                          const postingAmount = selectedAccount
                            ? t.postings
                                .filter((p) => p.account === selectedAccount)
                                .reduce((sum, p) => sum + p.amount, 0)
                            : 0;

                          const commodity =
                            selectedAccount &&
                            t.postings.find((p) => p.account === selectedAccount)?.commodity
                              ? t.postings.find((p) => p.account === selectedAccount)!.commodity
                              : "USD";

                          const other = selectedAccount
                            ? t.postings.find((p) => p.account !== selectedAccount)
                            : undefined;

                          const payment =
                            postingAmount < 0 ? formatAmount(Math.abs(postingAmount), commodity) : "";
                          const deposit = postingAmount > 0 ? formatAmount(postingAmount, commodity) : "";

                          return `
                            <tr data-testid="txn-row">
                              <td>${escapeText(t.date)}</td>
                              <td data-testid="txn-payee">${escapeText(t.payee ?? "")}</td>
                              <td>
                                <div class="notes__main" data-testid="txn-notes">${escapeText(
                                  t.narration ?? "",
                                )}</div>
                              </td>
                              <td class="category">${escapeText(other?.account ?? "—")}</td>
                              <td class="num">${escapeText(payment)}</td>
                              <td class="num" data-testid="txn-deposit">${escapeText(deposit)}</td>
                            </tr>
                          `;
                        })
                        .join("")
                }
              </tbody>
            </table>
          </div>

          ${
            diagnostics.length > 0
              ? `
                <details class="diagnostics" open>
                  <summary>Diagnostics (${diagnostics.length})</summary>
                  <ul class="diagList">
                    ${diagnostics
                      .map(
                        (d) =>
                          `<li><code>line ${d.line}, col ${d.column}</code> — ${escapeText(d.message)}</li>`,
                      )
                      .join("")}
                  </ul>
                </details>
              `
              : ""
          }
        </section>
      </main>
    </div>
    ${
      state.manualDraft
        ? `
          <div class="modalOverlay" role="dialog" aria-modal="true">
            <div class="modal">
              <div class="modal__title">Add New (Manual)</div>
              <label class="field">
                <div class="field__label">Date</div>
                <input id="manualDate" class="field__input" value="${escapeText(
                  state.manualDraft.datetime,
                )}" ${state.busy ? "disabled" : ""} />
              </label>
              <label class="field">
                <div class="field__label">Payee</div>
                <input id="manualPayee" class="field__input" value="${escapeText(
                  state.manualDraft.payee,
                )}" ${state.busy ? "disabled" : ""} />
              </label>
              <label class="field">
                <div class="field__label">Notes</div>
                <input id="manualNarration" class="field__input" value="${escapeText(
                  state.manualDraft.narration,
                )}" ${state.busy ? "disabled" : ""} />
              </label>
              <label class="field">
                <div class="field__label">Postings (one per line: <code>account amount commodity</code>)</div>
                <textarea id="manualPostings" class="field__textarea" ${
                  state.busy ? "disabled" : ""
                }>${escapeText(state.manualDraft.postingsText)}</textarea>
              </label>
              ${
                state.manualDraft.error
                  ? `<div class="modal__error">${escapeText(state.manualDraft.error)}</div>`
                  : ""
              }
              <div class="modal__actions">
                <button id="manualCancel" class="btn btn--secondary" ${
                  state.busy ? "disabled" : ""
                }>Cancel</button>
                <button id="manualSave" class="btn" ${state.busy ? "disabled" : ""}>Save</button>
              </div>
            </div>
          </div>
        `
        : ""
    }
    ${
      state.addAccountDraft
        ? `
          <div class="modalOverlay" role="dialog" aria-modal="true" data-testid="add-account-modal">
            <div class="modal">
              <div class="modal__title">Add Account</div>
              <label class="field">
                <div class="field__label">Account Name (e.g., assets:bank:savings)</div>
                <input id="newAccountName" class="field__input" value="${escapeText(
                  state.addAccountDraft.accountName,
                )}" ${state.busy ? "disabled" : ""} placeholder="assets:bank:savings" autocapitalize="off" autocorrect="off" spellcheck="false" />
              </label>
              <label class="field">
                <div class="field__label">Default Currency (optional)</div>
                <input id="newAccountCurrency" class="field__input" value="${escapeText(
                  state.addAccountDraft.currency,
                )}" ${state.busy ? "disabled" : ""} placeholder="USD" />
              </label>
              <label class="field">
                <div class="field__label">Opening Balance (optional)</div>
                <input id="newAccountBalance" class="field__input" value="${escapeText(
                  state.addAccountDraft.openingBalance,
                )}" ${state.busy ? "disabled" : ""} placeholder="0.00" />
              </label>
              ${
                state.addAccountDraft.error
                  ? `<div class="modal__error">${escapeText(state.addAccountDraft.error)}</div>`
                  : ""
              }
              <div class="modal__actions">
                <button id="addAccountCancel" class="btn btn--secondary" ${
                  state.busy ? "disabled" : ""
                }>Cancel</button>
                <button id="addAccountSubmit" class="btn" ${state.busy ? "disabled" : ""}>Add</button>
              </div>
            </div>
          </div>
        `
        : ""
    }
  `;

  const importButton = document.querySelector<HTMLButtonElement>("#importFile");
  importButton?.addEventListener("click", async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Transactions", extensions: ["transactions"] }],
    });

    if (!selected) {
      state.status = "No files selected.";
      render(state);
      return;
    }

    const paths = Array.isArray(selected) ? selected : [selected];
    await importSources(state, paths);
  });

  const addNew = document.querySelector<HTMLButtonElement>("#addNew");
  addNew?.addEventListener("click", () => {
    state.manualDraft = {
      datetime: nowYYYYMMDD(),
      payee: "",
      narration: "",
      postingsText: "assets:cash:usd 0.00 USD\nexpenses:unknown 0.00 USD",
      error: undefined,
    };
    render(state);
  });

  const filterBtn = document.querySelector<HTMLButtonElement>("#filterBtn");
  filterBtn?.addEventListener("click", () => {
    state.status = "Filter is a placeholder for now.";
    render(state);
  });

  const searchInput = document.querySelector<HTMLInputElement>("#search");
  searchInput?.addEventListener("input", (e) => {
    state.search = (e.target as HTMLInputElement).value;
    render(state);
  });

  document.querySelectorAll<HTMLButtonElement>('[data-testid="account-item"]').forEach((el) => {
    el.addEventListener("click", () => {
      const account = el.getAttribute("data-account") ?? "";
      state.selectedAccount = account;
      render(state);
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-testid="account-group-toggle"]').forEach((el) => {
    el.addEventListener("click", () => {
      const group = el.getAttribute("data-group") ?? "";
      state.expandedGroups[group] = !(state.expandedGroups[group] ?? true);
      render(state);
    });
  });

  const e2eRun = document.querySelector<HTMLButtonElement>("#e2e-parse-run");
  e2eRun?.addEventListener("click", async () => {
    const input = document.querySelector<HTMLInputElement>("#e2e-parse-path");
    const filePath = input?.value?.trim();
    if (!filePath) return;
    await importSources(state, [filePath]);
  });

  const manualCancel = document.querySelector<HTMLButtonElement>("#manualCancel");
  manualCancel?.addEventListener("click", () => {
    state.manualDraft = undefined;
    render(state);
  });

  const manualSave = document.querySelector<HTMLButtonElement>("#manualSave");
  manualSave?.addEventListener("click", async () => {
    if (!state.manualDraft) return;
    state.manualDraft.error = undefined;

    const date = document.querySelector<HTMLInputElement>("#manualDate")?.value ?? "";
    const payee = document.querySelector<HTMLInputElement>("#manualPayee")?.value ?? "";
    const narration = document.querySelector<HTMLInputElement>("#manualNarration")?.value ?? "";
    const postings = document.querySelector<HTMLTextAreaElement>("#manualPostings")?.value ?? "";

    state.manualDraft.datetime = date;
    state.manualDraft.payee = payee;
    state.manualDraft.narration = narration;
    state.manualDraft.postingsText = postings;

    try {
      await addManualTransaction(state);
    } catch (err) {
      if (state.manualDraft) {
        state.manualDraft.error = String(err);
        state.busy = false;
        render(state);
      }
    }
  });

  // Add Account handlers
  const addAccountBtn = document.querySelector<HTMLButtonElement>("#addAccountBtn");
  addAccountBtn?.addEventListener("click", () => {
    state.addAccountDraft = {
      accountName: "",
      currency: "",
      openingBalance: "",
      error: undefined,
    };
    render(state);
  });

  const addAccountCancel = document.querySelector<HTMLButtonElement>("#addAccountCancel");
  addAccountCancel?.addEventListener("click", () => {
    state.addAccountDraft = undefined;
    render(state);
  });

  const addAccountSubmit = document.querySelector<HTMLButtonElement>("#addAccountSubmit");
  addAccountSubmit?.addEventListener("click", async () => {
    if (!state.addAccountDraft) return;
    state.addAccountDraft.error = undefined;

    const accountName = document.querySelector<HTMLInputElement>("#newAccountName")?.value ?? "";
    const currency = document.querySelector<HTMLInputElement>("#newAccountCurrency")?.value ?? "";
    const openingBalance = document.querySelector<HTMLInputElement>("#newAccountBalance")?.value ?? "";
    state.addAccountDraft.accountName = accountName;
    state.addAccountDraft.currency = currency;
    state.addAccountDraft.openingBalance = openingBalance;

    try {
      await addAccountDeclaration(state);
    } catch (err) {
      if (state.addAccountDraft) {
        state.addAccountDraft.error = String(err);
        state.busy = false;
        render(state);
      }
    }
  });
}

type AppState = {
  busy: boolean;
  parse?: ParseResponse;
  selectedAccount?: string;
  search: string;
  status?: string;
  expandedGroups: Record<string, boolean>;
  manualDraft?: {
    datetime: string;
    payee: string;
    narration: string;
    postingsText: string;
    error?: string;
  };
  addAccountDraft?: {
    accountName: string;
    currency: string;
    openingBalance: string;
    error?: string;
  };
};

const state: AppState = {
  busy: false,
  search: "",
  expandedGroups: {},
};

render(state);

(async () => {
  try {
    await invoke<string>("rotate_generated_ledger", { nowYyyymm: nowYYYYMM() });
  } catch {
    // Best-effort rotation; ignore when unavailable (e.g. in non-Tauri contexts).
  }

  try {
    await loadGeneratedLedger(state);
  } catch {
    // Ignore when unavailable (e.g. in non-Tauri contexts).
  }
})();
