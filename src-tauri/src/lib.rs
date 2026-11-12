use serde::Serialize;
use std::process::Command;
use tauri::Manager;

mod engine;
mod pi_imager;

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

/// A storage device that can be considered for a future Pi OS installation.
/// Discovery is read-only; no command in this flow writes to the device.
#[derive(Serialize)]
struct PiBootTarget {
    id: String,
    name: String,
    size: String,
    size_bytes: u64,
    connection: String,
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

#[tauri::command]
async fn discover_new_pi(expected_hostname: String) -> Result<Vec<engine::DiscoveredPi>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::discover_new_pi("minecraft", &expected_hostname)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Lists likely removable SD/USB media while excluding the host's system disk.
/// The actual imaging step must repeat these safety checks immediately before
/// it writes, and must keep Raspberry Pi Imager's system-drive protection on.
#[tauri::command]
fn discover_pi_boot_targets() -> Result<Vec<PiBootTarget>, String> {
    match std::env::consts::OS {
        "macos" => discover_macos_boot_targets(),
        "windows" => discover_windows_boot_targets(),
        "linux" => discover_linux_boot_targets(),
        _ => Err(
            "La recherche des cartes SD et disques externes n’est pas disponible sur ce système."
                .into(),
        ),
    }
}

#[tauri::command]
async fn prepare_pi_sd(
    app: tauri::AppHandle,
    target_id: String,
    target_name: String,
    target_size_bytes: u64,
    image_url: String,
    image_sha256: String,
    wifi_ssid: String,
    wifi_password: String,
    wifi_country: String,
    confirm_erase: bool,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<pi_imager::PiFlashResult, String> {
    if !confirm_erase {
        return Err("Confirme l’effacement du support sélectionné pour continuer. Aucun support n’a été modifié.".into());
    }
    let app_data = app.path().app_data_dir().map_err(|error| {
        format!("Le dossier sécurisé de l’application est inaccessible : {error}")
    })?;
    tauri::async_runtime::spawn_blocking(move || {
        let current_recommendation = engine::pi_os_recommendation()?;
        if image_url != current_recommendation.image_url
            || image_sha256 != current_recommendation.download_sha256
        {
            return Err("Le catalogue Raspberry Pi a changé depuis l’affichage. Actualise la recommandation avant d’écrire la carte.".into());
        }
        let targets = discover_pi_boot_targets()?;
        let target = targets
            .into_iter()
            .find(|target| {
                target.id == target_id
                    && target.name == target_name
                    && target.size_bytes == target_size_bytes
            })
            .ok_or_else(|| "Le support a changé ou n’est plus détecté. Relance la détection avant de continuer.".to_string())?;
        let required_bytes = ((current_recommendation.image_size_gb
            + current_recommendation.initial_server_storage_gb)
            .ceil()
            * 1_000_000_000.0) as u64;
        if target.size_bytes < required_bytes {
            return Err(format!(
                "{} ne laisse pas assez de place. Il faut au moins {} Go. Aucun support n’a été modifié.",
                target.name,
                required_bytes.div_ceil(1_000_000_000)
            ));
        }
        let target_device = match std::env::consts::OS {
            "macos" => format!("/dev/{}", target.id),
            "windows" => format!(r"\\.\{}", target.id),
            "linux" => target.id.clone(),
            _ => return Err("Ce système ne peut pas préparer le support Raspberry Pi.".into()),
        };
        pi_imager::flash_pi_os(
            &app_data,
            &target_device,
            &image_url,
            &image_sha256,
            &wifi_ssid,
            &wifi_password,
            &wifi_country,
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

fn discover_macos_boot_targets() -> Result<Vec<PiBootTarget>, String> {
    let output = Command::new("/usr/sbin/diskutil")
        .args(["list", "external", "physical"])
        .output()
        .map_err(|_| "macOS n’a pas pu rechercher les supports externes. Vérifie les autorisations de l’application.".to_string())?;
    if !output.status.success() {
        return Err("macOS n’a pas autorisé la recherche des supports externes. Aucun disque n’a été modifié.".into());
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    for line in listing.lines() {
        let Some(rest) = line.strip_prefix("/dev/") else {
            continue;
        };
        let Some((disk, flags)) = rest.split_once(' ') else {
            continue;
        };
        if !flags.contains("external") || !flags.contains("physical") || !disk.starts_with("disk") {
            continue;
        }
        let info = Command::new("/usr/sbin/diskutil")
            .args(["info", &format!("/dev/{disk}")])
            .output()
            .ok()
            .filter(|result| result.status.success())
            .map(|result| String::from_utf8_lossy(&result.stdout).into_owned())
            .unwrap_or_default();
        let field = |key: &str| {
            info.lines().find_map(|row| {
                let (name, value) = row.split_once(':')?;
                (name.trim() == key).then(|| value.trim().to_string())
            })
        };
        let name = field("Media Name")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Support externe".into());
        let disk_size = field("Disk Size").unwrap_or_default();
        let size_bytes = parse_disk_size_bytes(&disk_size).unwrap_or_default();
        let size = if disk_size.is_empty() {
            "Taille inconnue".into()
        } else {
            disk_size
        };
        let connection = field("Bus Protocol").unwrap_or_else(|| "Externe".into());
        devices.push(PiBootTarget {
            id: disk.to_string(),
            name,
            size,
            size_bytes,
            connection,
        });
    }
    Ok(devices)
}

fn discover_windows_boot_targets() -> Result<Vec<PiBootTarget>, String> {
    let script = "$ErrorActionPreference='Stop'; Get-Disk | Where-Object { -not $_.IsBoot -and -not $_.IsSystem -and ($_.BusType -in @('USB','SD')) } | Select-Object Number,FriendlyName,Size,BusType | ConvertTo-Json -Compress";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|_| "Windows n’a pas pu rechercher les supports amovibles.".to_string())?;
    if !output.status.success() {
        return Err("Windows n’a pas autorisé la recherche des supports amovibles. Aucun disque n’a été modifié.".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or(serde_json::Value::Null);
    let entries = value.as_array().cloned().unwrap_or_else(|| {
        if value.is_object() {
            vec![value]
        } else {
            Vec::new()
        }
    });
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let number = entry.get("Number")?.as_u64()?;
            let bytes = entry.get("Size")?.as_u64().unwrap_or_default();
            let name = entry
                .get("FriendlyName")?
                .as_str()
                .unwrap_or("Support amovible")
                .to_string();
            let connection = entry
                .get("BusType")?
                .as_str()
                .unwrap_or("Externe")
                .to_string();
            Some(PiBootTarget {
                id: format!("PhysicalDrive{number}"),
                name,
                size: format_bytes(bytes),
                size_bytes: bytes,
                connection,
            })
        })
        .collect())
}

fn discover_linux_boot_targets() -> Result<Vec<PiBootTarget>, String> {
    let output = Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "--paths",
            "--output",
            "NAME,SIZE,MODEL,TRAN,RM,TYPE,MOUNTPOINTS",
        ])
        .output()
        .map_err(|_| "Linux n’a pas pu rechercher les supports amovibles.".to_string())?;
    if !output.status.success() {
        return Err("Linux n’a pas pu lire la liste des supports amovibles.".into());
    }
    let root_disk = Command::new("sh")
        .args([
            "-c",
            "lsblk -no PKNAME \"$(findmnt -n -o SOURCE --target /)\" 2>/dev/null",
        ])
        .output()
        .ok()
        .filter(|r| r.status.success())
        .map(|r| String::from_utf8_lossy(&r.stdout).trim().to_string())
        .unwrap_or_default();
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "La liste des supports Linux est illisible.".to_string())?;
    let disks = value
        .get("blockdevices")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let targets = disks
        .into_iter()
        .filter_map(|disk| {
            if disk.get("type")?.as_str()? != "disk" {
                return None;
            }
            let id = disk.get("name")?.as_str()?.to_string();
            let transport = disk
                .get("tran")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let removable = disk
                .get("rm")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
                == 1;
            if id.trim_start_matches("/dev/") == root_disk
                || contains_root_mount(&disk)
                || !(removable || matches!(transport, "usb" | "mmc"))
            {
                return None;
            }
            let bytes = disk
                .get("size")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default();
            let name = disk
                .get("model")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("Support amovible")
                .trim()
                .to_string();
            Some(PiBootTarget {
                id,
                name,
                size: format_bytes(bytes),
                size_bytes: bytes,
                connection: if transport.is_empty() {
                    "Externe".into()
                } else {
                    transport.to_uppercase()
                },
            })
        })
        .collect();
    Ok(targets)
}

fn contains_root_mount(device: &serde_json::Value) -> bool {
    let mounted_here = device
        .get("mountpoints")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|mounts| mounts.iter().any(|mount| mount.as_str() == Some("/")))
        || device.get("mountpoint").and_then(serde_json::Value::as_str) == Some("/");
    mounted_here
        || device
            .get("children")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|children| children.iter().any(contains_root_mount))
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "Taille inconnue".into();
    }
    format!("{:.1} Go", bytes as f64 / 1_000_000_000.0)
}

fn parse_disk_size_bytes(value: &str) -> Option<u64> {
    let start = value.find('(')? + 1;
    let remainder = &value[start..];
    let end = remainder.find(" Bytes")?;
    remainder[..end].replace(',', "").parse().ok()
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
async fn search_paper_plugins(
    minecraft_version: String,
    query: String,
) -> Result<Vec<engine::PaperPluginSuggestion>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::search_paper_plugins(&minecraft_version, &query)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn install_paper_plugin_copy(
    server_path: String,
    project_id: String,
    version_number: String,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<engine::PaperPluginInstallResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::install_paper_plugin_copy(
            std::path::Path::new(&server_path),
            &project_id,
            &version_number,
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
async fn install_pi_paper_plugin_copy(
    host: String,
    username: String,
    server_folder: String,
    project_id: String,
    version_number: String,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<engine::PaperPluginInstallResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::install_pi_paper_plugin_copy(
            &host,
            &username,
            &server_folder,
            &project_id,
            &version_number,
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
async fn get_pi_os_recommendation() -> Result<engine::PiOsRecommendation, String> {
    tauri::async_runtime::spawn_blocking(engine::pi_os_recommendation)
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
async fn inspect_local_server(server_path: String) -> Result<engine::LocalServerStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::inspect_local_server(std::path::Path::new(&server_path))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn check_local_update(server_path: String) -> Result<engine::PiUpdateCheck, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::check_local_update(std::path::Path::new(&server_path))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn start_local_server(
    server_path: String,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<engine::LocalStartResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::start_local_server(std::path::Path::new(&server_path), |step, percent| {
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
async fn stop_local_server(server_path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::stop_local_server(std::path::Path::new(&server_path))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn local_whitelist_status(
    server_path: String,
) -> Result<engine::LocalWhitelistStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::local_whitelist_status(std::path::Path::new(&server_path))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn change_local_whitelist(
    server_path: String,
    platform: String,
    action: String,
    player_name: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::change_local_whitelist(
            &std::path::Path::new(&server_path),
            &platform,
            &action,
            &player_name,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn set_local_whitelist_enabled(server_path: String, enabled: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::set_local_whitelist_enabled(std::path::Path::new(&server_path), enabled)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn export_local_server_log(
    server_path: String,
    output_file: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::export_local_server_log(
            std::path::Path::new(&server_path),
            std::path::Path::new(&output_file),
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
async fn prepare_pi_player_management(
    host: String,
    username: String,
    server_folder: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::prepare_pi_whitelist_migration(&host, &username, &server_folder)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn pi_whitelist_status(
    host: String,
    username: String,
    server_folder: String,
) -> Result<engine::PiWhitelistStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::pi_whitelist_status(&host, &username, &server_folder)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn change_pi_whitelist(
    host: String,
    username: String,
    server_folder: String,
    platform: String,
    action: String,
    player_name: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::change_pi_whitelist(
            &host,
            &username,
            &server_folder,
            &platform,
            &action,
            &player_name,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn set_pi_whitelist_enabled(
    host: String,
    username: String,
    server_folder: String,
    enabled: bool,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::set_pi_whitelist_enabled(&host, &username, &server_folder, enabled)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn check_pi_update(
    host: String,
    username: String,
    server_folder: String,
) -> Result<engine::PiUpdateCheck, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::check_pi_update(&host, &username, &server_folder)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn start_pi_server(
    host: String,
    username: String,
    server_folder: String,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::start_pi_server(&host, &username, &server_folder, |step, percent| {
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
async fn stop_pi_server(
    host: String,
    username: String,
    server_folder: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::stop_pi_server(&host, &username, &server_folder)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn export_pi_server_log(
    host: String,
    username: String,
    server_folder: String,
    output_file: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::export_pi_server_log(
            &host,
            &username,
            &server_folder,
            std::path::Path::new(&output_file),
        )
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
async fn backup_pi_server(
    host: String,
    username: String,
    server_folder: String,
    output_file: String,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<engine::OperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::create_pi_backup(
            &host,
            &username,
            &server_folder,
            std::path::Path::new(&output_file),
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

#[tauri::command]
async fn restore_pi_server(
    host: String,
    username: String,
    archive: String,
    progress: tauri::ipc::Channel<engine::ProgressMessage>,
) -> Result<engine::OperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::restore_pi_backup(
            &host,
            &username,
            std::path::Path::new(&archive),
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
            discover_new_pi,
            discover_pi_boot_targets,
            prepare_pi_sd,
            get_compatibility,
            search_paper_plugins,
            install_paper_plugin_copy,
            install_pi_paper_plugin_copy,
            get_pi_os_recommendation,
            create_server,
            inspect_local_server,
            check_local_update,
            start_local_server,
            stop_local_server,
            local_whitelist_status,
            change_local_whitelist,
            set_local_whitelist_enabled,
            prepare_pi_player_management,
            pi_whitelist_status,
            change_pi_whitelist,
            set_pi_whitelist_enabled,
            export_local_server_log,
            install_pi,
            inspect_pi_server,
            check_pi_update,
            start_pi_server,
            stop_pi_server,
            export_pi_server_log,
            backup_server,
            backup_pi_server,
            restore_server,
            restore_pi_server
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Monde Ensemble");
}
