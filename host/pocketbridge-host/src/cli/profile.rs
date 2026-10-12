use anyhow::{Context, Result};
use pocketbridge_core::{
    collect_host_profile, profiler::build_worker_profile_from_props, AdbTransport, DeviceProfile,
};

pub async fn run(_port: u16) -> Result<()> {
    println!("PocketBridge Device Profiler");
    println!("==========================================================================");

    // 1. Host Profile
    println!("\n[Host Profile - Arch Linux]");
    let host = collect_host_profile().await
        .context("Failed to collect host profile")?;
    print_profile(&host);

    // 2. Worker Profile via ADB
    println!("\n[Worker Profile - Android Device via ADB]");
    let transport = AdbTransport::default();
    match transport.check_device().await {
        Ok(device_str) => {
            println!("Attached ADB device: {device_str}");
            match transport.query_getprop().await {
                Ok(props) => {
                    let worker = build_worker_profile_from_props(&props);
                    print_profile(&worker);
                }
                Err(e) => eprintln!("Failed to query device properties via adb: {e}"),
            }
        }
        Err(e) => {
            println!("No ADB worker device currently attached: {e}");
        }
    }

    Ok(())
}

fn print_profile(profile: &DeviceProfile) {
    println!("  Role:             {}", profile.role);
    if let Some(h) = &profile.hostname {
        println!("  Hostname:         {}", h);
    }
    println!("  OS:               {} ({})", profile.os, profile.os_version);
    println!("  Architecture:     {}", profile.architecture);
    println!("  CPU Model:        {}", profile.cpu_model);
    println!("  CPU Cores:        {}", profile.cpu_cores);
    if profile.ram_total_bytes > 0 {
        println!("  RAM Total:        {}", DeviceProfile::format_ram(profile.ram_total_bytes));
    }
    if let Some(avail) = profile.ram_available_bytes {
        println!("  RAM Available:    {}", DeviceProfile::format_ram(avail));
    }
    if let Some(gpu) = &profile.gpu {
        println!("  GPU:              {}", gpu);
    }
    if let Some(dev) = &profile.device_model {
        println!("  Device Model:     {}", dev);
    }
    if let Some(soc) = &profile.soc {
        println!("  SoC:              {}", soc);
    }
    if let Some(api) = profile.android_api_level {
        println!("  Android API:      {}", api);
    }
    if let Some(bat) = profile.battery_percent {
        println!("  Battery:          {}% (charging: {:?})", bat, profile.battery_charging);
    }
    if let Some(therm) = &profile.thermal_status {
        println!("  Thermal Status:   {}", therm);
    }
}
