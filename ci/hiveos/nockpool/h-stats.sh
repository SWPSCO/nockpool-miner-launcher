#!/bin/bash
log_file="/var/log/miner/nockpool/nockpool.log"
if [[ ! -f $log_file ]] || [[ ! -s $log_file ]]; then
    khs=0
    stats="null"
else
    proofrate_line=$(tac $log_file 2>/dev/null | grep -m1 "Current mining rate:")
    if [[ -n $proofrate_line ]]; then
        proofs=$(echo "$proofrate_line" | sed -n 's/.*Current mining rate: \([0-9.]*\) proofs\/sec.*/\1/p')
        [[ -z $proofs ]] && proofs=0
    else
        proofs=0
    fi 
    khs=$proofs  
    stats=$(jq -n \
        --argjson hs "[$proofs]" \
        --arg hs_units "proofs/s" \
        --argjson temp "[]" \
        --argjson fan "[]" \
        --arg ver "0.3.2" \
        --arg algo "nockchain" \
        '{$hs, $hs_units, $temp, $fan, $ver, $algo}')
fi
