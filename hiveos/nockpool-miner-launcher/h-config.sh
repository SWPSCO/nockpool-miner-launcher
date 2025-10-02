#!/usr/bin/env bash

mkfile_from_symlink $CUSTOM_CONFIG_FILENAME

# Configuration is passed via command-line arguments to miner-launcher
# CUSTOM_TEMPLATE contains the account token (wallet field in HiveOS)
# CUSTOM_USER_CONFIG can contain additional command-line arguments

# Create a minimal config file for HiveOS compatibility
cat > $CUSTOM_CONFIG_FILENAME << EOF
# NockPool Miner Launcher Configuration
# Configuration is passed via command-line arguments

# Account token (from wallet field): ${CUSTOM_TEMPLATE:-not_set}
# Additional args (from extra config): ${CUSTOM_USER_CONFIG:-none}

EOF
