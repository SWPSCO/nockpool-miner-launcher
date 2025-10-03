#!/bin/bash
cd `dirname $0`
CONFIG_FILE="/hive/miners/custom/nockpool/nockpool.conf"
[[ ! -f $CONFIG_FILE ]] && echo -e "${RED}No config file found${NOCOLOR}" && exit 1
TOKEN=$(cat $CONFIG_FILE)
mkdir -p /var/log/miner/nockpool
./nockpool --account-token "$TOKEN" 2>&1 | tee --append /var/log/miner/nockpool/nockpool.log
