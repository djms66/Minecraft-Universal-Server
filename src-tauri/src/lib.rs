use serde::Serialize;
use std::process::Command;

mod engine;

/// Hardware and operating-system facts used to choose a safe installation path.
#[derive(Serialize)]
struct SystemInfo {
    os: String,
    version: String,
    architecture: String,
    memory_gb: Option<f64>,
    free_disk_gb: Option<f64>,
    disk_gb: Option<f64>,
    board: Option<String>,
    desktop: bool,
}

#[tauri::command]
fn inspect_system() -> SystemInfo {
    let os = std::env::consts::OS.to_string();
    let architecture = std::env::consts::ARCH.to_string();
    let version = os_version(&os);
    let (disk_gb, free_disk_gb) = disk_space(&os);
    let board = raspberry_pi_model();

    SystemInfo {
        os,
        version,
        architecture,
        memory_gb: memory_size(),
        free_disk_gb,
        disk_gb,
        board,
        desktop: desktop_available(),
    }
}

/// Tests an existing SSH setup without writing to the remote computer.
/// Password prompts are disabled; host keys must already be trusted.
#[tauri::command]
fn probe_ssh(host: String, username: String) -> Result<String, String> {
    if !valid_host(&host) || !valid_username(&username) {
        return Err("Vérifie le nom du Raspberry Pi et le nom du compte.".into());
    }

    let destination = format!("{username}@{host}");
    let output = Command::new("ssh")
        .args([
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=7",
            "-o", "StrictHostKeyChecking=yes",
            "-o", "PasswordAuthentication=no",
            &destination,
            "printf 'MONDE_ENSEMBLE_SSH_OK\\n'; cat /sys/firmware/devicetree/base/model 2>/dev/null | tr -d '\\000'; printf '\\n'; awk '/MemTotal/ {printf \"%.1f Go de mémoire\\n\", $2/1048576}' /proc/meminfo 2>/dev/null; df -h --output=size,avail / 2>/dev/null | tail -1; uname -m",
        ])
        .output()
        .map_err(|_| "La commande d’accès sécurisé SSH n’est pas disponible sur cet appareil.".to_string())?;

    if !output.status.success() {
        // Do not surface arbitrary remote stderr in beginner mode. It can contain
        // local paths or account names that are not useful to the user here.
        return Err("Connexion non vérifiée. Vérifie que le Pi est allumé, que son accès SSH est configuré et que sa clé est déjà approuvée sur cet ordinateur.".into());
    }

    let result = String::from_utf8_lossy(&output.stdout);
    if !result.lines().any(|line| line == "MONDE_ENSEMBLE_SSH_OK") {
        return Err(
            "Le Raspberry Pi a répondu, mais l’application n’a pas pu confirmer son identité."
                .into(),
        );
    }
    let facts: Vec<&str> = result
        .lines()
        .filter(|line| *line != "MONDE_ENSEMBLE_SSH_OK" && !line.trim().is_empty())
        .collect();
    let detail = facts.join(" · ");
    Ok(format!(
        "Accès confirmé · {detail}. Aucun fichier n’a été modifié."
    ))
}

#[tauri::command]
async fn discover_pi(username: String) -> Result<Vec<engine::DiscoveredPi>, String> {
    tauri::async_runtime::spawn_blocking(move || engine::discover_pi(&username))
        .await
        .map_err(|error| error.to_string())?
}

fn valid_host(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        && !value.starts_with('-')
}

fn valid_username(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !value.starts_with('-')
}

fn run_text(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn os_version(os: &str) -> String {
    match os {
        "macos" => run_text("sw_vers", &["-productVersion"]).unwrap_or_else(|| "macOS".into()),
        "windows" => run_text("cmd", &["/C", "ver"]).unwrap_or_else(|| "Windows".into()),
        "linux" => std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|contents| {
                contents
                    .lines()
                    .find(|line| line.starts_with("PRETTY_NAME="))
                    .map(|line| {
                        line.trim_start_matches("PRETTY_NAME=")
                            .trim_matches('"')
                            .to_string()
                    })
            })
            .unwrap_or_else(|| "Linux".into()),
        _ => "Version non détectée".into(),
    }
}

fn parse_gib(value: &str) -> Option<f64> {
    let bytes: u64 = value.trim().parse().ok()?;
    Some((bytes as f64 / 1_073_741_824.0 * 10.0).round() / 10.0)
}

fn memory_size() -> Option<f64> {
    match std::env::consts::OS {
        "macos" => parse_gib(&run_text("sysctl", &["-n", "hw.memsize"])?),
        "linux" => {
            let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
            let kb = contents
                .lines()
                .find(|line| line.starts_with("MemTotal:"))?
                .split_whitespace()
                .nth(1)?;
            let bytes = kb.parse::<u64>().ok()?.saturating_mul(1024);
            parse_gib(&bytes.to_string())
        }
        "windows" => {
            let value = run_text(
                "powershell",
                &[
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
                ],
            )?;
            parse_gib(&value)
        }
        _ => None,
    }
}

fn disk_space(os: &str) -> (Option<f64>, Option<f64>) {
    if os == "windows" {
        let command = "(Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='C:'\").Size; (Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='C:'\").FreeSpace";
        if let Some(value) = run_text(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", command],
        ) {
            let mut lines = value.lines();
            return (
                lines.next().and_then(parse_gib),
                lines.next().and_then(parse_gib),
            );
        }
        return (None, None);
    }

    let mount = if os == "macos" { "/" } else { "/" };
    let Some(output) = run_text("df", &["-kP", mount]) else {
        return (None, None);
    };
    let Some(line) = output.lines().last() else {
        return (None, None);
    };
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 5 {
        return (None, None);
    }
    let total_bytes = fields[1]
        .parse::<u64>()
        .ok()
        .map(|v| v.saturating_mul(1024));
    let free_bytes = fields[3]
        .parse::<u64>()
        .ok()
        .map(|v| v.saturating_mul(1024));
    (
        total_bytes.and_then(|v| parse_gib(&v.to_string())),
        free_bytes.and_then(|v| parse_gib(&v.to_string())),
    )
}

fn raspberry_pi_model() -> Option<String> {
    let path = "/sys/firmware/devicetree/base/model";
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes)
        .trim_matches('\0')
        .trim()
        .to_string();
    text.to_lowercase().contains("raspberry pi").then_some(text)
}

fn desktop_available() -> bool {
    match std::env::consts::OS {
        "macos" | "windows" => true,
        "linux" => {
            std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
        }
        _ => false,
    }
}

#[tauri::command]
async fn get_compatibility() -> Result<engine::CompatibilityReport, String> {
    tauri::async_runtime::spawn_blocking(engine::compatibility)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn create_server(
    destination_root: String,
    accept_eula: bool,
    memory_gb: Option<f64>,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<engine::OperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::install_local(
            std::path::Path::new(&destination_root),
            accept_eula,
            memory_gb,
            |step, percent| {
                let _ = progress.send(engine::ProgressMessage {
                    step: step.to_string(),
                    percent,
                });
            },
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn install_pi(
    host: String,
    username: String,
    accept_eula: bool,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<engine::OperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::install_remote_pi(&host, &username, accept_eula, |step, percent| {
            let _ = progress.send(engine::ProgressMessage {
                step: step.to_string(),
                percent,
            });
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn inspect_pi_server(
    host: String,
    username: String,
    server_folder: String,
) -> Result<engine::PiServerStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::inspect_pi_server(&host, &username, &server_folder)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn start_pi_server(
    host: String,
    username: String,
    server_folder: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::start_pi_server(&host, &username, &server_folder)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn backup_server(
    source: String,
    output_file: String,
    server_stopped: bool,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<engine::OperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::create_backup(
            std::path::Path::new(&source),
            std::path::Path::new(&output_file),
            server_stopped,
            |step, percent| {
                let _ = progress.send(engine::ProgressMessage {
                    step: step.to_string(),
                    percent,
                });
            },
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn restore_server(
    archive: String,
    destination_root: String,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<engine::OperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::restore_backup(
            std::path::Path::new(&archive),
            std::path::Path::new(&destination_root),
            |step, percent| {
                let _ = progress.send(engine::ProgressMessage {
                    step: step.to_string(),
                    percent,
                });
            },
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            inspect_system,
            probe_ssh,
            discover_pi,
            get_compatibility,
            create_server,
            install_pi,
            inspect_pi_server,
            start_pi_server,
            backup_server,
            restore_server
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Monde Ensemble");
}
