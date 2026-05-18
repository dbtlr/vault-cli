#!/usr/bin/env bash
# Generic repair recipe: detect → plan → dry-run → apply → verify.
# Slice 2 of the GitHub readiness work expands this into a full
# walkthrough. This placeholder exists so Slice 3 CI's shellcheck
# step has a file to lint.

set -euo pipefail

VAULT_ROOT="${1:-.}"
PLAN_PATH="${2:-./repair.json}"

vault -C "${VAULT_ROOT}" validate --summary --format json >/dev/null
vault -C "${VAULT_ROOT}" repair plan --out "${PLAN_PATH}"
vault -C "${VAULT_ROOT}" repair apply "${PLAN_PATH}" --dry-run
vault -C "${VAULT_ROOT}" repair apply "${PLAN_PATH}" --verify
