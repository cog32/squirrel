#!/usr/bin/env bash
set -euo pipefail

echo "Running E2E tests (Playwright, one-shot)…"

if ! node -e "require.resolve('@playwright/test')" >/dev/null 2>&1; then
  echo "Skipping E2E: @playwright/test is not installed."
  exit 0
fi

if [[ ! -f "playwright.config.ts" ]]; then
  echo "Skipping E2E: playwright.config.ts not found."
  exit 0
fi

npx playwright test --config=playwright.config.ts
