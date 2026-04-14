#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/bench-tracked-programs.sh capture <output-env-file>
  scripts/bench-tracked-programs.sh compare [<base-ref>]
  scripts/bench-tracked-programs.sh compare-files <baseline-env> <candidate-env>

Commands:
  capture        Build tracked programs, run CU tests, write metrics to file.
  compare        JIT: build base-ref (default: master) in a worktree, build HEAD,
                 and compare. Working tree stays untouched.
  compare-files  Compare two previously captured metric files.
EOF
}

capture_metric() {
  local output_file="$1"
  local key="$2"
  local value="$3"
  printf '%s=%s\n' "$key" "$value" >>"$output_file"
}

extract_metric() {
  local label="$1"
  local file="$2"
  grep "$label" "$file" | head -1 | grep -oE '[0-9]+'
}

binary_size() {
  local binary_name="$1"
  local binary_path

  binary_path="$(find target -name "$binary_name" -path '*/deploy/*' | head -1)"
  if [[ -z "$binary_path" ]]; then
    echo "missing binary: $binary_name" >&2
    exit 1
  fi

  wc -c <"$binary_path" | tr -d ' '
}

capture_program_metrics() {
  local output_file="$1"
  local manifest_path="$2"
  local package_name="$3"
  local binary_name="$4"
  local size_key="$5"
  shift 5
  local log_file
  log_file="$(mktemp)"

  cargo build-sbf --tools-version v1.52 --manifest-path "$manifest_path"
  cargo test -p "$package_name" -- --nocapture --test-threads=1 2>&1 | tee "$log_file"

  while (($#)); do
    local key="$1"
    local label="$2"
    shift 2
    capture_metric "$output_file" "$key" "$(extract_metric "$label" "$log_file")"
  done

  capture_metric "$output_file" "$size_key" "$(binary_size "$binary_name")"
  rm -f "$log_file"
}

capture() {
  local output_file="$1"
  mkdir -p "$(dirname "$output_file")"
  : >"$output_file"

  capture_program_metrics \
    "$output_file" \
    "examples/vault/Cargo.toml" \
    "quasar-vault" \
    "quasar_vault.so" \
    "VAULT_SIZE" \
    "VAULT_DEPOSIT_CU" "DEPOSIT CU:" \
    "VAULT_WITHDRAW_CU" "WITHDRAW CU:"

  capture_program_metrics \
    "$output_file" \
    "examples/escrow/Cargo.toml" \
    "quasar-escrow" \
    "quasar_escrow.so" \
    "ESCROW_SIZE" \
    "ESCROW_MAKE_CU" "MAKE CU:" \
    "ESCROW_TAKE_CU" "TAKE CU:" \
    "ESCROW_REFUND_CU" "REFUND CU:"
}

metric_value() {
  local key="$1"
  local value="${!key-}"
  printf '%s' "$value"
}

compare_metric() {
  local key="$1"
  local kind="$2"
  local base candidate
  base="$(metric_value "$key")"
  candidate="$(metric_value "CANDIDATE_$key")"

  if [[ -z "$base" || -z "$candidate" ]]; then
    return 0
  fi

  local delta=$((candidate - base))
  printf '%-20s base=%-8s candidate=%-8s delta=%+d\n' "$key" "$base" "$candidate" "$delta"

  if [[ "$kind" == "cu" && "$delta" -gt 0 ]]; then
    return 1
  fi
}

compare_files() {
  local baseline_file="$1"
  local candidate_file="$2"
  local failed=0

  set -a
  # shellcheck disable=SC1090
  source "$baseline_file"
  set +a

  while IFS='=' read -r key value; do
    [[ -z "$key" ]] && continue
    [[ "$key" =~ ^# ]] && continue
    export "CANDIDATE_$key=$value"
  done <"$candidate_file"

  echo "Comparing tracked CU and size metrics"
  echo

  for key in \
    VAULT_DEPOSIT_CU \
    VAULT_WITHDRAW_CU \
    ESCROW_MAKE_CU \
    ESCROW_TAKE_CU \
    ESCROW_REFUND_CU
  do
    if ! compare_metric "$key" "cu"; then
      failed=1
    fi
  done

  for key in VAULT_SIZE ESCROW_SIZE; do
    compare_metric "$key" "size" || true
  done

  if [[ "$failed" -ne 0 ]]; then
    echo
    echo "CU regression detected" >&2
    exit 1
  fi
}

compare() {
  local base_ref="${1:-master}"
  local base_env candidate_env worktree_dir

  base_env="$(mktemp)"
  candidate_env="$(mktemp)"
  worktree_dir="$(mktemp -d)"

  trap 'rm -f "$base_env" "$candidate_env"; git worktree remove --force "$worktree_dir" 2>/dev/null || true' EXIT

  echo "=== Capturing candidate (HEAD) ==="
  capture "$candidate_env"

  echo ""
  echo "=== Capturing base ($base_ref) in worktree ==="
  git worktree add --quiet "$worktree_dir" "$base_ref"
  (cd "$worktree_dir" && capture "$base_env")

  echo ""
  compare_files "$base_env" "$candidate_env"
}

main() {
  if (($# < 1)); then
    usage >&2
    exit 1
  fi

  case "$1" in
    capture)
      if (($# != 2)); then
        usage >&2
        exit 1
      fi
      capture "$2"
      ;;
    compare)
      compare "${2:-master}"
      ;;
    compare-files)
      if (($# != 3)); then
        usage >&2
        exit 1
      fi
      compare_files "$2" "$3"
      ;;
    *)
      usage >&2
      exit 1
      ;;
  esac
}

main "$@"
