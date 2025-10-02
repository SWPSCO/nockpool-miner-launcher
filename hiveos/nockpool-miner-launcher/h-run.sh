#!/usr/bin/env bash

cd `dirname $0`

[ -t 1 ] && . colors

. h-manifest.conf

# Stop any existing instances
[[ `ps aux | grep "./miner-launcher" | grep -v grep | wc -l` != 0 ]] &&
  while killall miner-launcher; do sleep 1; done

# Also stop any running nockpool-miner instances
[[ `ps aux | grep "nockpool-miner" | grep -v grep | wc -l` != 0 ]] &&
  while killall nockpool-miner; do sleep 1; done

# Create log directory if it doesn't exist
CUSTOM_LOG_BASEDIR=`dirname "$CUSTOM_LOG_BASENAME"`
[[ ! -d $CUSTOM_LOG_BASEDIR ]] && mkdir -p $CUSTOM_LOG_BASEDIR

# Get account token from wallet field (CUSTOM_TEMPLATE)
ACCOUNT_TOKEN="${CUSTOM_TEMPLATE}"

# Build command line arguments
MINER_ARGS=""

# Add account token (required)
[[ ! -z $ACCOUNT_TOKEN ]] && MINER_ARGS="--account-token $ACCOUNT_TOKEN"

# Add any additional arguments from extra config
[[ ! -z $CUSTOM_USER_CONFIG ]] && MINER_ARGS="$MINER_ARGS $CUSTOM_USER_CONFIG"

# Run the miner launcher with arguments
# The launcher will download and run the appropriate miner binary
./miner-launcher $MINER_ARGS 2>&1 | tee -a $CUSTOM_LOG_BASENAME.log
