mod tracer;

use anyhow::Result;
use reqwest::{header::USER_AGENT, Client};
use serde::{Deserialize, Serialize};
use sysinfo::{System, Disks};
use std::env;
use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, Notify};
use tokio::time::{interval, Duration};
use zip::ZipArchive;
use tracing::info;
use directories::ProjectDirs;

#[cfg(target_arch = "x86_64")]
use std::arch::is_x86_feature_detected;

const UPDATE_URL: &str = "https://nockpool.com/api/version";
const UPDATE_INTERVAL: u64 = 15 * 60;

#[derive(Debug, Serialize)]
struct GpuInfo {
    vendor: String,
    model: String,
    vram_mb: u64,
    driver_version: Option<String>,
    compute_capability: Option<String>,
    cuda_cores: Option<u32>,
    architecture: Option<String>,
    power_limit_watts: Option<u32>,
}

#[derive(Debug, Serialize)]
struct CpuInfo {
    model: String,
    vendor: String,
    cores_physical: u32,
    cores_logical: u32,
    base_frequency_mhz: Option<u64>,
    max_frequency_mhz: Option<u64>,
    cache_l1_kb: Option<u64>,
    cache_l2_kb: Option<u64>,
    cache_l3_kb: Option<u64>,
    features: Vec<String>,
    architecture: Option<String>,
}

#[derive(Debug, Serialize)]
struct SystemInfo {
    // Basic system information
    os_name: String,
    os_version: String,
    arch: String,
    kernel_version: Option<String>,
    distribution: Option<String>,
    
    // Hardware information
    cpu: CpuInfo,
    memory_total_mb: u64,
    memory_available_mb: u64,
    memory_type: Option<String>,
    
    // GPU information
    gpus: Vec<GpuInfo>,
    gpu_count: u32,
    
    // Mining configuration
    max_threads: u32,
    thread_affinity: Option<Vec<u32>>,
    mining_algorithm_preference: Option<Vec<String>>,
    
    // System environment
    is_virtualized: bool,
    virtualization_type: Option<String>,
    container_runtime: Option<String>,
    system_uptime_seconds: u64,
    
    // Performance and power
    cpu_governor: Option<String>,
    power_profile: Option<String>,
    thermal_throttling_active: Option<bool>,
    
    // Storage
    available_disk_space_mb: u64,
    storage_type: Option<String>,
    
    // Network
    network_interfaces: Vec<String>,
    
    // Launcher information
    launcher_version: String,
    launcher_config: Option<serde_json::Value>,
    
    // Additional system metrics
    load_average_1min: Option<f64>,
    load_average_5min: Option<f64>,
    load_average_15min: Option<f64>,
    
    // Optional debugging/telemetry info
    previous_miner_version: Option<String>,
    crash_count_24h: Option<u32>,
    uptime_percentage_7d: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ReleaseInfo {
    tag_name: String,
    assets: Vec<Asset>,
    selected_binary: Option<String>,
    system_analysis: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone)]
struct PackageInfo {
    os_name: String,
    arch: String,
    version: String,
    download_url: String,
    bin_name: String,
    package_name: String,
    versions_dir: PathBuf,
    current_symlink: PathBuf,
}

impl PackageInfo {
    pub fn new() -> Result<Self> {
        let (os_name, arch) = Self::get_device_info()?;
        let bin_name = "nockpool-miner".to_string();

        let base_dir = if let Some(proj_dirs) = ProjectDirs::from("com", "swps", "nockpool-miner") {
            proj_dirs.data_dir().to_path_buf()
        } else {
            return Err(anyhow::anyhow!("Could not determine application data directory"));
        };

        let versions_dir = base_dir.join("versions");
        let current_symlink = base_dir.join("current");

        Ok(PackageInfo {
            os_name,
            arch,
            version: String::new(),
            download_url: String::new(),
            bin_name,
            package_name: String::new(),
            versions_dir,
            current_symlink,
        })
    }

    fn get_device_info() -> Result<(String, String)> {
        let os_name = match System::name() {
            Some(os) => {
                if os.to_lowercase().contains("darwin") {
                    "macos".to_string()
                } else {
                    "linux".to_string()
                }
            }
            None => return Err(anyhow::anyhow!("Failed to get OS name")),
        };

        let arch = match System::cpu_arch() {
            Some(arch) => {
                if arch == "aarch64" || arch == "arm64" {
                    "aarch64".to_string()
                } else {
                    "x86_64".to_string()
                }
            }
            None => return Err(anyhow::anyhow!("Failed to get CPU architecture")),
        };

        Ok((os_name, arch))
    }

    fn collect_system_info() -> Result<SystemInfo> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let (os_name, arch) = Self::get_device_info()?;
        
        // Basic system information
        let os_version = System::long_os_version().unwrap_or_else(|| "Unknown".to_string());
        let kernel_version = System::kernel_version();
        let distribution = System::distribution_id();
        
        // CPU information
        let cpus = sys.cpus();
        let physical_cores = sys.physical_core_count().unwrap_or(0) as u32;
        let logical_cores = num_cpus::get() as u32;
        
        let cpu_info = if let Some(cpu) = cpus.first() {
            CpuInfo {
                model: cpu.brand().to_string(),
                vendor: cpu.vendor_id().to_string(),
                cores_physical: physical_cores,
                cores_logical: logical_cores,
                base_frequency_mhz: Some(cpu.frequency()),
                max_frequency_mhz: None, // Not easily available
                cache_l1_kb: None,
                cache_l2_kb: None,
                cache_l3_kb: None,
                features: Self::get_cpu_features(),
                architecture: Some(arch.clone()),
            }
        } else {
            return Err(anyhow::anyhow!("No CPU information available"));
        };
        
        // Memory information
        let memory_total_mb = sys.total_memory() / 1024 / 1024;
        let memory_available_mb = sys.available_memory() / 1024 / 1024;
        
        // GPU information (placeholder for now - would need platform-specific code)
        let gpus = Self::collect_gpu_info();
        let gpu_count = gpus.len() as u32;
        
        // Mining configuration
        let max_threads = std::cmp::max(1, logical_cores.saturating_sub(2)); // Leave 2 cores for system
        
        // System environment
        let is_virtualized = Self::detect_virtualization();
        let virtualization_type = if is_virtualized { 
            Self::get_virtualization_type() 
        } else { 
            None 
        };
        
        // Storage information
        let available_disk_space_mb = Self::get_available_disk_space();
        let storage_type = Self::detect_storage_type();
        
        // Network interfaces
        let network_interfaces = Self::get_network_interfaces();
        
        // System metrics
        let load_averages = System::load_average();
        
        Ok(SystemInfo {
            // Basic system information
            os_name,
            os_version,
            arch,
            kernel_version,
            distribution: Some(distribution),
            
            // Hardware information
            cpu: cpu_info,
            memory_total_mb,
            memory_available_mb,
            memory_type: None, // Would need platform-specific code
            
            // GPU information
            gpus,
            gpu_count,
            
            // Mining configuration
            max_threads,
            thread_affinity: None,
            mining_algorithm_preference: None,
            
            // System environment
            is_virtualized,
            virtualization_type,
            container_runtime: Self::detect_container_runtime(),
            system_uptime_seconds: System::uptime(),
            
            // Performance and power
            cpu_governor: Self::get_cpu_governor(),
            power_profile: None, // Platform-specific
            thermal_throttling_active: None,
            
            // Storage
            available_disk_space_mb,
            storage_type,
            
            // Network
            network_interfaces,
            
            // Launcher information
            launcher_version: env!("CARGO_PKG_VERSION").to_string(),
            launcher_config: None,
            
            // Additional system metrics
            load_average_1min: Some(load_averages.one),
            load_average_5min: Some(load_averages.five),
            load_average_15min: Some(load_averages.fifteen),
            
            // Optional debugging info
            previous_miner_version: None,
            crash_count_24h: None,
            uptime_percentage_7d: None,
        })
    }

    fn get_cpu_features() -> Vec<String> {
        let mut features = Vec::new();
        
        // On x86/x64, we can check for common features
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") { features.push("avx2".to_string()); }
            if is_x86_feature_detected!("aes") { features.push("aes".to_string()); }
            if is_x86_feature_detected!("sse4.2") { features.push("sse4_2".to_string()); }
            if is_x86_feature_detected!("fma") { features.push("fma3".to_string()); }
            if is_x86_feature_detected!("sha") { features.push("sha".to_string()); }
        }
        
        #[cfg(target_arch = "aarch64")]
        {
            features.push("neon".to_string());
            // Add other ARM-specific features as needed
        }
        
        features
    }

    fn collect_gpu_info() -> Vec<GpuInfo> {
        let mut gpus = Vec::new();
        
        // Only detect GPUs on Linux
        #[cfg(target_os = "linux")]
        {
            // Try to detect NVIDIA GPUs
            gpus.extend(Self::detect_nvidia_gpus());
            
            // Try to detect AMD GPUs
            gpus.extend(Self::detect_amd_gpus());
            
            // Try to detect Intel GPUs
            gpus.extend(Self::detect_intel_gpus());
        }
        
        gpus
    }

    #[cfg(target_os = "linux")]
    fn detect_nvidia_gpus() -> Vec<GpuInfo> {
        let mut gpus = Vec::new();
        
        // Check if nvidia-smi is available and working
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args(&["--query-gpu=name,memory.total,driver_version", "--format=csv,noheader,nounits"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split(", ").collect();
                    if parts.len() >= 3 {
                        let name = parts[0].trim().to_string();
                        let vram_mb = parts[1].trim().parse::<u64>().unwrap_or(0);
                        let driver_version = Some(parts[2].trim().to_string());
                        
                        gpus.push(GpuInfo {
                            vendor: "nvidia".to_string(),
                            model: name,
                            vram_mb,
                            driver_version,
                            compute_capability: None, // Could be queried separately
                            cuda_cores: None,
                            architecture: None,
                            power_limit_watts: None,
                        });
                    }
                }
            }
        }
        
        // Fallback: check /proc/driver/nvidia/version
        if gpus.is_empty() {
            if let Ok(contents) = fs::read_to_string("/proc/driver/nvidia/version") {
                if contents.contains("NVIDIA") {
                    // Basic NVIDIA GPU detected, but we can't get detailed info
                    gpus.push(GpuInfo {
                        vendor: "nvidia".to_string(),
                        model: "NVIDIA GPU (details unavailable)".to_string(),
                        vram_mb: 0,
                        driver_version: None,
                        compute_capability: None,
                        cuda_cores: None,
                        architecture: None,
                        power_limit_watts: None,
                    });
                }
            }
        }
        
        gpus
    }

    #[cfg(target_os = "linux")]
    fn detect_amd_gpus() -> Vec<GpuInfo> {
        let mut gpus = Vec::new();
        
        // Check lspci for AMD GPUs
        if let Ok(output) = std::process::Command::new("lspci")
            .args(&["-nn"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let line_lower = line.to_lowercase();
                    if (line_lower.contains("amd") || line_lower.contains("ati")) && 
                       (line_lower.contains("vga") || line_lower.contains("display") || line_lower.contains("3d")) {
                        
                        // Extract GPU name from lspci output
                        let parts: Vec<&str> = line.split(": ").collect();
                        let model = if parts.len() > 1 {
                            parts[1].to_string()
                        } else {
                            "AMD GPU".to_string()
                        };
                        
                        gpus.push(GpuInfo {
                            vendor: "amd".to_string(),
                            model,
                            vram_mb: 0, // Would need additional detection
                            driver_version: None,
                            compute_capability: None,
                            cuda_cores: None,
                            architecture: None,
                            power_limit_watts: None,
                        });
                    }
                }
            }
        }
        
        gpus
    }

    #[cfg(target_os = "linux")]
    fn detect_intel_gpus() -> Vec<GpuInfo> {
        let mut gpus = Vec::new();
        
        // Check lspci for Intel GPUs
        if let Ok(output) = std::process::Command::new("lspci")
            .args(&["-nn"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let line_lower = line.to_lowercase();
                    if line_lower.contains("intel") && 
                       (line_lower.contains("vga") || line_lower.contains("display") || line_lower.contains("graphics")) {
                        
                        // Extract GPU name from lspci output
                        let parts: Vec<&str> = line.split(": ").collect();
                        let model = if parts.len() > 1 {
                            parts[1].to_string()
                        } else {
                            "Intel GPU".to_string()
                        };
                        
                        gpus.push(GpuInfo {
                            vendor: "intel".to_string(),
                            model,
                            vram_mb: 0, // Intel GPUs typically use system RAM
                            driver_version: None,
                            compute_capability: None,
                            cuda_cores: None,
                            architecture: None,
                            power_limit_watts: None,
                        });
                    }
                }
            }
        }
        
        gpus
    }

    fn detect_virtualization() -> bool {
        // Check common virtualization indicators
        if let Ok(contents) = fs::read_to_string("/proc/cpuinfo") {
            if contents.contains("hypervisor") {
                return true;
            }
        }
        
        // Check for container environment
        if fs::metadata("/.dockerenv").is_ok() {
            return true;
        }
        
        // Check DMI/SMBIOS
        if let Ok(contents) = fs::read_to_string("/sys/class/dmi/id/sys_vendor") {
            let vendor = contents.trim().to_lowercase();
            if vendor.contains("vmware") || vendor.contains("virtualbox") || vendor.contains("qemu") {
                return true;
            }
        }
        
        false
    }

    fn get_virtualization_type() -> Option<String> {
        if fs::metadata("/.dockerenv").is_ok() {
            return Some("docker".to_string());
        }
        
        if let Ok(contents) = fs::read_to_string("/sys/class/dmi/id/sys_vendor") {
            let vendor = contents.trim().to_lowercase();
            if vendor.contains("vmware") {
                return Some("vmware".to_string());
            } else if vendor.contains("virtualbox") {
                return Some("virtualbox".to_string());
            } else if vendor.contains("qemu") {
                return Some("kvm".to_string());
            }
        }
        
        None
    }

    fn detect_container_runtime() -> Option<String> {
        if fs::metadata("/.dockerenv").is_ok() {
            return Some("docker".to_string());
        }
        
        // Check for Podman
        if env::var("container").as_deref() == Ok("podman") {
            return Some("podman".to_string());
        }
        
        None
    }

    fn get_available_disk_space() -> u64 {
        let disks = Disks::new_with_refreshed_list();
        
        // Get available space for root filesystem
        for disk in &disks {
            if disk.mount_point() == std::path::Path::new("/") {
                return disk.available_space() / 1024 / 1024; // Convert to MB
            }
        }
        
        // Fallback: sum all available space
        disks.iter()
            .map(|disk| disk.available_space())
            .sum::<u64>() / 1024 / 1024
    }

    fn detect_storage_type() -> Option<String> {
        // This is a simplified detection - real implementation would check block device info
        if let Ok(contents) = fs::read_to_string("/proc/mounts") {
            if contents.contains("nvme") {
                return Some("nvme".to_string());
            } else if contents.contains("ssd") {
                return Some("ssd".to_string());
            }
        }
        
        Some("unknown".to_string())
    }

    fn get_network_interfaces() -> Vec<String> {
        let mut interfaces = Vec::new();
        
        // Simple interface detection
        if let Ok(entries) = fs::read_dir("/sys/class/net") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("eth") || name.starts_with("en") {
                    interfaces.push("ethernet".to_string());
                } else if name.starts_with("wlan") || name.starts_with("wl") {
                    interfaces.push("wifi".to_string());
                }
            }
        }
        
        if interfaces.is_empty() {
            interfaces.push("unknown".to_string());
        }
        
        // Remove duplicates
        interfaces.sort();
        interfaces.dedup();
        interfaces
    }

    fn get_cpu_governor() -> Option<String> {
        // Linux-specific
        if let Ok(governor) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor") {
            return Some(governor.trim().to_string());
        }
        
        None
    }

    pub async fn fetch_latest(&mut self) -> Result<()> {
        let client = Client::new();
        
        // First try the enhanced endpoint with system information
        let system_info = Self::collect_system_info()?;
        
        // Sending system information to endpoint for binary selection
        let enhanced_response = client
            .post(UPDATE_URL)
            .header(USER_AGENT, "miner-launcher")
            .json(&system_info)
            .send()
            .await;

        let release_info = match enhanced_response {
            Ok(res) if res.status().is_success() => {
                res.json::<ReleaseInfo>().await?
            }
            Ok(_res) => {
                return Err(anyhow::anyhow!(
                    "This system isn't supported with the launcher. Please build from source."
                ));
            }
            Err(_e) => {
                return Err(anyhow::anyhow!(
                    "This system isn't supported with the launcher. Please build from source."
                ));
            }
        };

        // System analysis logging removed for cleaner output

        // Use selected_binary if provided, otherwise system not supported
        if let Some(_selected_binary) = &release_info.selected_binary {
            
            // Extract version from tag_name
            self.version = release_info.tag_name
                .split('-')
                .next()
                .unwrap_or(&release_info.tag_name)
                .replace("v", "");
            
            // Find the appropriate asset
            for asset in &release_info.assets {
                // Check if this asset matches our system (simplified matching)
                if self.is_compatible_asset(&asset.name, _selected_binary) {
                    self.download_url = asset.browser_download_url.clone();
                    self.package_name = asset.name.clone();
                    return Ok(());
                }
            }
        }

        Err(anyhow::anyhow!(
            "This system isn't supported with the launcher. Please build from source."
        ))
    }

    fn is_compatible_asset(&self, asset_name: &str, _selected_binary: &str) -> bool {
        // Simple compatibility check - in a real implementation, this would be more sophisticated
        let asset_lower = asset_name.to_lowercase();
        
        // Check for basic OS and architecture compatibility
        let os_match = asset_lower.contains(&self.os_name);
        let arch_match = asset_lower.contains(&self.arch);
        
        os_match && arch_match
    }

    pub fn get_local_version(&self) -> Option<String> {
        if self.current_symlink.exists() {
            let real_path = fs::read_link(&self.current_symlink).ok()?;
            let version = real_path.file_name()?.to_str()?.to_string();
            Some(version)
        } else {
            None
        }
    }

    pub async fn ensure_latest_version(&mut self) -> Result<()> {
        let local_version = self.get_local_version();
        self.fetch_latest().await?;

        let needs_update = match local_version {
            Some(lv) => lv != self.version,
            None => true,
        };

        if needs_update {
            info!("New version {} is available. Downloading...", self.version);
            self.download_and_install().await?;
            self.update_symlink()?;
        } else {
            info!("You are on the latest version.");
        }
        Ok(())
    }

    async fn download_and_install(&self) -> Result<()> {
        let response = reqwest::get(&self.download_url).await?;
        let bytes = response.bytes().await?;

        let version_dir = self.versions_dir.join(&self.version);
        fs::create_dir_all(&version_dir)?;

        let bin_path = version_dir.join(&self.bin_name);

        if self.os_name == "macos" {
            let mut archive = ZipArchive::new(Cursor::new(bytes))?;
            archive.extract(&version_dir)?;
        } else {
            let mut file = fs::File::create(&bin_path)?;
            file.write_all(&bytes)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
        }

        Ok(())
    }

    fn update_symlink(&self) -> Result<()> {
        let version_dir = self.versions_dir.join(&self.version);

        if self.current_symlink.exists() {
            fs::remove_file(&self.current_symlink)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(version_dir, &self.current_symlink)?;

        Ok(())
    }

    pub fn run_miner(&self, args: &[String]) -> Result<Child> {
        let bin_path = self.current_symlink.join(&self.bin_name);
        let child = Command::new(bin_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Ok(child)
    }

    pub fn kill_miner(&self, child: &mut Child) -> Result<()> {
        child.start_kill()?;
        Ok(())
    }

    pub fn start_update_watcher(
        package_info: Arc<Mutex<PackageInfo>>,
        update_notifier: Arc<Notify>,
    ) {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(UPDATE_INTERVAL));
            loop {
                interval.tick().await;
                info!("Checking for updates...");

                let mut pi = package_info.lock().await;
                let local_version = pi.get_local_version();

                if let Err(e) = pi.fetch_latest().await {
                    info!("Failed to check for updates: {}", e);
                    continue;
                }

                let needs_update = match local_version {
                    Some(lv) => lv != pi.version,
                    None => true,
                };

                if needs_update {
                    info!("Update found in background, preparing update...");
                    if let Err(e) = pi.download_and_install().await {
                        info!("Failed to download update: {}", e);
                        continue;
                    }
                    if let Err(e) = pi.update_symlink() {
                        info!("Failed to update symlink: {}", e);
                        continue;
                    }
                    update_notifier.notify_one();
                } else {
                    info!("Already on the latest version.");
                }
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracer::init();

    let mut disable_update_loop = false;
    let mut no_update = false;
    let mut miner_args = Vec::new();

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--disable-update-loop" => disable_update_loop = true,
            "--no-update" => no_update = true,
            _ => miner_args.push(arg),
        }
    }

    let package_info = PackageInfo::new()?;
    let package_info = Arc::new(Mutex::new(package_info));

    if no_update {
        let pi = package_info.lock().await;
        if pi.get_local_version().is_none() {
            return Err(anyhow::anyhow!(
                "No current version installed. Please run without --no-update first."
            ));
        }
    } else {
        let mut pi = package_info.lock().await;
        pi.ensure_latest_version().await?;
    }

    let restart_notifier = Arc::new(Notify::new());
    let update_notifier = Arc::new(Notify::new());

    if !disable_update_loop {
        PackageInfo::start_update_watcher(package_info.clone(), update_notifier.clone());
    }

    loop {
        let mut child = {
            let pi = package_info.lock().await;
            pi.run_miner(&miner_args)?
        };

        let stdout = child
            .stdout
            .take()
            .expect("child stdout was not configured to a pipe");

        let stderr = child
            .stderr
            .take()
            .expect("child stderr was not configured to a pipe");

        let restart_notifier_stdout = restart_notifier.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if line.contains("restart-miner-now") {
                    info!("Restart signal received from stdout, restarting miner...");
                    restart_notifier_stdout.notify_one();
                    break;
                }
                eprintln!("{}", line);
            }
        });

        let restart_notifier_stderr = restart_notifier.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if line.contains("restart-miner-now") {
                    info!("Restart signal received from stderr, restarting miner...");
                    restart_notifier_stderr.notify_one();
                    break;
                }
                eprintln!("{}", line);
            }
        });

        let child = Arc::new(Mutex::new(child));

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Ctrl-C received, shutting down miner...");
                let mut child_lock = child.lock().await;
                let pi = package_info.lock().await;
                pi.kill_miner(&mut child_lock)?;
                info!("Miner shut down.");
                break;
            }
            _ = restart_notifier.notified() => {
                info!("Restarting miner due to output signal...");
                let mut child_lock = child.lock().await;
                let pi = package_info.lock().await;
                let _ = pi.kill_miner(&mut child_lock);
                continue;
            }
            _ = update_notifier.notified() => {
                info!("Restarting miner due to update...");
                let mut child_lock = child.lock().await;
                let pi = package_info.lock().await;
                let _ = pi.kill_miner(&mut child_lock);
                continue;
            }
            res = async {
                let mut child_guard = child.lock().await;
                child_guard.wait().await
            } => {
                info!("Miner exited with status: {:?}. Restarting...", res);
                continue;
            }
        }
    }

    Ok(())
}
