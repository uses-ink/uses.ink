#!/bin/bash

# Cleanup @myriaddreamin/typst-ts-node-compiler
# it downloads both typst-ts-node-compiler.linux-arm64-musl.node and typst-ts-node-compiler-linux-arm64-gnu.node
# it can also download x64 versions
# But we only need one of them, so we remove the other one to save space

# Check if we have both files
BASEPATH=$(pwd)
cd node_modules/@myriaddreamin
MUSL_PATH=$(find . -regex '.*typst-ts-node-compiler.*musl.*')
GNU_PATH=$(find . -regex '.*typst-ts-node-compiler.*gnu.*')

if [ -z "$MUSL_PATH" ] || [ -z "$GNU_PATH" ]; then
    echo "No musl and gnu files found, skipping ($MUSL_PATH, $GNU_PATH)"
    exit 0
fi

# Determine which one to remove
# Are we running on musl?
if ldd --version | grep -q musl; then
    echo "Running on musl"
    REMOVE_PATH=$GNU_PATH
else
    echo "Running on gnu"
    REMOVE_PATH=$MUSL_PATH
fi

rm -rf $REMOVE_PATH

echo "Removed $REMOVE_PATH"