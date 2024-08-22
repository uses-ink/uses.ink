#!/bin/bash

BOLD=$(tput bold)
RESET=$(tput sgr0)

check_command() {
    if ! command -v $1 &> /dev/null; then
        echo "$BOLD$1$RESET is required to run this script. Please install $BOLD$1$RESET first."
        exit 1
    fi
}

# Check if redis-cli is installed
check_command redis-cli

# Clear the cache
redis-cli flushall
echo "Cache cleared!"
