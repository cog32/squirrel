#!/usr/bin/env bash
set -euo pipefail

echo "Running unit tests…"

# Prefer Rust unit tests for Tauri-based repos.
if [[ -f "src-tauri/Cargo.toml" ]]; then
  echo "Detected Rust/Tauri; running cargo tests…"
  cargo test --manifest-path src-tauri/Cargo.toml
  exit 0
fi

if [[ -f "package.json" ]]; then
  if node -e "require.resolve('vitest/package.json')" >/dev/null 2>&1; then
    echo "Detected Vitest; running with coverage thresholds…"

    if ! node -e "require.resolve('@vitest/coverage-v8/package.json')" >/dev/null 2>&1; then
      echo "ERROR: Coverage plugin '@vitest/coverage-v8' is required locally." >&2
      echo "Install it with: npm i -D @vitest/coverage-v8" >&2
      exit 1
    fi

    npx vitest run --coverage --reporter=dot

    # Enforce coverage thresholds explicitly using the JSON summary
    node - <<'NODE'
const fs = require('fs')
const path = 'coverage/coverage-summary.json'
if (!fs.existsSync(path)) {
  console.error('[coverage] coverage-summary.json not found; failing')
  process.exit(1)
}
const summary = JSON.parse(fs.readFileSync(path, 'utf8'))
const t = { lines: 80, functions: 80, branches: 70, statements: 80 }
const total = summary.total || {}
function pct(key) { return (total[key] && typeof total[key].pct === 'number') ? total[key].pct : 0 }
const results = {
  lines: pct('lines'),
  functions: pct('functions'),
  branches: pct('branches'),
  statements: pct('statements'),
}
let ok = true
for (const k of Object.keys(t)) {
  if (results[k] < t[k]) {
    console.error(`[coverage] ${k}: ${results[k]}% < threshold ${t[k]}%`)
    ok = false
  }
}
if (!ok) process.exit(1)
console.log('[coverage] thresholds met:', results)
NODE

    exit 0
  fi

  if grep -q '"test"' package.json; then
    echo "No Vitest detected; running npm test…"
    npm test
    exit 0
  fi
fi

echo "No unit test runner detected; skipping."
