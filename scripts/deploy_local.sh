#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "Building local deployment…"

if [[ ! -d "node_modules" ]]; then
  echo "Installing JS dependencies…"
  npm install
fi

echo "Building Tauri bundle…"
npm run -s tauri:build

OS_NAME="$(uname -s)"
if [[ "$OS_NAME" == "Darwin" ]]; then
  PRODUCT_NAME="$(node -p "require('./src-tauri/tauri.conf.json').package.productName")"
  VERSION="$(node -p "require('./src-tauri/tauri.conf.json').package.version")"

  APP_PATH="src-tauri/target/release/bundle/macos/${PRODUCT_NAME}.app"
  if [[ ! -d "$APP_PATH" ]]; then
    echo "ERROR: Expected app bundle not found: $APP_PATH" >&2
    exit 1
  fi

  SAFE_NAME="$(node -p "require('./src-tauri/tauri.conf.json').package.productName.replace(/\\s+/g,'-')")"
  OUT_DIR="bin/test-builds"
  mkdir -p "$OUT_DIR"
  OUT_ZIP="$OUT_DIR/${SAFE_NAME}-${VERSION}-macos.app.zip"
  rm -f "$OUT_ZIP"

  echo "Packaging zip…"
  ditto -c -k --sequesterRsrc --keepParent "$APP_PATH" "$OUT_ZIP"

  echo "Done:"
  echo "  App: $APP_PATH"
  echo "  Zip: $OUT_ZIP"
else
  echo "Done: bundle created under src-tauri/target/release/bundle/"
fi

