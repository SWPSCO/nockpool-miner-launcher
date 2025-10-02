#!/usr/bin/env bash

#######################
# Functions
#######################

get_cards_hashes(){
  # Get GPU stats from HiveOS
  local gpu_stats=$(gpu-stats)
  local gpu_detect_json=$(gpu-detect)

  # Initialize arrays
  hs=""
  a_temp=""
  a_fan=""
  bus_numbers=""

  # Try to get hashrate and stats from miner API
  # This assumes the miner has an API endpoint that returns JSON stats
  local api_response=$(timeout 5 curl -s http://localhost:${MINER_API_PORT}/stats 2>/dev/null)

  if [[ ! -z "$api_response" && "$api_response" != *"Connection refused"* ]]; then
    # Parse API response if available
    # Expected format: {"hashrate": [val1, val2, ...], "temps": [...], "fans": [...]}
    local gpu_count=$(echo "$api_response" | jq -r '.hashrate | length' 2>/dev/null || echo "0")

    if [[ $gpu_count -gt 0 ]]; then
      # Extract hashrates
      for (( i=0; i < $gpu_count; i++ )); do
        local hash=$(echo "$api_response" | jq -r ".hashrate[$i]" 2>/dev/null || echo "0")
        hs+="$hash "

        local temp=$(echo "$api_response" | jq -r ".temps[$i]" 2>/dev/null || echo "0")
        a_temp+="$temp "

        local fan=$(echo "$api_response" | jq -r ".fans[$i]" 2>/dev/null || echo "0")
        a_fan+="$fan "
      done

      # Get total hashrate
      khs=$(echo "$api_response" | jq -r '.total_hashrate // 0' 2>/dev/null)

      # Get accepted/rejected shares
      ac=$(echo "$api_response" | jq -r '.accepted_shares // 0' 2>/dev/null)
      rj=$(echo "$api_response" | jq -r '.rejected_shares // 0' 2>/dev/null)
    fi
  else
    # Fallback: Try to get stats from GPU monitoring
    local t_temp=$(echo "$gpu_stats" | jq -r '.temp // []')
    local t_fan=$(echo "$gpu_stats" | jq -r '.fan // []')

    # Get number of GPUs
    local nvidia_count=$(gpu-detect NVIDIA 2>/dev/null || echo "0")
    local amd_count=$(gpu-detect AMD 2>/dev/null || echo "0")
    local total_gpus=$((nvidia_count + amd_count))

    if [[ $total_gpus -gt 0 ]]; then
      for (( i=0; i < $total_gpus; i++ )); do
        # Placeholder hashrate - will need to be updated based on actual miner API
        hs+="0 "

        local temp=$(echo "$t_temp" | jq -r ".[$i] // 0")
        a_temp+="$temp "

        local fan=$(echo "$t_fan" | jq -r ".[$i] // 0")
        a_fan+="$fan "
      done
    else
      # CPU mining fallback
      hs="0"
      a_temp=$(cpu-temp)
      a_fan="0"
    fi

    khs=0
    ac=0
    rj=0
  fi
}

get_miner_uptime(){
  local a=0
  if [[ -f $log_name && -f $conf_name ]]; then
    let a=`date +%s`-`stat --format='%Y' $log_name`
  fi
  echo $a
}

get_log_time_diff(){
  local a=0
  if [[ -f $log_name ]]; then
    let a=`date +%s`-`stat --format='%Y' $log_name`
  fi
  echo $a
}

#######################
# MAIN script body
#######################

. /hive/miners/custom/$CUSTOM_NAME/h-manifest.conf

local log_name="$CUSTOM_LOG_BASENAME.log"
local conf_name="$CUSTOM_CONFIG_FILENAME"

khs=0
hs=""
bus_numbers=""
ac=0
rj=0
a_temp=""
a_fan=""

# Calc log freshness
local diffTime=$(get_log_time_diff)
local maxDelay=320

# If log is fresh then calc miner stats or set to null if not
if [ "$diffTime" -lt "$maxDelay" ]; then
  uptime=$(get_miner_uptime)
  get_cards_hashes
  local hs_units='hs'

  # Ensure arrays are not empty
  [[ -z "$hs" ]] && hs="0"
  [[ -z "$a_temp" ]] && a_temp="0"
  [[ -z "$a_fan" ]] && a_fan="0"
  [[ -z "$bus_numbers" ]] && bus_numbers="null"

  # Make JSON
  stats=$(jq -nc \
        --argjson hs "`echo ${hs[@]} | tr " " "\n" | jq -cs '.'`" \
        --arg hs_units "$hs_units" \
        --argjson temp "`echo ${a_temp[@]} | tr " " "\n" | jq -cs '.'`" \
        --argjson fan "`echo ${a_fan[@]} | tr " " "\n" | jq -cs '.'`" \
        --arg ver "$CUSTOM_VERSION" \
        --arg ac "$ac" --arg rj "$rj" \
        --arg algo "nockchain" \
        '{$hs, $hs_units, temp: $temp, fan: $fan, uptime: '$uptime', ar: [$ac, $rj], $ver, $algo}')
else
  stats=""
  khs=0
fi
