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


isFileMusl() {
    local f=$1
    if [[ $f == *"libc.musl-"* || $f == *"ld-musl-"* ]]; then
        return 0
    else
        return 1
    fi
}

isMuslFromFilesystem() {
    if [[ -f "/usr/bin/ldd" ]]; then
        if grep -q "musl" "/usr/bin/ldd"; then
            return 0
        else
            return 1
        fi
    else
        return 2
    fi
}

isMuslFromChildProcess() {
    if ldd --version | grep -q "musl"; then
        return 0
    else
        return 1
    fi
}


musl=false
if [[ $(uname -s) == "Linux" ]]; then
    isMuslFromFilesystem
    result=$?
    if [[ $result == 2 ]]; then
        isMuslFromChildProcess
        result=$?
    fi
    if [[ $result == 0 ]]; then
        musl=true
    fi
fi

# Determine which one to remove
if $musl == true; then
    echo "Running on musl"
    REMOVE_PATH=$GNU_PATH
else
    echo "Running on gnu"
    REMOVE_PATH=$MUSL_PATH
fi

rm -rf $REMOVE_PATH

echo "Removed $REMOVE_PATH"
