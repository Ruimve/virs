//! System-level handlers — paper mode, engine status, system metrics.

use axum::{extract::State, http::HeaderMap, Json};
use sysinfo::{Disks, Networks, System};
use virs_error::VirsError;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

pub async fn paper_status(State(state): State<AppState>) -> Result<Json<ApiResponse>, VirsError> {
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "paper_mode": state.engine_manager.paper_mode(),
    }))))
}

pub async fn paper_enable(
    State(state): State<AppState>,
    _headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&_headers)?;
    state.ws_broadcaster.broadcast(serde_json::json!({
        "type": "paper_mode",
        "enabled": true,
    }));
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "paper_mode": true,
        "message": "Paper mode is configured at startup. Restart the server with PAPER_MODE=true to enable.",
    }))))
}

pub async fn paper_disable(
    State(state): State<AppState>,
    _headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&_headers)?;
    state.ws_broadcaster.broadcast(serde_json::json!({
        "type": "paper_mode",
        "enabled": false,
    }));
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "paper_mode": false,
        "message": "Paper mode is configured at startup. Restart the server with PAPER_MODE=false to disable.",
    }))))
}

/// 系统性能信息：CPU、内存、磁盘、网络、运行时长、负载、进程数
pub async fn system_info() -> Result<Json<ApiResponse>, VirsError> {
    // CPU 使用率需要两次刷新之间的差值才能得到真实值
    let mut sys = System::new_all();
    sys.refresh_cpu_all();
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    sys.refresh_cpu_all();

    // 内存、Swap 等即时信息
    sys.refresh_memory();

    let networks = Networks::new_with_refreshed_list();

    // CPU
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
        .unwrap_or_default();
    // CPU 主频（MHz）
    let cpu_frequency_mhz = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);

    // 内存
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let memory_usage = if total_memory > 0 {
        used_memory as f64 / total_memory as f64 * 100.0
    } else {
        0.0
    };

    // 交换分区
    let total_swap = sys.total_swap();
    let used_swap = sys.used_swap();

    // 系统负载（1/5/15 分钟）— 仅 Linux/macOS 有意义，Windows 返回 0
    let load_avg = System::load_average();

    // 进程数
    let process_count = sys.processes().len();

    // 磁盘
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

    // 网络：返回累计字节数和 IP 地址，速率由前端两次采样差值计算
    // 过滤规则：① 排除已知虚拟接口前缀 ② 必须有可用 IP（排除 IPv4 link-local 169.254/16 和 IPv6 fe80::/10）
    //           这样无网线连接的雷雳桥接 enX、无 IP 的 ap1/vmenet0 等都会被过滤掉
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

    // 系统运行时长
    let uptime_secs = System::uptime();

    // 主机名
    let host_name = System::host_name().unwrap_or_default();
    let os_name = System::name().unwrap_or_default();
    let os_version = System::os_version().unwrap_or_default();

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

/// 判断是否为物理网卡或容器主接口。
/// 排除回环、Docker 网桥、veth 虚拟网卡、CNI/flannel 等容器网络虚拟接口，
/// 以及 macOS 上的 AP/vmenet/vlan 等虚拟接口。
fn is_physical_interface(name: &str) -> bool {
    // 排除回环
    if name == "lo" {
        return false;
    }
    // 排除 Docker 相关虚拟接口
    let docker_prefixes = [
        "docker",  // docker0
        "br-",     // br-xxx (docker custom bridge)
        "veth",    // vethxxx (container veth pair)
        "cni",     // cni0, cni-xxx (Kubernetes CNI)
        "flannel", // flannel.1
        "calico",  // calico
        "tunl",    // tunl0 (calico)
        "kube",    // kube-ipvs0
        "virbr",   // libvirt bridge
        "utun",    // macOS utun
        "awdl",    // macOS awdl
        "llw",     // macOS llw
        "anpi",    // macOS anpi
        "bridge",  // bridge0
        "p2p",     // p2p0
        "gif",     // gif0
        "stf",     // stf0
        "ap",      // ap1 (macOS WiFi AP 虚拟接口)
        "vmenet",  // vmenet0 (Parallels/VirtualBox 虚拟机网络)
        "vlan",    // vlan1 (VLAN 虚拟接口)
    ];
    if docker_prefixes.iter().any(|p| name.starts_with(p)) {
        return false;
    }
    true
}

/// 判断接口是否有可用 IP 地址（排除 IPv4 link-local 169.254/16 和 IPv6 fe80::/10）。
/// 用于过滤掉无网线连接的雷雳桥接 enX 等无实际网络的接口。
fn has_usable_ip(data: &sysinfo::NetworkData) -> bool {
    data.ip_networks().iter().any(|ip_net| {
        match ip_net.addr {
            std::net::IpAddr::V4(v4) => {
                let octets = v4.octets();
                // 排除 169.254.0.0/16 (IPv4 link-local)
                !(octets[0] == 169 && octets[1] == 254)
            }
            std::net::IpAddr::V6(v6) => !v6.is_unicast_link_local(),
        }
    })
}
