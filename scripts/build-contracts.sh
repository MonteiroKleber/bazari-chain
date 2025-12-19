#!/bin/bash
# Build all ink! contracts

set -e

CONTRACTS_DIR="/root/bazari-chain/contracts"

echo "Building ink! contracts..."

for contract in loyalty escrow revenue-split factory; do
  echo "Building $contract..."
  cd "$CONTRACTS_DIR/$contract"
  cargo +nightly contract build --release
  echo "$contract built successfully!"
done

echo ""
echo "All contracts built successfully!"
echo ""
echo "Artifacts can be found in:"
for contract in loyalty escrow revenue-split factory; do
  echo "  $CONTRACTS_DIR/$contract/target/ink/"
done
