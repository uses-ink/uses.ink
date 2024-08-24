# Cleanup @myriaddreamin/typst-ts-node-compiler
# it downloads both typst-ts-node-compiler.linux-arm64-musl.node and typst-ts-node-compiler-linux-arm64-gnu.node
# But we only need one of them, so we remove the other one to save space

# Check if we have both files
if [ -f node_modules/@myriaddreamin/typst-ts-node-compiler/typst-ts-node-compiler.linux-arm64-musl.node ] && [ -f node_modules/@myriaddreamin/typst-ts-node-compiler/typst-ts-node-compiler-linux-arm64-gnu.node ]; then
    echo "Both typst-ts-node-compiler.linux-arm64-musl.node and typst-ts-node-compiler-linux-arm64-gnu.node found"
else
    echo "Both typst-ts-node-compiler.linux-arm64-musl.node and typst-ts-node-compiler-linux-arm64-gnu.node not found"
    exit 0
fi

rm -rf node_modules/@myriaddreamin/typst-ts-node-compiler-linux-arm64-gnu

echo "Done"