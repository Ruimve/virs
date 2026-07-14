use axum::{extract::State, Json};
use sysinfo::{Disks, Networks, System};
use virs_error::VirsError;

use crate::handlers::response::ApiResponse;
use crate::state::AppState;

pub async fn paper_status(State(state): State<AppState>) -> Result<Json<ApiResponse>, VirsError> {
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "paper_mode": state.engine_manager.paper_mode(),
        "restore_error": state.engine_manager.restore_error(),
    }))))
}


pub async fn system_info() -> Result<Json<ApiResponse>, VirsError> {

    let mut sys = System::new_all();
    sys.refresh_cpu_all();
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    sys.refresh_cpu_all();


    sys.refresh_memory();

    let networks = Networks::new_with_refreshed_list();


    let cpu_usage = if !sys.cpus().is_empty() {
        let total: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum();
        total / sys.cpus().len() as f32
    } else {
        0.0
    };
    let cpu_count = sys.cpus().len();
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or(String::new());

    let cpu_frequency_mhz = sys
        .cpus()
        .first()
        .map(|c| c.frequency())
        .unwrap_or(0);


    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let memory_usage = if total_memory > 0 {
        used_memory as f64 / total_memory as f64 * 100.0
    } else {
        0.0
    };


    let total_swap = sys.total_swap();
    let used_swap = sys.used_swap();


    let load_avg = System::load_average();


    let process_count = sys.processes().len();


    let disks_info = Disks::new_with_refreshed_list();
    let disks: Vec<serde_json::Value> = disks_info
        .list()
        .iter()
        .map(|d| {
            let total = d.total_space();
            let available = d.available_space();
            let used = total.saturating_sub(available);
            serde_json::json!({
                "mount_point": d.mount_point().to_string_lossy(),
                "total_bytes": total,
                "used_bytes": used,
                "usage_pct": if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 },
            })
        })
        .collect();


    let net_interfaces: Vec<serde_json::Value> = networks
        .list()
        .iter()
        .filter(|(name, _)| is_physical_interface(name))
        .filter(|(_, data)| has_usable_ip(data))
        .map(|(name, data)| {
            let ips: Vec<String> = data.ip_networks().iter().map(|ip| ip.to_string()).collect();
            serde_json::json!({
                "name": name,
                "total_rx_bytes": data.total_received(),
                "total_tx_bytes": data.total_transmitted(),
                "ips": ips,
            })
        })
        .collect();


    let uptime_secs = System::uptime();


    let host_name = System::host_name().unwrap_or(String::new());
    let os_name = System::name().unwrap_or(String::new());
    let os_version = System::os_version().unwrap_or(String::new());

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "cpu": {
            "usage_pct": cpu_usage,
            "core_count": cpu_count,
            "brand": cpu_brand,
            "frequency_mhz": cpu_frequency_mhz,
        },
        "memory": {
            "total_bytes": total_memory,
            "used_bytes": used_memory,
            "usage_pct": memory_usage,
        },
        "swap": {
            "total_bytes": total_swap,
            "used_bytes": used_swap,
        },
        "load_average": {
            "one": load_avg.one,
            "five": load_avg.five,
            "fifteen": load_avg.fifteen,
        },
        "process_count": process_count,
        "disks": disks,
        "network": net_interfaces,
        "uptime_secs": uptime_secs,
        "host_name": host_name,
        "os_name": os_name,
        "os_version": os_version,
    }))))
}


fn is_physical_interface(name: &str) -> bool {

    if name == "lo" {
        return false;
    }

    let docker_prefixes = [
        "docker",
        "br-",
        "veth",
        "cni",
        "flannel",
        "calico",
        "tunl",
        "kube",
        "virbr",
        "utun",
        "awdl",
        "llw",
        "anpi",
        "bridge",
        "p2p",
        "gif",
        "stf",
        "ap",
        "vmenet",
        "vlan",
    ];
    if docker_prefixes.iter().any(|p| name.starts_with(p)) {
        return false;
    }
    true
}


fn has_usable_ip(data: &sysinfo::NetworkData) -> bool {
    data.ip_networks().iter().any(|ip_net| {
        match ip_net.addr {
            std::net::IpAddr::V4(v4) => {
                let octets = v4.octets();

                !(octets[0] == 169 && octets[1] == 254)
            }
            std::net::IpAddr::V6(v6) => !v6.is_unicast_link_local(),
        }
    })
}
