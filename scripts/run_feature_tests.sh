#!/usr/bin/env bash
set -euo pipefail

echo "Running BDD feature tests (Cucumber, one-shot)…"
npm run -s test:bdd

