# NockPool Miner Launcher - HiveOS Flight Sheet

This package enables NockPool mining on HiveOS using the official NockPool Miner Launcher.

## Installation

1. Copy this entire directory to your HiveOS rig at:
   ```
   /hive/miners/custom/nockpool-miner-launcher/
   ```

2. Create a new flight sheet in HiveOS:
   - Miner: Custom Miner
   - Installation URL: Point to your hosted package or local path
   - Miner name: `nockpool-miner-launcher`

## Configuration

When setting up the flight sheet in HiveOS, configure the following fields:

### Wallet (Required)
Enter your **account token** in the "Wallet" field. This will be passed to the miner launcher as `--account-token YOUR_TOKEN`

### Pool URL
Not currently used, but can be left empty or set for future use

### Password
Not currently used, but can be left as default (x) or set for future use

### Extra Config Arguments
You can add additional command-line arguments in the "Extra Config Arguments" field:
- Example: `--verbose` or other miner-specific flags
- These will be appended to the miner launcher command

## Files in this Package

- `h-manifest.conf` - Package metadata and configuration
- `h-config.sh` - Configuration script that generates miner config
- `h-run.sh` - Script that runs the miner launcher
- `h-stats.sh` - Script that reports statistics to HiveOS
- `miner-launcher` - The NockPool miner launcher binary

## How it Works

1. HiveOS calls `h-config.sh` to prepare the miner (creates minimal config for tracking)
2. HiveOS then calls `h-run.sh` to start the miner with `--account-token` from the wallet field
3. The launcher downloads the appropriate miner binary based on your system
4. The miner starts with your account token
5. Periodically, HiveOS calls `h-stats.sh` to get mining statistics

## Statistics

The stats script attempts to gather statistics from the miner's API (if available) or falls back to system monitoring. You may need to adjust the API port in `h-manifest.conf` if the miner uses a different port.

## Troubleshooting

### Logs
Check miner logs at:
```
/var/log/miner/nockpool-miner-launcher/nockpool-miner-launcher.log
```

### No Statistics Showing
If statistics are not showing in HiveOS:
1. Verify the miner is running: `screen -r`
2. Check if the miner has an API running on port 8080
3. Update `MINER_API_PORT` in `h-manifest.conf` if needed
4. Review `h-stats.sh` and adjust the stats gathering logic

### Miner Not Starting
1. Check if the binary has execute permissions: `chmod +x miner-launcher`
2. Verify configuration file was created: `cat /hive/miners/custom/nockpool-miner-launcher/nockpool.conf`
3. Check system logs: `journalctl -xe`

## Updates

To update the miner:
1. Build or download the latest `miner-launcher` binary
2. Replace the `miner-launcher` file in this directory
3. Restart the miner in HiveOS

## Notes

- The launcher automatically downloads and updates the actual miner binary
- Make sure your rig has internet connectivity for the first run
- The downloaded miner is stored in the user's home directory
- Statistics reporting may need customization based on the actual miner's API format

## Support

For issues specific to:
- HiveOS integration: Check HiveOS forums or this README
- Miner issues: See the main [NockPool Miner](https://github.com/SWPSCO/nockpool-miner) repository
- Launcher issues: See the [NockPool Launcher](https://github.com/SWPSCO/nockpool-miner-launcher) repository
