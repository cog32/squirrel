const assert = require("node:assert/strict");
const os = require("node:os");
const path = require("node:path");
const fs = require("node:fs");
const { spawn, spawnSync } = require("node:child_process");

const { BeforeAll, AfterAll, Given, When, Then, setDefaultTimeout } = require("@cucumber/cucumber");
const { Builder, By, Capabilities, until } = require("selenium-webdriver");

setDefaultTimeout(120_000);

const repoRoot = path.resolve(__dirname, "../..");
const tauriDriverPath = path.resolve(os.homedir(), ".cargo", "bin", "tauri-driver");

let tauriDriver;
let driver;
let generatedDir;

function mkTempDir(prefix) {
  const base = fs.mkdtempSync(path.join(os.tmpdir(), `${prefix}-`));
  return base;
}

function resolveAppBinaryPath() {
  const targetDir = path.join(repoRoot, "src-tauri", "target", "debug");
  const candidateNames = [
    "squirrel-covid",
    "squirrel_covid",
    "tauri-app",
    "app",
  ];

  const platformExt = process.platform === "win32" ? ".exe" : "";
  for (const name of candidateNames) {
    const candidate = path.join(targetDir, `${name}${platformExt}`);
    if (fs.existsSync(candidate)) return candidate;
  }

  // macOS may output an .app even with --no-bundle, depending on Tauri version/config.
  const bundleDir = path.join(targetDir, "bundle", "macos");
  if (fs.existsSync(bundleDir)) {
    const apps = fs.readdirSync(bundleDir).filter((f) => f.endsWith(".app"));
    for (const app of apps) {
      const binName = path.basename(app, ".app");
      const candidate = path.join(bundleDir, app, "Contents", "MacOS", binName);
      if (fs.existsSync(candidate)) return candidate;
    }
  }

  throw new Error(
    `Unable to locate built Tauri binary under ${targetDir}. Run: npx tauri build --debug --no-bundle`,
  );
}

function ensureTauriDriverInstalled() {
  if (!fs.existsSync(tauriDriverPath)) {
    throw new Error(
      `tauri-driver is not installed at ${tauriDriverPath}. Install with: cargo install tauri-driver --locked`,
    );
  }
}

async function startWebdriverSession() {
  ensureTauriDriverInstalled();

  spawnSync("npx", ["tauri", "build", "--debug", "--bundles", "none"], {
    cwd: repoRoot,
    stdio: "inherit",
    shell: true,
    env: { ...process.env, VITE_E2E: "1" },
  });

  generatedDir = mkTempDir("squirrel-covid-e2e-generated");

  tauriDriver = spawn(tauriDriverPath, [], {
    stdio: [null, process.stdout, process.stderr],
    env: { ...process.env, SQUIRREL_GENERATED_DIR: generatedDir },
  });

  const appBinary = resolveAppBinaryPath();
  const capabilities = new Capabilities();
  capabilities.set("tauri:options", { application: appBinary });
  capabilities.setBrowserName("wry");

  driver = await new Builder()
    .withCapabilities(capabilities)
    .usingServer("http://127.0.0.1:4444/")
    .build();

  await driver.wait(until.elementLocated(By.css('[data-testid="app-ready"]')), 30_000);
}

async function stopWebdriverSession() {
  try {
    if (driver) await driver.quit();
  } finally {
    if (tauriDriver) tauriDriver.kill();
    if (generatedDir) {
      try {
        fs.rmSync(generatedDir, { recursive: true, force: true });
      } catch {}
    }
  }
}

BeforeAll(async () => {
  if (process.env.E2E_BACKEND === "appium-mac2") return;
  await startWebdriverSession();
});

AfterAll(async () => {
  if (process.env.E2E_BACKEND === "appium-mac2") return;
  await stopWebdriverSession();
});

Given("the app is running", async () => {
  const el = await driver.findElement(By.css('[data-testid="app-ready"]'));
  assert.equal(await el.getAttribute("data-testid"), "app-ready");
});

When('I parse the transactions file {string}', async (relativePath) => {
  const fixturePath = path.resolve(repoRoot, relativePath);
  assert.ok(fs.existsSync(fixturePath), `fixture file missing: ${fixturePath}`);

  const input = await driver.findElement(By.css("#e2e-parse-path"));
  await input.clear();
  await input.sendKeys(fixturePath);

  const run = await driver.findElement(By.css("#e2e-parse-run"));
  await run.click();

  await driver.wait(
    until.elementTextContains(
      driver.findElement(By.css('[data-testid="parse-status"]')),
      "Parsed",
    ),
    30_000,
  );
});

Then('the sidebar should include the account {string}', async (account) => {
  const selector = `[data-testid="account-item"][data-account="${account}"]`;
  await driver.wait(until.elementLocated(By.css(selector)), 10_000);
});

Then('the sidebar should include the account group {string}', async (group) => {
  const selector = `[data-testid="account-group"][data-group="${group}"]`;
  await driver.wait(until.elementLocated(By.css(selector)), 10_000);
});

Then('the account group {string} should include the account {string}', async (group, account) => {
  const selector = `[data-testid="account-group"][data-group="${group}"] [data-testid="account-item"][data-account="${account}"]`;
  await driver.wait(until.elementLocated(By.css(selector)), 10_000);
});

Then('the account {string} should show an amount of {string}', async (account, amount) => {
  const selector = `[data-testid="account-item"][data-account="${account}"]`;
  const item = await driver.findElement(By.css(selector));
  const amt = await item.findElement(By.css(".account__amt"));
  assert.equal(await amt.getText(), amount);
});

When('I select the account {string}', async (account) => {
  const selector = `[data-testid="account-item"][data-account="${account}"]`;
  const item = await driver.findElement(By.css(selector));
  await item.click();

  await driver.wait(
    until.elementTextContains(
      driver.findElement(By.css('[data-testid="selected-account"]')),
      account,
    ),
    10_000,
  );
});

Then('I should see a transaction row for payee {string}', async (payee) => {
  const row = await driver.findElement(By.css('[data-testid="txn-row"]'));
  const cell = await row.findElement(By.css('[data-testid="txn-payee"]'));
  assert.equal(await cell.getText(), payee);
});

Then('I should see a notes value of {string}', async (notes) => {
  const row = await driver.findElement(By.css('[data-testid="txn-row"]'));
  const cell = await row.findElement(By.css('[data-testid="txn-notes"]'));
  assert.equal(await cell.getText(), notes);
});

Then('that transaction row should show a deposit of {string}', async (amount) => {
  const row = await driver.findElement(By.css('[data-testid="txn-row"]'));
  const deposit = await row.findElement(By.css('[data-testid="txn-deposit"]'));
  assert.equal(await deposit.getText(), amount);
});

When('I add a manual transaction with payee {string} and notes {string}', async (payee, notes) => {
  const addNew = await driver.findElement(By.css("#addNew"));
  await addNew.click();

  await driver.wait(until.elementLocated(By.css("#manualPayee")), 10_000);
  const payeeInput = await driver.findElement(By.css("#manualPayee"));
  const notesInput = await driver.findElement(By.css("#manualNarration"));
  const postings = await driver.findElement(By.css("#manualPostings"));

  await payeeInput.clear();
  await payeeInput.sendKeys(payee);
  await notesInput.clear();
  await notesInput.sendKeys(notes);

  await postings.clear();
  await postings.sendKeys("assets:cash:usd 3.50 USD\nexpenses:coffee -3.50 USD");

  const save = await driver.findElement(By.css("#manualSave"));
  await save.click();

  await driver.wait(until.elementTextContains(driver.findElement(By.css('[data-testid="parse-status"]')), "added manual"), 30_000);
});

// Add Account feature steps
When('I click the "Add Account" button', async () => {
  const addAccountBtn = await driver.findElement(By.css('[data-testid="add-account-btn"]'));
  await addAccountBtn.click();
  await driver.wait(until.elementLocated(By.css('[data-testid="add-account-modal"]')), 10_000);
});

When('I enter account name {string}', async (accountName) => {
  const input = await driver.findElement(By.css('#newAccountName'));
  await input.clear();
  await input.sendKeys(accountName);
});

When('I submit the new account form', async () => {
  const submitBtn = await driver.findElement(By.css('#addAccountSubmit'));
  await submitBtn.click();
  // Wait for modal to close
  await driver.wait(async () => {
    const modals = await driver.findElements(By.css('[data-testid="add-account-modal"]'));
    return modals.length === 0;
  }, 10_000);
});
