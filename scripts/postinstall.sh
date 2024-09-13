# Cleanup @myriaddreamin/typst-ts-node-compiler
# it downloads both typst-ts-node-compiler.linux-arm64-musl.node and typst-ts-node-compiler-linux-arm64-gnu.node
# But we only need one of them, so we remove the other one to save space

# Check if we have both files
BASEPATH=$(pwd)
cd node_modules/@myriaddreamin
MUSL_PATH=$(find . -name "typst-ts-node-compiler-linux-arm64-musl")
GNU_PATH=$(find . -name "typst-ts-node-compiler-linux-arm64-gnu")

if [ -z "$MUSL_PATH" ] || [ -z "$GNU_PATH" ]; then
    echo "No musl and gnu files found, skipping ($MUSL_PATH, $GNU_PATH)"
    exit 0
fi

echo "Both musl and gnu files found, removing gnu file"

rm -rf $GNU_PATH

echo "Done"