#!/bin/bash
[[ -z $CUSTOM_TEMPLATE ]] && echo -e "${YELLOW}CUSTOM_TEMPLATE is empty${NOCOLOR}" && return 1
mkdir -p /hive/miners/custom/nockpool
echo "$CUSTOM_TEMPLATE" > /hive/miners/custom/nockpool/nockpool.conf
echo "Config created with token: $CUSTOM_TEMPLATE"
