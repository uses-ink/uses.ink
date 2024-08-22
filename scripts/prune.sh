#!/usr/bin/env bash

# Prune all directories in pnpm-workspace.yaml to only keep the packages.json files
# This is useful to build a minimal Docker image
# Output the pruned directories to pruned/ directory

BOLD=$(tput bold)
RESET=$(tput sgr0)

check_tools() {
  for TOOL in $@; do
    if ! command -v $TOOL &> /dev/null; then
      echo "$BOLD$TOOL$RESET is required to run this script"
      exit 1
    fi
  done
}

set -e

check_tools git wget jq

# Get the root directory of the project
ROOT_DIR=$(git rev-parse --show-toplevel)

echo "Pruning the directories in $BOLD$ROOT_DIR$RESET"


# Get the list of directories to prune
DIRECTORIES=$(cat $ROOT_DIR/package.json | jq '.workspaces[]')

# Resolve the globs
DIRECTORIES=$(echo $DIRECTORIES | xargs -n1)

echo "Pruning the following directories:"
echo $DIRECTORIES

# Create the pruned directory
mkdir -p $ROOT_DIR/pruned

# Prune the directories
for DIR in $DIRECTORIES; do
  DIR=$(echo $DIR | tr -d '"')
  echo "Pruning $DIR"
  mkdir -p $ROOT_DIR/pruned/$DIR
  cp $ROOT_DIR/$DIR/package.json $ROOT_DIR/pruned/$DIR/package.json
done

