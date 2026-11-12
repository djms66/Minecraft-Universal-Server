//! Safe local server preparation and portable world archives.
//!
//! The engine never replaces a running installation. New servers and restores
//! are assembled in a sibling staging folder and become visible only after all
//! downloads, checksums, archive paths and generated configuration pass checks.

use chrono::Utc;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    net::{TcpListener, UdpSocket},
    path::{Component, Path, PathBuf},
    process::Command,
    time::Duration,
};
use tar::{Archive, Builder, EntryType};
use uuid::Uuid;
use walkdir::WalkDir;

/// Minimum resources required by the shared Java/Bedrock server profile.
pub const MINIMUM_SERVER_MEMORY_GB: f64 = 2.0;
pub const INITIAL_SERVER_STORAGE_GB: f64 = 8.0;

const JAVA_DEFAULT_PORT: u16 = 25565;
const BEDROCK_DEFAULT_PORT: u16 = 19132;
const JAVA_ALTERNATE_PORT: u16 = 25566;
const BEDROCK_ALTERNATE_PORT: u16 = 19133;
const MAX_RESTORE_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
const REMOTE_PROBE_COMMAND: &str = r#"printf 'MODEL='; tr -d '\000' </sys/firmware/devicetree/base/model 2>/dev/null || true; printf '\nMEMORY_GB='; awk '/MemTotal/ {printf "%.1f\n", $2/1048576}' /proc/meminfo; printf 'FREE_GB='; df -BG --output=avail "$HOME" | tail -1 | tr -dc '0-9'; printf '\nARCH='; uname -m; printf '\nIP='; hostname -I | awk '{print $1}'; for p in 25565 25566 19132 19133; do printf 'PORT_%s=' "$p"; if ss -H -lntu 2>/dev/null | awk '{print $5}' | grep -qE "[:.]$p$"; then echo busy; else echo free; fi; done"#;

fn available_game_port(primary: u16, alternate: u16, udp: bool) -> Result<u16, String> {
    let can_bind = |port| {
        let address = format!("0.0.0.0:{port}");
        if udp {
            UdpSocket::bind(address).is_ok()
        } else {
            TcpListener::bind(address).is_ok()
        }
    };
    if can_bind(primary) {
        return Ok(primary);
    }
    if can_bind(alternate) {
        return Ok(alternate);
    }
    Err(format!(
        "Les ports Minecraft {primary} et {alternate} sont déjà utilisés. Ferme l’autre serveur puis réessaie."
    ))
}

#[derive(Clone, Serialize)]
pub struct ProgressMessage {
    pub step: String,
    pub percent: u8,
}

#[derive(Serialize)]
pub struct CompatibilityReport {
    pub paper_version: String,
    pub geyser_version: String,
    pub floodgate_version: String,
    pub bedrock_versions: String,
    pub java_version: String,
    pub checked_at: String,
    pub source: String,
}

#[derive(Serialize)]
pub struct OperationResult {
    pub path: String,
    pub detail: String,
    pub sha256: Option<String>,
}

#[derive(Serialize)]
pub struct DiscoveredPi {
    pub host: String,
    pub address: String,
    pub model: String,
}

#[derive(Serialize)]
pub struct PiOsRecommendation {
    pub name: String,
    pub release_date: String,
    pub image_url: String,
    pub image_sha256: String,
    pub download_sha256: String,
    pub download_size_gb: f64,
    pub image_size_gb: f64,
    pub devices: Vec<String>,
    pub init_format: String,
    pub minimum_server_memory_gb: f64,
    pub initial_server_storage_gb: f64,
    pub checked_at: String,
}

#[derive(Serialize)]
pub struct PiServerStatus {
    pub running: bool,
    pub ready: bool,
    pub java_port_open: bool,
    pub bedrock_port_open: bool,
    pub autostart_configured: bool,
    pub minecraft_java: String,
    pub minecraft_bedrock: String,
    pub java_port: u16,
    pub bedrock_port: u16,
    pub address: String,
    pub log: String,
}

#[derive(Deserialize)]
struct PaperBuild {
    id: u64,
    channel: String,
    downloads: std::collections::HashMap<String, PaperDownload>,
}

#[derive(Deserialize)]
struct PaperDownload {
    name: String,
    url: String,
    checksums: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct BridgeBuild {
    version: String,
    build: u64,
    downloads: std::collections::HashMap<String, BridgeDownload>,
}

#[derive(Deserialize)]
struct BridgeDownload {
    name: String,
    sha256: String,
}

#[derive(Deserialize)]
struct RuntimeAsset {
    binaries: Vec<RuntimeBinary>,
}

#[derive(Deserialize)]
struct RuntimeBinary {
    package: RuntimePackage,
}

#[derive(Deserialize)]
struct RuntimePackage {
    checksum: String,
    link: String,
    name: String,
}

#[derive(Deserialize)]
struct RaspberryPiOsManifest {
    os_list: Vec<RaspberryPiOsEntry>,
}

#[derive(Deserialize)]
struct RaspberryPiOsEntry {
    name: String,
    release_date: Option<String>,
    url: Option<String>,
    extract_sha256: Option<String>,
    image_download_sha256: Option<String>,
    image_download_size: Option<u64>,
    extract_size: Option<u64>,
    init_format: Option<String>,
    devices: Option<Vec<String>>,
    subitems: Option<Vec<RaspberryPiOsEntry>>,
}

struct Artifact {
    name: String,
    version: String,
    url: String,
    checksum: String,
}

/// Removes an in-progress archive on every early-return path.
struct RemoveFileOnDrop(PathBuf);

impl Drop for RemoveFileOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct RemoveDirectoryOnDrop(PathBuf);

impl Drop for RemoveDirectoryOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn compatibility() -> Result<CompatibilityReport, String> {
    let client = client()?;
    let supported = supported_versions(&client)?;
    let paper = paper_build(&client, &supported.java)?;
    let geyser = bridge_build(&client, "geyser", "spigot")?;
    let floodgate = bridge_build(&client, "floodgate", "spigot")?;

    Ok(CompatibilityReport {
        paper_version: format!("{} · build {}", supported.java, paper.id),
        geyser_version: format!("{} · build {}", geyser.version, geyser.build),
        floodgate_version: format!("{} · build {}", floodgate.version, floodgate.build),
        bedrock_versions: supported.bedrock,
        java_version: supported.java,
        checked_at: Utc::now().to_rfc3339(),
        source: "PaperMC et GeyserMC officiels".into(),
    })
}

/// Selects the current official Raspberry Pi OS Lite 64-bit image suitable
/// for first-boot customization. No image is downloaded or written here.
pub fn pi_os_recommendation() -> Result<PiOsRecommendation, String> {
    const MANIFEST_URL: &str = "https://downloads.raspberrypi.com/os_list_imagingutility_v4.json";
    let client = client()?;
    let manifest: RaspberryPiOsManifest = get_json(&client, MANIFEST_URL)?;
    let mut entries = Vec::new();
    flatten_pi_os_entries(manifest.os_list, &mut entries);
    let mut candidates: Vec<RaspberryPiOsEntry> = entries
        .into_iter()
        .filter(|entry| {
            entry
                .name
                .eq_ignore_ascii_case("Raspberry Pi OS Lite (64-bit)")
                && entry.init_format.as_deref() == Some("cloudinit-rpi")
                && entry
                    .url
                    .as_deref()
                    .is_some_and(|url| url.starts_with("https://"))
                && entry.devices.as_ref().is_some_and(|devices| {
                    devices.iter().any(|device| {
                        ["pi3", "pi4", "pi5"]
                            .iter()
                            .any(|supported| device.starts_with(supported))
                    })
                })
                && entry.extract_sha256.as_deref().is_some_and(valid_sha256)
                && entry.extract_size.is_some()
                && entry.release_date.is_some()
        })
        .collect();
    candidates.sort_by(|a, b| b.release_date.cmp(&a.release_date));
    let image = candidates.into_iter().next().ok_or_else(|| {
        "Le catalogue officiel ne fournit pas d’image Raspberry Pi OS Lite 64 bits avec configuration de premier démarrage vérifiable. Préparation bloquée.".to_string()
    })?;
    Ok(PiOsRecommendation {
        name: image.name,
        release_date: image.release_date.unwrap_or_default(),
        image_url: image.url.unwrap_or_default(),
        image_sha256: image.extract_sha256.unwrap_or_default(),
        download_sha256: image
            .image_download_sha256
            .filter(|digest| valid_sha256(digest))
            .unwrap_or_default(),
        download_size_gb: (image.image_download_size.unwrap_or_default() as f64 / 1_000_000_000.0
            * 10.0)
            .round()
            / 10.0,
        image_size_gb: (image.extract_size.unwrap_or_default() as f64 / 1_000_000_000.0 * 10.0)
            .round()
            / 10.0,
        devices: image.devices.unwrap_or_default(),
        init_format: image.init_format.unwrap_or_default(),
        minimum_server_memory_gb: MINIMUM_SERVER_MEMORY_GB,
        initial_server_storage_gb: INITIAL_SERVER_STORAGE_GB,
        checked_at: Utc::now().to_rfc3339(),
    })
}

fn flatten_pi_os_entries(
    entries: Vec<RaspberryPiOsEntry>,
    flattened: &mut Vec<RaspberryPiOsEntry>,
) {
    for mut entry in entries {
        if let Some(children) = entry.subitems.take() {
            flatten_pi_os_entries(children, flattened);
        }
        flattened.push(entry);
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

struct SupportedVersions {
    java: String,
    bedrock: String,
}

fn supported_versions(client: &Client) -> Result<SupportedVersions, String> {
    let page = get_text(
        client,
        "https://geysermc.org/wiki/geyser/supported-versions/",
    )?;
    let plain = Regex::new(r"(?s)<[^>]+>")
        .map_err(|e| e.to_string())?
        .replace_all(&page, " ");
    let collapsed = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let start = collapsed
        .find("Currently supported versions")
        .ok_or_else(|| {
            "Geyser ne publie pas le tableau de compatibilité attendu; installation bloquée."
                .to_string()
        })?;
    let section = &collapsed[start..collapsed.len().min(start + 600)];
    let bedrock = capture_version(section, "Bedrock")?;
    let java = capture_version(section, "Java")?;
    if !safe_version(&java) || !safe_version(&bedrock) {
        return Err(
            "Le catalogue Geyser a renvoyé une version inattendue; installation bloquée.".into(),
        );
    }
    Ok(SupportedVersions { java, bedrock })
}

fn capture_version(section: &str, label: &str) -> Result<String, String> {
    let pattern = format!(
        r"(?i)Minecraft\s+{label}\s+([0-9]+(?:\.[0-9]+)+(?:\s*[-–]\s*[0-9]+(?:\.[0-9]+)+)?)"
    );
    Regex::new(&pattern)
        .map_err(|e| e.to_string())?
        .captures(section)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().replace(' ', ""))
        .ok_or_else(|| {
            format!("La version Minecraft {label} n’a pas pu être lue dans le catalogue Geyser.")
        })
}

fn paper_build(client: &Client, game_version: &str) -> Result<PaperBuild, String> {
    let url = format!("https://fill.papermc.io/v3/projects/paper/versions/{game_version}/builds");
    let builds: Vec<PaperBuild> = get_json(client, &url)?;
    builds
        .into_iter()
        .filter(|build| build.channel.eq_ignore_ascii_case("STABLE"))
        .max_by_key(|build| build.id)
        .filter(|build| build.downloads.contains_key("server:default"))
        .ok_or_else(|| {
            format!("Paper n’a aucun build stable disponible pour Minecraft {game_version}.").into()
        })
}

fn bridge_build(client: &Client, project: &str, flavor: &str) -> Result<BridgeBuild, String> {
    let url = format!(
        "https://download.geysermc.org/v2/projects/{project}/versions/latest/builds/latest"
    );
    let build: BridgeBuild = get_json(client, &url)?;
    if !build.downloads.contains_key(flavor) {
        return Err(format!(
            "Le téléchargement officiel {project} / {flavor} est absent."
        ));
    }
    Ok(build)
}

pub fn install_local(
    destination_root: &Path,
    accept_eula: bool,
    memory_gb: Option<f64>,
    progress: impl Fn(&str, u8),
) -> Result<OperationResult, String> {
    if !accept_eula {
        return Err("Confirme le CLUF Minecraft avant de préparer le serveur.".into());
    }
    let memory_gb = memory_gb.ok_or_else(|| "La mémoire de cet ordinateur n’a pas pu être mesurée; installation bloquée pour éviter un réglage hasardeux.".to_string())?;
    if memory_gb < MINIMUM_SERVER_MEMORY_GB {
        return Err("Cette machine a moins de 2 Go de mémoire. L’application bloque la préparation plutôt que de créer un serveur instable.".into());
    }
    // Keep enough memory for the operating system and the desktop. The cap is
    // deliberately conservative for a beginner-friendly shared server.
    let heap_gb = ((memory_gb * 0.4).floor() as u8).clamp(1, 6);
    let java_port = available_game_port(JAVA_DEFAULT_PORT, JAVA_ALTERNATE_PORT, false)?;
    let bedrock_port = available_game_port(BEDROCK_DEFAULT_PORT, BEDROCK_ALTERNATE_PORT, true)?;
    let root = prepare_root(destination_root)?;
    let staging = root.join(format!(".monde-ensemble-{}.preparing", Uuid::new_v4()));
    fs::create_dir(&staging).map_err(io_error)?;
    let result = build_fresh_server(&staging, heap_gb, false, java_port, bedrock_port, &progress);
    let artifacts = match result {
        Ok(artifacts) => artifacts,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    let final_path = unique_folder(&root, "Monde-Ensemble");
    fs::rename(&staging, &final_path).map_err(|error| {
        let _ = fs::remove_dir_all(&staging);
        format!("Impossible de terminer l’installation dans le dossier choisi : {error}")
    })?;
    progress("Premier démarrage du nouveau serveur…", 93);
    start_local_server_and_wait(
        &final_path,
        heap_gb,
        artifacts.4,
        artifacts.5,
        &progress,
    )
    .map_err(|error| {
        format!(
            "Les fichiers sont prêts dans {} mais le serveur n’a pas confirmé son démarrage. Le monde précédent n’a pas été touché. Consulte le journal : {} · {error}",
            final_path.display(),
            final_path.join("serveur.log").display()
        )
    })?;
    let address = local_network_address().unwrap_or_else(|| "127.0.0.1".into());
    let profile = format!(
        "Serveur démarré et contrôlé · Minecraft Java {} · Bedrock compatible {} · Geyser {} · Floodgate {} · Java {address}:{} · Bedrock {address}:{} · ports Java {} / Bedrock {}",
        artifacts.0,
        artifacts.1,
        artifacts.2,
        artifacts.3,
        artifacts.4,
        artifacts.5,
        artifacts.4,
        artifacts.5
    );
    progress("Serveur démarré et prêt sur cet ordinateur.", 100);
    Ok(OperationResult {
        path: final_path.display().to_string(),
        detail: profile,
        sha256: None,
    })
}

fn start_local_server_and_wait(
    server_dir: &Path,
    heap_gb: u8,
    java_port: u16,
    bedrock_port: u16,
    progress: &impl Fn(&str, u8),
) -> Result<(), String> {
    let java = WalkDir::new(server_dir.join("runtime"))
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| {
            entry.file_type().is_file()
                && if cfg!(windows) {
                    entry.file_name() == "java.exe"
                } else {
                    entry.file_name() == "java"
                }
        })
        .map(|entry| entry.into_path())
        .ok_or_else(|| "Le runtime Java inclus est introuvable.".to_string())?;
    let log_path = server_dir.join("serveur.log");
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(io_error)?;
    let stdout = log.try_clone().map_err(io_error)?;
    let mut command = Command::new(java);
    command
        .current_dir(server_dir)
        .args([
            format!("-Xms1G"),
            format!("-Xmx{heap_gb}G"),
            "-jar".into(),
            "paper.jar".into(),
            "--nogui".into(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(stdout)
        .stderr(log);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("Java n’a pas pu démarrer : {error}"))?;
    let pid_file = server_dir.join("serveur.pid");
    if let Err(error) = fs::write(&pid_file, child.id().to_string()) {
        terminate_local_process(&mut child);
        return Err(io_error(error));
    }
    for attempt in 0..60 {
        if fs::read_to_string(&log_path)
            .map(|contents| contents.contains("Done ("))
            .unwrap_or(false)
        {
            if !tcp_port_open(java_port) {
                terminate_local_process(&mut child);
                let _ = fs::remove_file(&pid_file);
                return Err(format!(
                    "Le journal indique un démarrage, mais le port Java {java_port} ne répond pas."
                ));
            }
            if udp_port_available(bedrock_port) {
                terminate_local_process(&mut child);
                let _ = fs::remove_file(&pid_file);
                return Err(format!(
                    "Le serveur Java est prêt, mais le port Bedrock UDP {bedrock_port} n’est pas ouvert."
                ));
            }
            return Ok(());
        }
        if let Some(status) = child.try_wait().map_err(io_error)? {
            let _ = fs::remove_file(&pid_file);
            return Err(format!(
                "Le serveur s’est arrêté avant d’être prêt (code {status})."
            ));
        }
        progress(
            "Minecraft démarre et vérifie les connexions Java et Bedrock…",
            93 + (attempt / 10) as u8,
        );
        std::thread::sleep(Duration::from_secs(2));
    }
    terminate_local_process(&mut child);
    let _ = fs::remove_file(pid_file);
    Err("Le premier démarrage a dépassé deux minutes.".into())
}

fn tcp_port_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(500),
    )
    .is_ok()
}

fn udp_port_available(port: u16) -> bool {
    UdpSocket::bind(("127.0.0.1", port)).is_ok()
}

fn local_network_address() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_unspecified() && !address.is_loopback()).then(|| address.to_string())
}

fn terminate_local_process(child: &mut std::process::Child) {
    let pid = child.id().to_string();
    #[cfg(windows)]
    let _ = Command::new("taskkill.exe")
        .args(["/PID", &pid, "/T"])
        .status();
    #[cfg(unix)]
    let _ = Command::new("/bin/kill").args(["-TERM", &pid]).status();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Installs a complete Linux ARM64 server on an already configured Raspberry Pi.
/// It only creates a new, uniquely named folder under the SSH account's home.
pub fn install_remote_pi(
    host: &str,
    username: &str,
    accept_eula: bool,
    progress: impl Fn(&str, u8),
) -> Result<OperationResult, String> {
    if !valid_remote_name(host) || !valid_remote_name(username) {
        return Err("Nom du Raspberry Pi ou compte invalide.".into());
    }
    if !accept_eula {
        return Err("Confirme le CLUF Minecraft avant de préparer le serveur.".into());
    }
    progress(
        "Recherche du Raspberry Pi et vérification de sa capacité…",
        3,
    );
    let destination = format!("{username}@{host}");
    let inspect = run_ssh(&destination, REMOTE_PROBE_COMMAND)?;
    let facts = parse_remote_facts(&inspect)?;
    if facts.architecture != "aarch64" {
        return Err(format!(
            "Ce Pi utilise {}. Le profil sécurisé demande Raspberry Pi OS 64 bits sur Raspberry Pi 3 ou plus récent.",
            facts.architecture
        ));
    }
    if facts.memory_gb < MINIMUM_SERVER_MEMORY_GB {
        return Err(format!(
            "{} n’a que {:.1} Go de mémoire. Le serveur partagé demande au moins 2 Go.",
            facts.model, facts.memory_gb
        ));
    }
    if facts.free_gb < 8 {
        return Err(format!(
            "Il reste {} Go libres sur le disque système du Pi. Libère au moins 8 Go avant l’installation.",
            facts.free_gb
        ));
    }
    if facts.ip.is_empty() {
        return Err("Le Pi a répondu, mais son adresse locale n’a pas pu être détectée. Aucun fichier n’a été envoyé.".into());
    }
    let java_port = select_remote_port(
        facts.java_default_busy,
        facts.java_alternate_busy,
        JAVA_DEFAULT_PORT,
        JAVA_ALTERNATE_PORT,
    )?;
    let bedrock_port = select_remote_port(
        facts.bedrock_default_busy,
        facts.bedrock_alternate_busy,
        BEDROCK_DEFAULT_PORT,
        BEDROCK_ALTERNATE_PORT,
    )?;
    let heap_gb = ((facts.memory_gb * 0.4).floor() as u8).clamp(1, 6);
    let id = Uuid::new_v4().simple().to_string();
    let local_stage = std::env::temp_dir().join(format!("monde-ensemble-{id}"));
    fs::create_dir(&local_stage).map_err(io_error)?;
    let _cleanup_stage = RemoveDirectoryOnDrop(local_stage.clone());
    progress("Préparation du profil Linux ARM64 compatible…", 8);
    let profile = build_fresh_server(
        &local_stage,
        heap_gb,
        true,
        java_port,
        bedrock_port,
        &progress,
    )?;

    let package_path = local_stage.with_extension("tar.gz");
    let _cleanup_package = RemoveFileOnDrop(package_path.clone());
    let package = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&package_path)
        .map_err(io_error)?;
    let encoder = GzEncoder::new(package, Compression::fast());
    let mut archive = Builder::new(encoder);
    archive
        .append_dir_all("server", &local_stage)
        .map_err(io_error)?;
    archive
        .into_inner()
        .map_err(io_error)?
        .finish()
        .map_err(io_error)?
        .sync_all()
        .map_err(io_error)?;

    let remote_archive = format!("Monde-Ensemble-{id}.tar.gz");
    progress("Envoi sécurisé des fichiers vérifiés au Pi…", 84);
    let scp = Command::new("scp")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "PasswordAuthentication=no",
            package_path.to_string_lossy().as_ref(),
            &format!("{destination}:{remote_archive}"),
        ])
        .output()
        .map_err(|_| {
            "L’outil de transfert sécurisé n’est pas disponible sur cet ordinateur.".to_string()
        })?;
    if !scp.status.success() {
        return Err("Le transfert vers le Pi a échoué. Rien n’a été installé. Vérifie l’accès SSH et réessaie.".into());
    }

    progress("Installation et démarrage du serveur sur le Pi…", 91);
    let prepare_command = format!(
        "set -eu; d=\"$HOME/Monde-Ensemble-{id}\"; mkdir \"$d\"; tar -xzf \"$HOME/{remote_archive}\" -C \"$d\"; rm \"$HOME/{remote_archive}\"; cd \"$d/server\"; test -s paper.jar; test -s plugins/Geyser-Spigot.jar; test -s plugins/Floodgate-Spigot.jar; AUTOSTART=no; if command -v crontab >/dev/null 2>&1 && (systemctl is-enabled --quiet cron 2>/dev/null || systemctl is-enabled --quiet crond 2>/dev/null); then t=\"$(mktemp)\"; (crontab -l 2>/dev/null || true) | grep -Fv '# MONDE_ENSEMBLE_AUTO_START' > \"$t\" || true; printf '%s\\n' '@reboot \"$HOME/Monde-Ensemble-{id}/server/demarrer-au-redemarrage.sh\" # MONDE_ENSEMBLE_AUTO_START' >> \"$t\"; crontab \"$t\"; rm -f \"$t\"; if crontab -l 2>/dev/null | grep -Fq '# MONDE_ENSEMBLE_AUTO_START'; then AUTOSTART=yes; fi; fi; nohup ./demarrer.sh > serveur.log 2>&1 < /dev/null & echo $! > serveur.pid; printf 'AUTOSTART=%s\\n' \"$AUTOSTART\""
    );
    run_ssh(&destination, &prepare_command).map_err(|error| {
        format!("Les fichiers ont été transférés, mais leur préparation a échoué dans ~/Monde-Ensemble-{id}/server. Rien n’a été écrit dans les autres dossiers du Pi. Détail : {error}")
    })?;
    let mut started = false;
    for attempt in 0..30 {
        let check_command = format!(
            "cd \"$HOME/Monde-Ensemble-{id}/server\"; if grep -q 'Done (' serveur.log; then echo READY; elif kill -0 \"$(cat serveur.pid)\" 2>/dev/null; then echo STARTING; else tail -n 60 serveur.log; exit 3; fi"
        );
        let status = run_ssh(&destination, &check_command).map_err(|error| {
            format!("Le serveur a été installé, mais n’a pas confirmé son démarrage. Les détails sont dans `~/Monde-Ensemble-{id}/server/serveur.log` sur le Pi. {error}")
        })?;
        if status.lines().any(|line| line == "READY") {
            started = true;
            break;
        }
        progress(
            "Le Pi démarre Minecraft pour la première fois…",
            92 + (attempt / 5) as u8,
        );
        std::thread::sleep(Duration::from_secs(2));
    }
    if !started {
        return Err(format!(
            "Le serveur a été installé, mais le premier démarrage dépasse 60 secondes. Consulte `~/Monde-Ensemble-{id}/server/serveur.log` sur le Pi."
        ));
    }
    let autostart_configured = run_ssh(
        &destination,
        &format!(
            "crontab -l 2>/dev/null | grep -Fq '# MONDE_ENSEMBLE_AUTO_START' && echo yes || echo no"
        ),
    )
    .map(|output| output.lines().any(|line| line == "yes"))
    .unwrap_or(false);
    let ip = &facts.ip;
    if ip.is_empty() {
        return Err(
            "Le serveur a démarré, mais l’adresse locale du Pi n’a pas pu être lue.".into(),
        );
    }
    progress("Serveur démarré et contrôlé sur le Raspberry Pi.", 100);
    Ok(OperationResult {
        path: format!("{username}@{host}:~/Monde-Ensemble-{id}/server"),
        detail: format!(
            "Serveur démarré et prêt · démarrage automatique après redémarrage {} · {} · Minecraft Java {} · Bedrock compatible {} · Geyser {} · Floodgate {} · Java {} · mémoire serveur {} Go · Chez toi, PC/Mac Java : {ip}:{} · mobile/tablette Bedrock : {ip}:{} · Depuis l’extérieur, le routeur doit aussi autoriser ces ports.",
            if autostart_configured {
                "configuré"
            } else {
                "non disponible sur ce Pi"
            },
            facts.model,
            profile.0,
            profile.1,
            profile.2,
            profile.3,
            java_major_for(&profile.0)?,
            heap_gb,
            profile.4,
            profile.5
        ),
        sha256: None,
    })
}

/// Uses local DNS-SD to find SSH devices and confirms Raspberry Pi identity
/// without writing to any device. A configured SSH key is required.
pub fn discover_pi(username: &str) -> Result<Vec<DiscoveredPi>, String> {
    discover_pi_internal(username, None)
}

/// Finds only the Pi provisioned by the first-boot card workflow. Its SSH
/// host key is accepted on first contact only for the unique hostname written
/// to that card; ordinary Pi discovery keeps strict host-key checking.
pub fn discover_new_pi(
    username: &str,
    expected_hostname: &str,
) -> Result<Vec<DiscoveredPi>, String> {
    if !expected_hostname.starts_with("monde-ensemble-") || !valid_remote_name(expected_hostname) {
        return Err("Le nom sécurisé du Pi préparé n’est pas reconnu.".into());
    }
    discover_pi_internal(username, Some(expected_hostname))
}

fn discover_pi_internal(
    username: &str,
    expected_hostname: Option<&str>,
) -> Result<Vec<DiscoveredPi>, String> {
    if !valid_remote_name(username) {
        return Err("Saisis le nom du compte du Pi pour le retrouver automatiquement.".into());
    }
    let mdns = ServiceDaemon::new()
        .map_err(|error| format!("La détection du réseau local n’a pas pu démarrer : {error}"))?;
    let receiver = mdns
        .browse("_ssh._tcp.local.")
        .map_err(|error| format!("La recherche du Raspberry Pi a échoué : {error}"))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    let mut candidates = std::collections::BTreeMap::<String, String>::new();
    while std::time::Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(ServiceEvent::ServiceResolved(service)) if service.get_port() == 22 => {
                let host = service.get_hostname().trim_end_matches('.').to_string();
                let hostname_without_local = host.strip_suffix(".local").unwrap_or(&host);
                if expected_hostname
                    .is_some_and(|expected| !hostname_without_local.eq_ignore_ascii_case(expected))
                {
                    continue;
                }
                for address in service.get_addresses_v4() {
                    candidates
                        .entry(host.clone())
                        .or_insert_with(|| address.to_string());
                }
            }
            Ok(_) | Err(_) => {}
        }
    }
    let _ = mdns.stop_browse("_ssh._tcp.local.");
    let _ = mdns.shutdown();
    if candidates.is_empty() {
        return Err("Aucun appareil SSH n’a été annoncé sur le réseau local. Vérifie que le Pi est allumé sur le même Wi-Fi ou câble réseau.".into());
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    let mut targets = Vec::new();
    for (host, address) in candidates {
        targets.push((host, address.clone()));
        if expected_hostname.is_none() {
            targets.push((address.clone(), address));
        }
    }
    targets.sort();
    targets.dedup();
    let accept_new_host_key = expected_hostname.is_some();
    for (host, address) in targets.into_iter().take(12) {
        let sender = sender.clone();
        let username = username.to_string();
        std::thread::spawn(move || {
            let result = run_ssh_timeout_policy(
                &format!("{username}@{host}"),
                REMOTE_PROBE_COMMAND,
                3,
                accept_new_host_key,
            );
            let _ = sender.send((host, address, result));
        });
    }
    drop(sender);
    let mut discovered = receiver
        .into_iter()
        .filter_map(|(host, address, result)| {
            let facts = parse_remote_facts(&result.ok()?).ok()?;
            Some(DiscoveredPi {
                host,
                address,
                model: facts.model,
            })
        })
        .collect::<Vec<_>>();
    discovered.sort_by(|left, right| left.host.cmp(&right.host));
    discovered.dedup_by(|left, right| left.address == right.address);
    if discovered.is_empty() {
        return Err("Le réseau a répondu, mais aucun Pi n’a pu être vérifié avec le compte et la clé SSH de cet ordinateur. Rien n’a été modifié.".into());
    }
    Ok(discovered)
}

/// Reads the saved installation's process, ports and recent log without
/// changing anything on the Raspberry Pi.
pub fn inspect_pi_server(
    host: &str,
    username: &str,
    server_folder: &str,
) -> Result<PiServerStatus, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Le dossier enregistré du serveur n’est pas reconnu.".into());
    }
    let destination = format!("{username}@{host}");
    let command = format!(
        "d=\"$HOME/{server_folder}\"; test -d \"$d\" || {{ echo 'DOSSIER_ABSENT'; exit 5; }}; printf 'RUNNING='; if test -r \"$d/serveur.pid\" && kill -0 \"$(cat \"$d/serveur.pid\")\" 2>/dev/null; then echo yes; else echo no; fi; printf 'READY='; if grep -q 'Done (' \"$d/serveur.log\" 2>/dev/null; then echo yes; else echo no; fi; printf 'JAVA_PORT='; if ss -H -lnt 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.](25565|25566)$'; then echo yes; else echo no; fi; printf 'BEDROCK_PORT='; if ss -H -lnu 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.](19132|19133)$'; then echo yes; else echo no; fi; printf 'AUTOSTART='; if crontab -l 2>/dev/null | grep -Fq '# MONDE_ENSEMBLE_AUTO_START'; then echo yes; else echo no; fi; printf 'IP='; hostname -I | awk '{{print $1}}'; printf 'PROFILE_START\\n'; cat \"$d/monde-ensemble-profile.json\" 2>/dev/null || true; printf '\\nPROFILE_END\\n'; printf 'LOG_START\\n'; tail -n 200 \"$d/serveur.log\" 2>/dev/null || true"
    );
    let output = run_ssh(&destination, &command)?;
    let (header, after_profile) = output
        .split_once("PROFILE_START\n")
        .ok_or_else(|| "Le Pi n’a pas renvoyé le profil Minecraft installé.".to_string())?;
    let (profile, after_profile) = after_profile
        .split_once("\nPROFILE_END\n")
        .ok_or_else(|| "Le profil Minecraft installé est incomplet.".to_string())?;
    let (_, log) = after_profile
        .split_once("LOG_START\n")
        .ok_or_else(|| "Le Pi n’a pas renvoyé les informations attendues.".to_string())?;
    let profile: serde_json::Value = serde_json::from_str(profile.trim())
        .map_err(|_| "Le profil des versions Minecraft est illisible.".to_string())?;
    let get = |key: &str| {
        header
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_default()
            .trim()
    };
    Ok(PiServerStatus {
        running: get("RUNNING=") == "yes",
        ready: get("RUNNING=") == "yes" && get("READY=") == "yes",
        java_port_open: get("JAVA_PORT=") == "yes",
        bedrock_port_open: get("BEDROCK_PORT=") == "yes",
        autostart_configured: get("AUTOSTART=") == "yes",
        minecraft_java: profile
            .get("minecraft_java")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        minecraft_bedrock: profile
            .get("minecraft_bedrock")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        java_port: profile
            .get("java_port")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(JAVA_ALTERNATE_PORT as u64) as u16,
        bedrock_port: profile
            .get("bedrock_port")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(BEDROCK_ALTERNATE_PORT as u64) as u16,
        address: get("IP=").to_string(),
        log: log.to_string(),
    })
}

/// Restarts only this app's saved installation, and only while both game
/// ports are free. The existing log is kept so the user can inspect history.
pub fn start_pi_server(host: &str, username: &str, server_folder: &str) -> Result<(), String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Le dossier enregistré du serveur n’est pas reconnu.".into());
    }
    let destination = format!("{username}@{host}");
    let command = format!(
        "set -eu; d=\"$HOME/{server_folder}\"; if ! test -x \"$d/demarrer.sh\"; then echo 'LANCEUR_ABSENT'; exit 0; fi; if test -r \"$d/serveur.pid\" && kill -0 \"$(cat \"$d/serveur.pid\")\" 2>/dev/null; then echo 'DEJA_LANCE'; exit 0; fi; if ss -H -lntu 2>/dev/null | awk '{{print $5}}' | grep -qE '[:.](25565|25566|19132|19133)$'; then echo 'PORT_OCCUPE'; exit 0; fi; cd \"$d\"; nohup ./demarrer.sh >> serveur.log 2>&1 < /dev/null & echo $! > serveur.pid; echo 'DEMARRAGE_LANCE'"
    );
    let output = run_ssh(&destination, &command)?;
    if output.lines().any(|line| line == "PORT_OCCUPE") {
        return Err(
            "Un autre programme utilise déjà un port Minecraft. Le serveur n’a pas été lancé."
                .into(),
        );
    }
    if output.lines().any(|line| line == "LANCEUR_ABSENT") {
        return Err("Le lanceur du serveur est absent. Aucun fichier n’a été modifié.".into());
    }
    Ok(())
}

fn valid_server_folder(value: &str) -> bool {
    Regex::new(r"^Monde-Ensemble-[a-f0-9]{32}/server$")
        .map(|pattern| pattern.is_match(value))
        .unwrap_or(false)
}

fn select_remote_port(
    primary_busy: bool,
    alternate_busy: bool,
    primary: u16,
    alternate: u16,
) -> Result<u16, String> {
    if !primary_busy {
        Ok(primary)
    } else if !alternate_busy {
        Ok(alternate)
    } else {
        Err(format!(
            "Les ports Minecraft {primary} et {alternate} sont déjà utilisés sur le Pi. Ferme l’autre serveur puis réessaie. Aucun fichier n’a été envoyé."
        ))
    }
}

struct RemoteFacts {
    model: String,
    memory_gb: f64,
    free_gb: u64,
    architecture: String,
    ip: String,
    java_default_busy: bool,
    java_alternate_busy: bool,
    bedrock_default_busy: bool,
    bedrock_alternate_busy: bool,
}

fn parse_remote_facts(output: &str) -> Result<RemoteFacts, String> {
    let values = output
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect::<std::collections::HashMap<_, _>>();
    let required = |key: &str| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| format!("Le Pi n’a pas fourni la caractéristique {key}."))
    };
    let model = required("MODEL")?.trim().to_string();
    if !model.to_lowercase().contains("raspberry pi") {
        return Err("L’appareil trouvé ne s’identifie pas comme un Raspberry Pi. Aucun fichier n’a été envoyé.".into());
    }
    Ok(RemoteFacts {
        model,
        memory_gb: required("MEMORY_GB")?
            .parse()
            .map_err(|_| "Mémoire du Pi illisible.".to_string())?,
        free_gb: required("FREE_GB")?
            .parse()
            .map_err(|_| "Espace libre du Pi illisible.".to_string())?,
        architecture: required("ARCH")?.trim().to_string(),
        ip: required("IP")?.trim().to_string(),
        java_default_busy: required("PORT_25565")?.trim() == "busy",
        java_alternate_busy: required("PORT_25566")?.trim() == "busy",
        bedrock_default_busy: required("PORT_19132")?.trim() == "busy",
        bedrock_alternate_busy: required("PORT_19133")?.trim() == "busy",
    })
}

fn valid_remote_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        && !value.starts_with('-')
}

fn run_ssh(destination: &str, command: &str) -> Result<String, String> {
    run_ssh_timeout(destination, command, 8)
}

fn run_ssh_timeout(
    destination: &str,
    command: &str,
    timeout_seconds: u8,
) -> Result<String, String> {
    run_ssh_timeout_policy(destination, command, timeout_seconds, false)
}

fn run_ssh_timeout_policy(
    destination: &str,
    command: &str,
    timeout_seconds: u8,
    accept_new_host_key: bool,
) -> Result<String, String> {
    let host_key_policy = if accept_new_host_key {
        "StrictHostKeyChecking=accept-new"
    } else {
        "StrictHostKeyChecking=yes"
    };
    let output = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            &format!("ConnectTimeout={timeout_seconds}"),
            "-o",
            host_key_policy,
            "-o",
            "PasswordAuthentication=no",
            destination,
            command,
        ])
        .output()
        .map_err(|_| {
            "L’outil d’accès sécurisé SSH n’est pas disponible sur cet ordinateur.".to_string()
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = [stderr, stdout]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let detail = detail.chars().take(6000).collect::<String>();
        return Err(if detail.is_empty() {
            "Connexion SSH refusée. Vérifie que le Pi est allumé et que sa clé d’hôte est déjà approuvée sur cet ordinateur.".into()
        } else {
            format!("Connexion SSH impossible : {detail}")
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn build_fresh_server(
    stage: &Path,
    heap_gb: u8,
    remote_linux: bool,
    java_port: u16,
    bedrock_port: u16,
    progress: &impl Fn(&str, u8),
) -> Result<(String, String, String, String, u16, u16), String> {
    let client = client()?;
    progress("Lecture des versions compatibles auprès de GeyserMC…", 3);
    let supported = supported_versions(&client)?;
    let paper = paper_build(&client, &supported.java)?;
    let geyser = bridge_build(&client, "geyser", "spigot")?;
    let floodgate = bridge_build(&client, "floodgate", "spigot")?;
    let paper_download = paper
        .downloads
        .get("server:default")
        .ok_or_else(|| "Le fichier Paper officiel est introuvable.".to_string())?;
    let geyser_download = geyser
        .downloads
        .get("spigot")
        .ok_or_else(|| "Le fichier Geyser officiel est introuvable.".to_string())?;
    let floodgate_download = floodgate
        .downloads
        .get("spigot")
        .ok_or_else(|| "Le fichier Floodgate officiel est introuvable.".to_string())?;

    let paper_sha = paper_download.checksums.get("sha256").ok_or_else(|| {
        "Paper n’a pas publié de contrôle SHA-256; installation bloquée.".to_string()
    })?;
    let artifacts = [
        Artifact {
            name: paper_download.name.clone(),
            version: format!("Paper {} build {}", supported.java, paper.id),
            url: paper_download.url.clone(),
            checksum: paper_sha.clone(),
        },
        Artifact {
            name: geyser_download.name.clone(),
            version: geyser.version.clone(),
            url: format!(
                "https://download.geysermc.org/v2/projects/geyser/versions/latest/builds/latest/downloads/spigot"
            ),
            checksum: geyser_download.sha256.clone(),
        },
        Artifact {
            name: floodgate_download.name.clone(),
            version: floodgate.version.clone(),
            url: format!(
                "https://download.geysermc.org/v2/projects/floodgate/versions/latest/builds/latest/downloads/spigot"
            ),
            checksum: floodgate_download.sha256.clone(),
        },
    ];
    fs::create_dir_all(stage.join("plugins")).map_err(io_error)?;
    for (index, artifact) in artifacts.iter().enumerate() {
        let percent = 10 + (index as u8 * 18);
        progress(
            &format!("Téléchargement et contrôle de {}…", artifact.name),
            percent,
        );
        let destination = if index == 0 {
            stage.join("paper.jar")
        } else {
            stage.join("plugins").join(&artifact.name)
        };
        download_verified(&client, artifact, &destination)?;
    }

    progress("Préparation du Java adapté à cet ordinateur…", 66);
    let java_major = java_major_for(&supported.java)?;
    let runtime = runtime_package(&client, java_major, remote_linux)?;
    let runtime_archive = stage.join(&runtime.name);
    download_url_verified(&client, &runtime.link, &runtime.checksum, &runtime_archive)?;
    progress("Installation du runtime Java inclus…", 78);
    unpack_runtime(&runtime_archive, stage.join("runtime"))?;
    fs::remove_file(runtime_archive).map_err(io_error)?;

    progress("Réglage des ports Java et Bedrock…", 89);
    configure_server(
        stage,
        &supported.java,
        &supported.bedrock,
        java_major,
        heap_gb,
        remote_linux,
        java_port,
        bedrock_port,
        &geyser,
        &floodgate,
    )?;
    Ok((
        supported.java,
        supported.bedrock,
        geyser.version,
        floodgate.version,
        java_port,
        bedrock_port,
    ))
}

/// Paper's current Java requirement follows its published game-version matrix.
/// Unknown formats are rejected rather than guessed.
fn java_major_for(game_version: &str) -> Result<u8, String> {
    if game_version.starts_with("26.") {
        Ok(25)
    } else {
        Err(format!(
            "La version Minecraft {game_version} ne correspond à aucune règle Java validée par l’application. Installation bloquée."
        ))
    }
}

fn runtime_package(
    client: &Client,
    major: u8,
    remote_linux: bool,
) -> Result<RuntimePackage, String> {
    let (os, arch) = runtime_target(remote_linux)?;
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jre&os={os}&vendor=eclipse"
    );
    let assets: Vec<RuntimeAsset> = get_json(client, &url)?;
    assets
        .into_iter()
        .flat_map(|asset| asset.binaries)
        .map(|binary| binary.package)
        .find(|package| !package.checksum.is_empty() && package.link.starts_with("https://"))
        .ok_or_else(|| format!("Aucun Java {major} officiel ne correspond à cet appareil."))
}

fn runtime_target(remote_linux: bool) -> Result<(&'static str, &'static str), String> {
    let os = if remote_linux {
        "linux"
    } else {
        match std::env::consts::OS {
            "macos" => "mac",
            "windows" => "windows",
            "linux" => "linux",
            other => return Err(format!("Le système {other} n’est pas pris en charge.")),
        }
    };
    let arch = if remote_linux {
        "aarch64"
    } else {
        match std::env::consts::ARCH {
            "aarch64" => "aarch64",
            "x86_64" => "x64",
            other => return Err(format!("L’architecture {other} n’est pas prise en charge.")),
        }
    };
    Ok((os, arch))
}

fn configure_server(
    stage: &Path,
    java_version: &str,
    bedrock_versions: &str,
    java_major: u8,
    heap_gb: u8,
    remote_linux: bool,
    java_port: u16,
    bedrock_port: u16,
    geyser: &BridgeBuild,
    floodgate: &BridgeBuild,
) -> Result<(), String> {
    let eula = format!("# EULA acceptée séparément dans l’application Monde Ensemble\neula=true\n");
    fs::write(stage.join("eula.txt"), eula).map_err(io_error)?;
    let properties = format!(
        "motd=Monde Ensemble\nserver-port={java_port}\nserver-ip=\nonline-mode=true\nenforce-secure-profile=false\nview-distance=8\nsimulation-distance=6\n"
    );
    fs::write(stage.join("server.properties"), properties).map_err(io_error)?;

    let geyser_jar = stage.join("plugins").join("Geyser-Spigot.jar");
    let mut jar =
        zip::ZipArchive::new(File::open(geyser_jar).map_err(io_error)?).map_err(|error| {
            format!("Le fichier Geyser téléchargé n’est pas une archive valide : {error}")
        })?;
    let mut template = String::new();
    jar.by_name("config.yml")
        .map_err(|_| "Le modèle officiel de configuration Geyser est absent.".to_string())?
        .read_to_string(&mut template)
        .map_err(io_error)?;
    let template = set_yaml(&template, "bedrock", "port", &bedrock_port.to_string());
    let template = set_yaml(&template, "bedrock", "transport", "nethernet");
    let template = set_yaml(&template, "remote", "auth-type", "floodgate");
    fs::create_dir_all(stage.join("plugins/Geyser-Spigot")).map_err(io_error)?;
    fs::write(stage.join("plugins/Geyser-Spigot/config.yml"), template).map_err(io_error)?;

    write_launch_files(stage, heap_gb, remote_linux, java_port, bedrock_port)?;
    let profile = serde_json::json!({
        "application": "Monde Ensemble",
        "minecraft_java": java_version,
        "minecraft_bedrock": bedrock_versions,
        "paper_build": "stable",
        "geyser": geyser.version,
        "floodgate": floodgate.version,
        "java_runtime": java_major,
        "java_heap_gb": heap_gb,
        "java_port": java_port,
        "bedrock_port": bedrock_port,
        "plugins": ["Geyser-Spigot", "Floodgate-Spigot"],
        "created_at": Utc::now().to_rfc3339(),
    });
    fs::write(
        stage.join("monde-ensemble-profile.json"),
        serde_json::to_vec_pretty(&profile).map_err(|error| error.to_string())?,
    )
    .map_err(io_error)?;
    Ok(())
}

fn set_yaml(source: &str, section: &str, key: &str, value: &str) -> String {
    let mut lines = source.lines().map(str::to_string).collect::<Vec<_>>();
    let section_start = lines.iter().position(|line| {
        !line.starts_with([' ', '\t']) && line.trim_start().starts_with(&format!("{section}:"))
    });
    let Some(start) = section_start else {
        lines.push(format!("{section}:"));
        lines.push(format!("  {key}: {value}"));
        return lines.join("\n") + "\n";
    };
    let end = ((start + 1)..lines.len())
        .find(|index| {
            let line = lines[*index].as_str();
            !line.trim().is_empty() && !line.starts_with([' ', '\t', '#'])
        })
        .unwrap_or(lines.len());
    if let Some(index) =
        ((start + 1)..end).find(|index| lines[*index].trim_start().starts_with(&format!("{key}:")))
    {
        let indent = lines[index].len() - lines[index].trim_start().len();
        lines[index] = format!("{}{}: {}", " ".repeat(indent), key, value);
    } else {
        lines.insert(start + 1, format!("  {key}: {value}"));
    }
    lines.join("\n") + "\n"
}

fn write_launch_files(
    stage: &Path,
    heap_gb: u8,
    remote_linux: bool,
    java_port: u16,
    bedrock_port: u16,
) -> Result<(), String> {
    if cfg!(windows) && !remote_linux {
        let script = format!(
            "@echo off\r\ncd /d \"%~dp0\"\r\nfor /r runtime %%J in (java.exe) do set JAVA_BIN=%%J\r\nif not defined JAVA_BIN echo Java inclus introuvable & exit /b 1\r\n\"%JAVA_BIN%\" -Xms1G -Xmx{heap_gb}G -jar paper.jar --nogui\r\npause\r\n"
        );
        fs::write(stage.join("demarrer.bat"), script).map_err(io_error)?;
    } else {
        let script = format!(
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nJAVA_BIN=$(find runtime -type f -name java -print -quit)\n[ -n \"$JAVA_BIN\" ] || {{ echo 'Java inclus introuvable.'; exit 1; }}\nexec \"$JAVA_BIN\" -Xms1G -Xmx{heap_gb}G -jar paper.jar --nogui\n"
        );
        let path = stage.join("demarrer.sh");
        fs::write(&path, script).map_err(io_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).map_err(io_error)?;
        }
    }
    if remote_linux {
        let script = "#!/bin/sh\nset -eu\nsleep 25\ncd \"$(dirname \"$0\")\"\nnohup ./demarrer.sh >> serveur.log 2>&1 < /dev/null &\necho $! > serveur.pid\n";
        let path = stage.join("demarrer-au-redemarrage.sh");
        fs::write(&path, script).map_err(io_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).map_err(io_error)?;
        }
    }
    let notes = format!(
        "MONDE ENSEMBLE — NOUVELLE INSTALLATION\n\nMinecraft Java : {java_port} (TCP)\nMinecraft Bedrock : {bedrock_port} (UDP et TCP)\nLes deux éditions rejoignent le même monde avec Paper, Geyser et Floodgate.\n\nServeur démarré et vérifié par Monde Ensemble. Pour le relancer, utilise demarrer.bat ou demarrer.sh.\n\nLes consoles peuvent demander une méthode d’accès supplémentaire. Les mods qui exigent une installation chez les joueurs et les packs Java ne sont pas universels.\n"
    );
    fs::write(stage.join("LIRE AVANT DE JOUER.txt"), notes).map_err(io_error)
}

pub fn create_backup(
    source: &Path,
    output_file: &Path,
    server_stopped: bool,
    progress: impl Fn(&str, u8),
) -> Result<OperationResult, String> {
    if !server_stopped {
        return Err(
            "Arrête le serveur avant la sauvegarde, puis confirme qu’il est bien arrêté.".into(),
        );
    }
    let source = source
        .canonicalize()
        .map_err(|error| format!("Le dossier du serveur n’est pas accessible : {error}"))?;
    if !source.is_dir() {
        return Err("Choisis le dossier qui contient paper.jar ou server.properties.".into());
    }
    let output_parent = output_file
        .parent()
        .ok_or_else(|| "Choisis l’emplacement de la sauvegarde.".to_string())?;
    fs::create_dir_all(output_parent).map_err(io_error)?;
    let manifest_path = PathBuf::from(format!("{}.sha256", output_file.display()));
    if output_file.exists() || manifest_path.exists() {
        return Err("Ce nom de sauvegarde existe déjà. Choisis un autre nom pour conserver les deux copies.".into());
    }
    let total = folder_size(&source)?;
    let available = fs2::available_space(output_parent).map_err(io_error)?;
    if available < total.saturating_add(256 * 1024 * 1024) {
        return Err("Le disque choisi n’a pas assez d’espace pour une sauvegarde complète.".into());
    }
    let staging = output_file.with_extension(format!("{}.preparing", Uuid::new_v4()));
    let _cleanup = RemoveFileOnDrop(staging.clone());
    progress("Préparation de l’archive complète…", 5);
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .map_err(io_error)?;
    let encoder = GzEncoder::new(file, Compression::fast());
    let mut archive = Builder::new(encoder);
    let entries = WalkDir::new(&source)
        .follow_links(false)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Impossible de lire tout le serveur : {error}"))?;
    let count = entries.len().max(1);
    for (index, entry) in entries.iter().enumerate() {
        let kind = entry.file_type();
        if kind.is_symlink() {
            let _ = fs::remove_file(&staging);
            return Err(format!(
                "Le dossier contient un lien symbolique ({}) que la sauvegarde ne peut pas copier sûrement.",
                entry.path().display()
            ));
        }
        if entry.path() == source {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&source)
            .map_err(|error| error.to_string())?;
        let archive_path = Path::new("server").join(relative);
        if kind.is_dir() {
            archive
                .append_dir(archive_path, entry.path())
                .map_err(io_error)?;
        } else if kind.is_file() {
            archive
                .append_path_with_name(entry.path(), archive_path)
                .map_err(io_error)?;
        }
        progress(
            "Copie du monde, des réglages et des extensions…",
            5 + ((index * 80 / count) as u8),
        );
    }
    let encoder = archive.into_inner().map_err(io_error)?;
    encoder
        .finish()
        .map_err(io_error)?
        .sync_all()
        .map_err(io_error)?;
    progress("Contrôle de l’archive et calcul de son empreinte…", 90);
    let checksum = hash_file(&staging)?;
    // A hard link in the same folder publishes the complete file atomically
    // and fails instead of replacing a file created concurrently.
    fs::hard_link(&staging, output_file).map_err(|error| {
        let _ = fs::remove_file(&staging);
        format!("Impossible de créer la sauvegarde sans écraser un fichier existant : {error}")
    })?;
    fs::remove_file(&staging).map_err(io_error)?;
    let manifest = format!(
        "{checksum}  {}\n",
        output_file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    let manifest_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest_path)
        .map_err(|error| {
            let _ = fs::remove_file(output_file);
            format!("Impossible de créer le contrôle d’intégrité sans écraser un fichier : {error}")
        })?;
    use std::io::Write;
    (&manifest_file)
        .write_all(manifest.as_bytes())
        .map_err(|error| {
            let _ = fs::remove_file(&manifest_path);
            let _ = fs::remove_file(output_file);
            error.to_string()
        })?;
    progress("Sauvegarde vérifiée et terminée.", 100);
    Ok(OperationResult {
        path: output_file.display().to_string(),
        detail: format!("Archive complète · {} octets avant compression", total),
        sha256: Some(checksum),
    })
}

pub fn restore_backup(
    archive_path: &Path,
    destination_root: &Path,
    progress: impl Fn(&str, u8),
) -> Result<OperationResult, String> {
    let expected_checksum = PathBuf::from(format!("{}.sha256", archive_path.display()));
    let archive_hash_before = hash_file(archive_path)?;
    if expected_checksum.is_file() {
        let manifest = fs::read_to_string(&expected_checksum).map_err(io_error)?;
        let expected = manifest.split_whitespace().next().unwrap_or_default();
        if expected.len() != 64 || !expected.eq_ignore_ascii_case(&archive_hash_before) {
            return Err(
                "Le contrôle d’intégrité de la sauvegarde a échoué. Restauration bloquée.".into(),
            );
        }
    }
    let source = File::open(archive_path)
        .map_err(|error| format!("La sauvegarde choisie est inaccessible : {error}"))?;
    let decoder = GzDecoder::new(source);
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| format!("Cette sauvegarde ne peut pas être lue : {error}"))?;
    let mut validated = Vec::new();
    let mut paths = HashSet::new();
    let mut total = 0u64;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Archive endommagée : {error}"))?;
        let kind = entry.header().entry_type();
        if kind != EntryType::Regular && kind != EntryType::Directory {
            return Err("La sauvegarde contient un type de fichier spécial ou un lien; restauration bloquée.".into());
        }
        let path = entry
            .path()
            .map_err(|error| error.to_string())?
            .into_owned();
        if !safe_archive_path(&path) {
            return Err(
                "La sauvegarde contient un chemin dangereux; aucune restauration n’a été faite."
                    .into(),
            );
        }
        if !paths.insert(path.clone()) {
            return Err(
                "La sauvegarde contient des chemins en double; restauration bloquée.".into(),
            );
        }
        let entry_size = entry.size();
        total = total.saturating_add(entry_size);
        if total > MAX_RESTORE_BYTES {
            return Err("La sauvegarde dépasse la taille maximale acceptée.".into());
        }
        validated.push((path, kind, entry_size));
    }
    if validated.is_empty()
        || !validated
            .iter()
            .any(|(path, _, _)| path.starts_with("server/"))
    {
        return Err("Cette sauvegarde n’a pas la structure attendue (dossier server).".into());
    }
    let root = prepare_root(destination_root)?;
    let staging = root.join(format!(".restauration-{}.preparing", Uuid::new_v4()));
    fs::create_dir(&staging).map_err(io_error)?;
    progress("Sauvegarde vérifiée; préparation d’une nouvelle copie…", 15);
    let result = extract_validated(archive_path, &staging, &validated, &progress);
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let restored_root = staging.join("server");
    if !restored_root.join("server.properties").is_file()
        && !restored_root.join("paper.jar").is_file()
    {
        let _ = fs::remove_dir_all(&staging);
        return Err("La sauvegarde ne contient pas de fichiers de serveur reconnus.".into());
    }
    let final_path = unique_folder(&root, "Monde-Ensemble-Restaure");
    fs::rename(&staging, &final_path).map_err(io_error)?;
    let path = final_path.join("server");
    progress("Restauration terminée dans un nouveau dossier.", 100);
    let archive_hash_after = hash_file(archive_path)?;
    if archive_hash_after != archive_hash_before {
        let _ = fs::remove_dir_all(&final_path);
        return Err(
            "La sauvegarde a changé pendant la restauration; la nouvelle copie a été retirée."
                .into(),
        );
    }
    Ok(OperationResult {
        path: path.display().to_string(),
        detail: format!("Monde restauré · {total} octets décompressés · l’original reste intact"),
        sha256: Some(archive_hash_after),
    })
}

fn extract_validated(
    archive_path: &Path,
    staging: &Path,
    validated: &[(PathBuf, EntryType, u64)],
    progress: &impl Fn(&str, u8),
) -> Result<(), String> {
    let source = File::open(archive_path).map_err(io_error)?;
    let mut archive = Archive::new(GzDecoder::new(source));
    let count = validated.len().max(1);
    let mut extracted_count = 0usize;
    for (index, entry) in archive.entries().map_err(io_error)?.enumerate() {
        let entry = entry.map_err(io_error)?;
        let path = entry.path().map_err(io_error)?.into_owned();
        if !safe_archive_path(&path) {
            return Err("Chemin d’archive modifié pendant la restauration.".into());
        }
        let (expected_path, expected_kind, expected_size) = validated
            .get(index)
            .ok_or_else(|| "L’archive a changé pendant la restauration.".to_string())?;
        if path != *expected_path
            || entry.header().entry_type() != *expected_kind
            || entry.size() != *expected_size
        {
            return Err("Le contenu de l’archive a changé après sa vérification.".into());
        }
        let relative = path.strip_prefix("server").unwrap_or(&path);
        if relative.as_os_str().is_empty() {
            continue;
        }
        let destination = staging.join("server").join(relative);
        if !destination.starts_with(staging) {
            return Err("Chemin hors du dossier de restauration; arrêt immédiat.".into());
        }
        if entry.header().entry_type() == EntryType::Directory {
            fs::create_dir_all(&destination).map_err(io_error)?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(io_error)?;
            }
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(io_error)?;
            let copied = io::copy(
                &mut entry.take(expected_size.saturating_add(1)),
                &mut output,
            )
            .map_err(io_error)?;
            if copied != *expected_size {
                return Err("La taille d’un fichier a changé pendant la restauration.".into());
            }
        }
        extracted_count += 1;
        progress(
            "Restauration du monde et des réglages…",
            15 + ((index * 75 / count) as u8),
        );
    }
    if extracted_count != validated.len() {
        return Err("Des éléments de l’archive manquent pendant la restauration.".into());
    }
    Ok(())
}

fn safe_archive_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .next()
            .is_some_and(|component| component == Component::Normal(std::ffi::OsStr::new("server")))
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn folder_size(path: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.file_type().is_symlink() {
            return Err(format!(
                "Le dossier du serveur contient un lien symbolique : {}",
                entry.path().display()
            ));
        }
        if entry.file_type().is_file() {
            total = total.saturating_add(entry.metadata().map_err(io_error)?.len());
        }
    }
    Ok(total)
}

fn unpack_runtime(archive_path: &Path, destination: PathBuf) -> Result<(), String> {
    fs::create_dir_all(&destination).map_err(io_error)?;
    if archive_path.extension().is_some_and(|ext| ext == "zip") {
        let mut archive = zip::ZipArchive::new(File::open(archive_path).map_err(io_error)?)
            .map_err(|error| format!("Archive Java invalide : {error}"))?;
        for index in 0..archive.len() {
            let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
            let Some(relative) = file.enclosed_name().map(PathBuf::from) else {
                return Err("Archive Java contient un chemin invalide.".into());
            };
            let target = destination.join(relative);
            if file.is_dir() {
                fs::create_dir_all(target).map_err(io_error)?;
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(io_error)?;
                }
                let mut output = File::create(target).map_err(io_error)?;
                io::copy(&mut file, &mut output).map_err(io_error)?;
            }
        }
    } else {
        let decoder = GzDecoder::new(File::open(archive_path).map_err(io_error)?);
        let mut archive = Archive::new(decoder);
        for item in archive.entries().map_err(io_error)? {
            let mut item = item.map_err(io_error)?;
            let path = item.path().map_err(io_error)?.into_owned();
            if !safe_archive_path(&path)
                || item.header().entry_type().is_symlink()
                || item.header().entry_type().is_hard_link()
            {
                return Err("Archive Java contient un chemin ou un lien invalide.".into());
            }
            let destination_path = destination.join(path);
            if !destination_path.starts_with(&destination) {
                return Err("Chemin Java hors du dossier prévu.".into());
            }
            item.unpack(destination_path).map_err(io_error)?;
        }
    }
    let java = WalkDir::new(&destination)
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| {
            entry.file_type().is_file()
                && (entry.file_name() == "java" || entry.file_name() == "java.exe")
        })
        .ok_or_else(|| "Le runtime téléchargé ne contient pas Java.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(java.path()).map_err(io_error)?.permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        fs::set_permissions(java.path(), permissions).map_err(io_error)?;
    }
    Ok(())
}

fn download_verified(
    client: &Client,
    artifact: &Artifact,
    destination: &Path,
) -> Result<(), String> {
    download_url_verified(client, &artifact.url, &artifact.checksum, destination)
        .map_err(|error| format!("{} ({}) : {error}", artifact.name, artifact.version))
}

fn download_url_verified(
    client: &Client,
    url: &str,
    checksum: &str,
    destination: &Path,
) -> Result<(), String> {
    let response = client
        .get(url)
        .send()
        .map_err(network_error)?
        .error_for_status()
        .map_err(network_error)?;
    if !response.url().scheme().eq("https") {
        return Err("Téléchargement redirigé hors HTTPS; arrêt sûr.".into());
    }
    let mut response = response;
    let mut file = File::create(destination).map_err(io_error)?;
    io::copy(&mut response, &mut file).map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    let actual = hash_file(destination)?;
    if !actual.eq_ignore_ascii_case(checksum) {
        let _ = fs::remove_file(destination);
        return Err("Le contrôle SHA-256 a échoué. Le fichier n’a pas été installé.".into());
    }
    if destination.extension().is_some_and(|ext| ext == "jar") {
        let jar = zip::ZipArchive::new(File::open(destination).map_err(io_error)?)
            .map_err(|error| format!("Fichier serveur invalide : {error}"))?;
        if jar.len() == 0 {
            return Err("Le fichier téléchargé est vide.".into());
        }
    }
    Ok(())
}

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent("MondeEnsemble/0.3")
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| error.to_string())
}

fn get_text(client: &Client, url: &str) -> Result<String, String> {
    client
        .get(url)
        .send()
        .map_err(network_error)?
        .error_for_status()
        .map_err(network_error)?
        .text()
        .map_err(network_error)
}

fn get_json<T: for<'de> Deserialize<'de>>(client: &Client, url: &str) -> Result<T, String> {
    client
        .get(url)
        .send()
        .map_err(network_error)?
        .error_for_status()
        .map_err(network_error)?
        .json()
        .map_err(|error| format!("Réponse catalogue illisible : {error}"))
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn prepare_root(root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("Impossible d’utiliser le dossier choisi : {error}"))?;
    root.canonicalize().map_err(io_error)
}

fn unique_folder(root: &Path, prefix: &str) -> PathBuf {
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let base = format!("{prefix}-{stamp}");
    let mut candidate = root.join(&base);
    let mut suffix = 2u32;
    while candidate.exists() {
        candidate = root.join(format!("{base}-{suffix}"));
        suffix += 1;
    }
    candidate
}

fn safe_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-')
}

fn network_error(error: reqwest::Error) -> String {
    format!("Catalogue ou téléchargement indisponible : {error}")
}

fn io_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
