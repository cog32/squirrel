
# Overview

- This is a Plain Text Accounting App

- This is an Open Source, [Local First](https://www.inkandswitch.com/essay/local-first/),  Progressive Web App that prioritizes [File Over Application](https://stephango.com/file-over-app)

- In stark contrast to most other accounting apps, your data is your own and you'll never be locked in.

## Format

This is defined in [Ledger Spec](./Ledger.g4)

## UI

- Click `Import` to select a `.transactions` file.
- The left sidebar lists accounts and their running balances (per commodity, preferring `USD` for display).
- The main table shows transactions affecting the selected account, with `Payment` / `Deposit` split from the selected account’s posting amount.

## Install

Download the latest release from the [Releases](../../releases) tab.

### macOS

1. Download the `.dmg` file
2. Open the disk image and drag **Squirrel COVID** to Applications
3. On first launch, macOS may block the app. Go to **System Settings > Privacy & Security** and click **Open Anyway**

Alternatively, download the `.app.zip`, unzip it, and move the `.app` to Applications.

### Windows

Download **either** installer:

- `.msi` — standard Windows Installer (add/remove programs support)
- `.exe` — NSIS installer

Run the installer and follow the prompts.

### Linux (Debian / Ubuntu)

```sh
sudo dpkg -i squirrel-covid_*_amd64.deb
```

### Linux (Arch)

Use the `.AppImage`:

```sh
chmod +x squirrel-covid_*_amd64.AppImage
./squirrel-covid_*_amd64.AppImage
```

Or convert the `.deb` with [`debtap`](https://aur.archlinux.org/packages/debtap):

```sh
debtap squirrel-covid_*_amd64.deb
sudo pacman -U squirrel-covid-*.pkg.tar.zst
```

### Linux (other)

The `.AppImage` runs on any distribution with FUSE support:

```sh
chmod +x squirrel-covid_*_amd64.AppImage
./squirrel-covid_*_amd64.AppImage
```

## Development

### Run the app

- Web (Vite): `npm run dev`
- Desktop (Tauri): `npm run tauri:dev`

### Tests

- Parser BDD (Rust/Cucumber): `npm run test:bdd`
- UI E2E (Tauri WebDriver + Cucumber):
  - Install `tauri-driver`: `cargo install tauri-driver --locked`
  - Run: `npm run e2e` (or `scripts/run_e2e_tests.sh`)
  - Note: if `tauri-driver` is unsupported on your platform, the runner will skip.

- UI E2E on macOS uses Appium mac2 (since `tauri-driver` is Linux/Windows only today):
  - Setup once: `./scripts/setup_appium_mac2.sh`
  - Run: `npm run e2e` (macOS auto-selects Appium mode)
