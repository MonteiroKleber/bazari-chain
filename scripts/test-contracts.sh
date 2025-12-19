#!/bin/bash
# Test all ink! contracts

set -e

CONTRACTS_DIR="/root/bazari-chain/contracts"

echo "Testing ink! contracts..."

for contract in loyalty escrow revenue-split factory; do
  echo "Testing $contract..."
  cd "$CONTRACTS_DIR/$contract"
  cargo +nightly test
  echo "$contract tests passed!"
  echo ""
done

echo "All tests passed!"
