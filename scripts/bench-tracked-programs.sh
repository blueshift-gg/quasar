#!/usr/bin/env bash
set -euo pipefail

PLATFORM_TOOLS_VERSION="${PLATFORM_TOOLS_VERSION:-v1.52}"
TRACKED_BASELINE_FILE="${TRACKED_BASELINE_FILE:-benchmarks/v0.1.0.env}"

CU_METRICS=(
  VAULT_DEPOSIT_CU
  VAULT_WITHDRAW_CU
  ESCROW_MAKE_CU
  ESCROW_TAKE_CU
  ESCROW_REFUND_CU
  MULTISIG_CREATE_CU
  MULTISIG_DEPOSIT_CU
  MULTISIG_SET_LABEL_CU
  MULTISIG_EXECUTE_TRANSFER_CU
)
SIZE_METRICS=(VAULT_SIZE ESCROW_SIZE MULTISIG_SIZE)
ALL_METRICS=("${CU_METRICS[@]}" "${SIZE_METRICS[@]}")

usage() {
  cat <<'EOF'
Usage:
  scripts/bench-tracked-programs.sh capture <output-env-file>
  scripts/bench-tracked-programs.sh compare [<baseline-env>]
  scripts/bench-tracked-programs.sh compare-files <baseline-env> <candidate-env>

Commands:
  capture        Build tracked programs, run CU tests, write metrics to file.
  compare        Capture HEAD and compare it to the checked-in v0.1.0 baseline.
  compare-files  Compare two previously captured metric files.
EOF
}

platform_tools_rustc() {
  local rustc="$HOME/.cache/solana/$PLATFORM_TOOLS_VERSION/platform-tools/rust/bin/rustc"
  if [[ -x "$rustc" ]]; then
    printf '%s\n' "$rustc"
  fi
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

  local rustc
  rustc="$(platform_tools_rustc)"
  if [[ -n "$rustc" ]]; then
    RUSTC="$rustc" cargo build-sbf \
      --tools-version "$PLATFORM_TOOLS_VERSION" \
      --no-rustup-override \
      --manifest-path "$manifest_path"
  else
    cargo build-sbf --tools-version "$PLATFORM_TOOLS_VERSION" --manifest-path "$manifest_path"
  fi
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

  capture_program_metrics \
    "$output_file" \
    "examples/multisig/Cargo.toml" \
    "quasar-multisig" \
    "quasar_multisig.so" \
    "MULTISIG_SIZE" \
    "MULTISIG_CREATE_CU" "CREATE CU:" \
    "MULTISIG_DEPOSIT_CU" "DEPOSIT CU:" \
    "MULTISIG_SET_LABEL_CU" "SET_LABEL CU:" \
    "MULTISIG_EXECUTE_TRANSFER_CU" "EXECUTE_TRANSFER CU:"
}

is_tracked_metric() {
  local candidate="$1"
  local key
  for key in "${ALL_METRICS[@]}"; do
    if [[ "$candidate" == "$key" ]]; then
      return 0
    fi
  done
  return 1
}

load_metric_file() {
  local file="$1"
  local prefix="$2"
  local key value variable line line_number=0 seen="|"

  if [[ ! -f "$file" ]]; then
    echo "missing metric file: $file" >&2
    return 1
  fi

  for key in "${ALL_METRICS[@]}"; do
    unset "${prefix}${key}"
  done

  while IFS= read -r line || [[ -n "$line" ]]; do
    line_number=$((line_number + 1))
    [[ -z "$line" || "$line" == \#* ]] && continue
    if [[ "$line" != *=* ]]; then
      echo "$file:$line_number: expected KEY=VALUE" >&2
      return 1
    fi

    key="${line%%=*}"
    value="${line#*=}"
    if ! is_tracked_metric "$key"; then
      echo "$file:$line_number: unknown tracked metric: $key" >&2
      return 1
    fi
    case "$seen" in
      *"|$key|"*)
        echo "$file:$line_number: duplicate tracked metric: $key" >&2
        return 1
        ;;
    esac
    if [[ ! "$value" =~ ^[0-9]+$ ]]; then
      echo "$file:$line_number: non-numeric value for $key: $value" >&2
      return 1
    fi

    variable="${prefix}${key}"
    printf -v "$variable" '%s' "$value"
    export "$variable"
    seen="${seen}${key}|"
  done <"$file"

  for key in "${ALL_METRICS[@]}"; do
    variable="${prefix}${key}"
    if [[ -z "${!variable-}" ]]; then
      echo "$file: missing tracked metric: $key" >&2
      return 1
    fi
  done
}

compare_metric() {
  local key="$1"
  local base candidate
  base="${!key}"
  local candidate_key="CANDIDATE_$key"
  candidate="${!candidate_key}"

  local delta=$((candidate - base))
  printf '%-28s baseline=%-8s candidate=%-8s delta=%+d\n' \
    "$key" "$base" "$candidate" "$delta"

  if [[ "$delta" -gt 0 ]]; then
    return 1
  fi
}

compare_files() {
  local baseline_file="$1"
  local candidate_file="$2"
  local failed=0

  load_metric_file "$baseline_file" ""
  load_metric_file "$candidate_file" "CANDIDATE_"

  echo "Comparing tracked metrics to absolute v0.1.0 baselines"
  echo

  local key
  for key in "${ALL_METRICS[@]}"; do
    if ! compare_metric "$key"; then
      failed=1
    fi
  done

  if [[ "$failed" -ne 0 ]]; then
    echo
    echo "tracked metric regression detected" >&2
    exit 1
  fi
}

compare() {
  local baseline_file="${1:-$TRACKED_BASELINE_FILE}"
  local candidate_env
  candidate_env="$(mktemp)"
  trap "rm -f '$candidate_env'" EXIT

  echo "=== Capturing candidate (HEAD) ==="
  capture "$candidate_env"

  echo ""
  compare_files "$baseline_file" "$candidate_env"
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
      if (($# > 2)); then
        usage >&2
        exit 1
      fi
      compare "${2:-$TRACKED_BASELINE_FILE}"
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
