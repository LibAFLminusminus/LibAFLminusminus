#!/bin/bash
set -e
set -o pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

# Set debug variable to let the target print stderr.
export LIBAFL_FUZZBENCH_DEBUG=1

if [[ ! -x "$QEMU_LAUNCHER" ]]; then
  echo "env variable QEMU_LAUNCHER does not point to a valid executable"
  echo "QEMU_LAUNCHER should point to qemu_launcher"
  exit 1
fi

cd "$SCRIPT_DIR"
make

tests=(
  "overflow"
  "underflow"
  "double_free"
  "memset"
  "uaf"
  "test_limits"
)

tests_verdict=(
  "Replay verdict: Crash (Objective)"
  "Replay verdict: Crash (Objective)"
  "Replay verdict: Crash (Objective)"
  "Replay verdict: Crash (Objective)"
  "Replay verdict: Crash (Objective)"
  "Replay verdict: Ok (Uninteresting)"
)

tests_expected=(
  "Overflow"
  "Underflow"
  "AddressSanitizer Error: Double free"
  "AddressSanitizer Error: Invalid 11 bytes write"
  "Use after free"
  "Test-Limits - No Error"
)

tests_not_expected=(
  "dummy"
  "dummy"
  "dummy"
  "dummy"
  "dummy"
  "AddressSanitizer Error"
)

# We don't want any core dumps. They can potentially be quite large
ulimit -c 0

OUT_FILE=$(mktemp)
trap 'rm -f "$OUT_FILE"' EXIT

for i in "${!tests[@]}"
do
  test="${tests[i]}"
  verdict="${tests_verdict[i]}"
  expected="${tests_expected[i]}"
  not_expected="${tests_not_expected[i]}"

  echo "Running $test detection test..."

  set +e
  "$QEMU_LAUNCHER" \
    replay \
    --input "inputs/$test.txt" \
    --asan-guest \
    -- qasan > "$OUT_FILE" 2>&1
  status=$?
  set -e
  OUT=$(tr -d '\0' < "$OUT_FILE")

  if [[ $status -ne 0 ]]; then
    echo "ERROR: replay exited with $status."
    echo "Output is:"
    echo "$OUT"
    exit 1
  elif ! grep -q -- "$verdict" <<< "$OUT"; then
    echo "ERROR: Expected verdict: $verdict."
    echo "Output is:"
    echo "$OUT"
    exit 1
  elif ! grep -q -- "$expected" <<< "$OUT"; then
    echo "ERROR: Expected: $expected."
    echo "Output is:"
    echo "$OUT"
    exit 1
  elif grep -q -- "$not_expected" <<< "$OUT"; then
    echo "ERROR: Did not expect: $not_expected."
    echo "Output is:"
    echo "$OUT"
    exit 1
  else
    echo "OK."
  fi
done
