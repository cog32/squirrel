import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

function resolveBin(binName) {
  const whichCmd = process.platform === "win32" ? "where" : "which";
  const res = spawnSync(whichCmd, [binName], { encoding: "utf8" });
  if (res.status === 0) {
    const first = res.stdout.trim().split(/\r?\n/)[0];
    if (first) return first;
  }

  const cargoBin = path.join(os.homedir(), ".cargo", "bin", binName + (process.platform === "win32" ? ".exe" : ""));
  if (fs.existsSync(cargoBin)) return cargoBin;

  return null;
}

function hasNodeDep(name) {
  try {
    require.resolve(name);
    return true;
  } catch {
    return false;
  }
}

function main() {
  if (process.platform === "darwin") {
    if (!hasNodeDep("appium")) {
      console.log("Skipping E2E (macOS): appium is not installed.");
      process.exit(0);
    }
    if (!hasNodeDep("selenium-webdriver")) {
      console.log("Skipping E2E (macOS): selenium-webdriver is not installed.");
      process.exit(0);
    }

    const mac2 = spawnSync("npx", ["appium", "driver", "list", "--installed"], {
      encoding: "utf8",
      shell: true,
    });
    const mac2Out = `${mac2.stdout ?? ""}${mac2.stderr ?? ""}`.toLowerCase();
    if (!mac2Out.includes("mac2")) {
      console.log("Skipping E2E (macOS): Appium mac2 driver is not installed.");
      console.log("Install it with: ./scripts/setup_appium_mac2.sh");
      process.exit(0);
    }

    const res = spawnSync("npx", ["cucumber-js", "--tags", "@appium"], {
      stdio: "inherit",
      shell: true,
      env: { ...process.env, E2E_BACKEND: "appium-mac2" },
    });
    process.exit(res.status ?? 1);
  }

  if (!resolveBin("cargo")) {
    console.log("Skipping E2E: cargo is not installed.");
    process.exit(0);
  }

  const tauriDriver = resolveBin("tauri-driver");
  if (!tauriDriver) {
    console.log("Skipping E2E: tauri-driver is not installed.");
    console.log("Install it with: cargo install tauri-driver --locked");
    process.exit(0);
  }

  const version = spawnSync(tauriDriver, ["--version"], { encoding: "utf8" });
  const versionOutput = `${version.stdout ?? ""}${version.stderr ?? ""}`.toLowerCase();
  if (version.status !== 0 && versionOutput.includes("not supported on this platform")) {
    console.log("Skipping E2E: tauri-driver is not supported on this platform.");
    process.exit(0);
  }

  if (!hasNodeDep("selenium-webdriver")) {
    console.log("Skipping E2E: selenium-webdriver is not installed.");
    process.exit(0);
  }

  const res = spawnSync("npx", ["cucumber-js"], { stdio: "inherit", shell: true });
  process.exit(res.status ?? 1);
}

main();
