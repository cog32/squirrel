# UI / E2E BDD (Cucumber)

UI/E2E `.feature` files executed with `cucumber-js`.

## Requirements

- Node dependency `selenium-webdriver` installed (via `npm install`)

### Linux/Windows (Tauri WebDriver)

- `tauri-driver` installed: `cargo install tauri-driver --locked`

### macOS (Appium mac2)

- Xcode installed and accessibility permissions configured (see `./scripts/setup_appium_mac2.sh`)
- Install mac2 driver once: `./scripts/setup_appium_mac2.sh`

## Run

- `npm run e2e` (skips if `tauri-driver` is missing/unsupported)
- Or directly: `npm run test:bdd:ui` (requires a working `tauri-driver`)
