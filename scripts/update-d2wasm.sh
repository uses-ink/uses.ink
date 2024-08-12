#!/bin/bash

BOLD=$(tput bold)
RESET=$(tput sgr0)

check_command() {
    if ! command -v $1 &> /dev/null; then
        echo "$BOLD$1$RESET is required to run this script. Please install $BOLD$1$RESET first."
        exit 1
    fi
}

# Check if jq, wget, and curl are installed
check_command jq
check_command wget
check_command curl


default_location="wasm/d2.wasm.br"
download_location=${1:-$default_location}

mkdir -p $(dirname $download_location)

# Fetch the latest release information from GitHub API
response=$(curl -s https://api.github.com/repos/uses-ink/d2wasm/releases/latest)

tag_name=$(echo $response | jq -r '.tag_name')
echo "Latest release: $BOLD$tag_name$RESET"

url=$(echo $response | jq -r '.assets[] | select(.name == "d2.wasm.br") | .browser_download_url')

if [[ -n $url ]]; then
  wget -O $download_location $url -q
  echo "Download completed! Saved to $BOLD$download_location$RESET"
else
  echo "d2.wasm.br not found in the latest release."
fi
