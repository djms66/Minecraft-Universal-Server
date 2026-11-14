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
use sha2::{Digest, Sha256, Sha512};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    time::Duration,
};
use tar::{Archive, Builder, EntryType};
use uuid::Uuid;
use walkdir::WalkDir;

/// Minimum resources required by the shared Java/Bedrock server profile.
pub const MINIMUM_SERVER_MEMORY_GB: f64 = 2.0;
/// A Pi with 1 GB physical RAM reports about 0.9 GiB in /proc/meminfo.
/// This is the lower bound for the automatically selected compact Pi profile.
const MINIMUM_PI_MEMORY_GB: f64 = 0.8;
const COMPACT_PI_MEMORY_GB: f64 = 2.0;
const COMPACT_PI_HEAP_MB: u16 = 512;
pub const INITIAL_SERVER_STORAGE_GB: f64 = 8.0;

const JAVA_DEFAULT_PORT: u16 = 25565;
const BEDROCK_DEFAULT_PORT: u16 = 19132;
// GeyserMC announced NetherNet support beginning at build 1237; Bedrock 26.60+
// requires this transport. Older builds are not safe for managed cross-play.
const MIN_GEYSER_NETHERNET_BUILD: u64 = 1237;
const JAVA_ALTERNATE_PORT: u16 = 25566;
const BEDROCK_ALTERNATE_PORT: u16 = 19133;
const GEYSER_PLUGIN_JAR: &str = "Geyser-Spigot.jar";
const FLOODGATE_PLUGIN_JAR: &str = "Floodgate-Spigot.jar";
const PAPER_READY_LOG_PATTERN: &str = r"INFO\]: Done \(";
const MAX_RESTORE_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
const REMOTE_SERVER_ARTIFACT_CHECK: &str = "for f in paper.jar plugins/Geyser-Spigot.jar plugins/Floodgate-Spigot.jar plugins/Geyser-Spigot/config.yml; do if [ ! -s \"$f\" ]; then printf 'Fichier requis absent ou vide : %s\\n' \"$f\" >&2; exit 42; fi; done; if ! grep -Eq '^[[:space:]]*transport:[[:space:]]*nethernet([[:space:]#]|$)' plugins/Geyser-Spigot/config.yml; then echo 'Configuration Geyser sans NetherNet.' >&2; exit 43; fi;";
static APP_DATA_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

/// Stores the app-owned location used for first-boot Raspberry Pi SSH access.
pub fn configure_app_data_directory(path: PathBuf) {
    let _ = APP_DATA_DIRECTORY.set(path);
}

/// Runs SSH with the app's Pi key when one has been provisioned. Existing
/// OpenSSH identities remain available for Pi installations made earlier.
pub fn ssh_command() -> Command {
    let mut command = Command::new("ssh");
    add_managed_identity(&mut command);
    command
}

fn scp_command() -> Command {
    let mut command = Command::new("scp");
    add_managed_identity(&mut command);
    command
}

fn add_managed_identity(command: &mut Command) {
    let Some(directory) = APP_DATA_DIRECTORY.get() else {
        return;
    };
    let identity = directory.join("ssh").join("monde-ensemble-pi");
    add_identity_file(command, &identity);
}

fn add_identity_file(command: &mut Command, identity: &Path) {
    if identity.is_file() {
        command.arg("-i").arg(identity);
    }
}

const REMOTE_NETWORK_IDENTITY_COMMAND: &str = r#"network_interface="$(ip -4 route show default 2>/dev/null | awk 'NR == 1 { for (i = 1; i <= NF; i++) if ($i == "dev") { print $(i + 1); exit } }')"; network_ip=""; if [ -n "$network_interface" ]; then network_ip="$(ip -o -4 address show dev "$network_interface" scope global 2>/dev/null | awk 'NR == 1 { split($4, address, "/"); print address[1] }')"; fi; if [ -z "$network_ip" ]; then network_interface="$(ip -o -4 address show scope global up 2>/dev/null | awk 'NR == 1 { print $2 }')"; network_ip="$(ip -o -4 address show dev "$network_interface" scope global up 2>/dev/null | awk 'NR == 1 { split($4, address, "/"); print address[1] }')"; fi; network_mac=""; if [ -n "$network_interface" ] && [ -r "/sys/class/net/$network_interface/address" ]; then network_mac="$(cat "/sys/class/net/$network_interface/address")"; fi; printf 'IP=%s\nNETWORK_INTERFACE=%s\nMAC_ADDRESS=%s\n' "$network_ip" "$network_interface" "$network_mac""#;
const REMOTE_PROBE_COMMAND_TEMPLATE: &str = r#"printf 'MODEL='; tr -d '\000' </sys/firmware/devicetree/base/model 2>/dev/null || true; printf '\nMEMORY_GB='; awk '/MemTotal/ {printf "%.1f\n", $2/1048576}' /proc/meminfo; printf 'FREE_GB='; df -BG --output=avail "$HOME" | tail -1 | tr -dc '0-9'; printf '\nARCH='; uname -m; @NETWORK_IDENTITY@; for p in 25565 25566 19132 19133 25575 25576; do printf 'PORT_%s=' "$p"; if ss -H -lntu 2>/dev/null | awk '{print $5}' | grep -qE "[:.]$p$"; then echo busy; else echo free; fi; done"#;

fn remote_probe_command() -> String {
    REMOTE_PROBE_COMMAND_TEMPLATE.replace("@NETWORK_IDENTITY@", REMOTE_NETWORK_IDENTITY_COMMAND)
}

fn available_game_port(primary: u16, alternate: u16, udp: bool) -> Result<u16, String> {
    let can_bind = |port| {
        if udp {
            // Bedrock on recent clients uses UDP for gameplay and TCP for
            // NetherNet signaling on the same port. Reserve only a port that
            // is available for both transports before creating the profile.
            UdpSocket::bind(("0.0.0.0", port)).is_ok()
                && TcpListener::bind(("0.0.0.0", port)).is_ok()
        } else {
            TcpListener::bind(("0.0.0.0", port)).is_ok()
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
    pub paper_build_id: u64,
    pub geyser_version: String,
    pub geyser_build_id: u64,
    pub floodgate_version: String,
    pub floodgate_build_id: u64,
    pub bedrock_versions: String,
    pub java_version: String,
    pub checked_at: String,
    pub source: String,
}

#[derive(Serialize)]
pub struct PiUpdateCheck {
    pub ready: bool,
    pub update_available: bool,
    pub bedrock_transport_migration: bool,
    pub current_java: String,
    pub current_bedrock: String,
    pub current_geyser: String,
    pub current_floodgate: String,
    pub latest_java: String,
    pub latest_bedrock: String,
    pub latest_geyser: String,
    pub latest_geyser_build: u64,
    pub latest_floodgate: String,
    pub latest_floodgate_build: u64,
    pub latest_paper_build: u64,
    pub profile: serde_json::Value,
    pub profile_sha256: String,
    pub paper_sha256: String,
    pub geyser_sha256: String,
    pub floodgate_sha256: String,
    pub reasons: Vec<String>,
    pub checked_at: String,
}

enum PiUpdateFailure {
    Safe(String),
    Uncertain(String),
}

impl From<String> for PiUpdateFailure {
    fn from(value: String) -> Self {
        Self::Safe(value)
    }
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

#[derive(Clone, Serialize)]
pub struct PiOsRecommendation {
    pub name: String,
    pub release_date: String,
    pub image_url: String,
    pub image_sha256: String,
    pub download_sha256: Option<String>,
    pub download_size_gb: f64,
    pub image_size_gb: f64,
    pub devices: Vec<String>,
    pub init_format: String,
    pub minimum_server_memory_gb: f64,
    pub initial_server_storage_gb: f64,
    pub checked_at: String,
}

#[derive(Serialize)]
pub struct PiOsCatalog {
    pub recommended: PiOsRecommendation,
    pub options: Vec<PiOsRecommendation>,
}

#[derive(Serialize)]
pub struct PiServerStatus {
    pub running: bool,
    pub ready: bool,
    pub java_port_open: bool,
    pub bedrock_tcp_port_open: bool,
    pub bedrock_port_open: bool,
    pub bedrock_nethernet_ready: bool,
    pub autostart_configured: bool,
    pub minecraft_java: String,
    pub minecraft_bedrock: String,
    pub paper_build_id: u64,
    pub geyser_version: String,
    pub geyser_build_id: u64,
    pub floodgate_version: String,
    pub floodgate_build_id: u64,
    pub java_port: u16,
    pub bedrock_port: u16,
    pub address: String,
    pub network_interface: String,
    pub mac_address: String,
    pub log: String,
}

#[derive(Serialize)]
pub struct LocalServerStatus {
    pub running: bool,
    pub ready: bool,
    pub java_port_open: bool,
    pub minecraft_java: String,
    pub minecraft_bedrock: String,
    pub java_port: u16,
    pub bedrock_port: u16,
    pub bedrock_tcp_port_open: bool,
    pub bedrock_udp_port_open: bool,
    pub bedrock_nethernet_ready: bool,
    pub address: String,
    pub players: String,
    pub log: String,
}

pub fn local_game_settings(server_dir: &Path) -> Result<GameSettings, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier du serveur local enregistré est introuvable.".to_string())?;
    local_profile(&server_dir)?;
    let properties = read_server_properties(&server_dir.join("server.properties"))?;
    parse_game_settings(&properties)
}

pub fn save_local_game_settings(
    server_dir: &Path,
    settings: &GameSettings,
) -> Result<String, String> {
    validate_game_settings(settings)?;
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier du serveur local enregistré est introuvable.".to_string())?;
    local_profile(&server_dir)?;
    if inspect_local_server(&server_dir)?.running {
        return Err(
            "Arrête le serveur depuis son tableau de bord avant de modifier ses réglages.".into(),
        );
    }
    let properties_path = server_dir.join("server.properties");
    let previous = read_server_properties(&properties_path)?;
    let updated = apply_game_settings(&previous, settings)?;
    let backup_path = server_dir.join(format!(
        "server.properties.before-settings-{}",
        Uuid::new_v4().simple()
    ));
    write_new_private_file(&backup_path, previous.as_bytes())?;
    let staged_path = server_dir.join(format!(
        ".server.properties.{}.tmp",
        Uuid::new_v4().simple()
    ));
    let _cleanup = RemoveFileOnDrop(staged_path.clone());
    write_new_private_file(&staged_path, updated.as_bytes())?;
    fs::rename(&staged_path, &properties_path).map_err(|error| {
        format!(
            "Les réglages n’ont pas été appliqués. La copie précédente est conservée dans {} : {error}",
            backup_path.display()
        )
    })?;
    Ok(format!(
        "Réglages enregistrés. Le serveur doit être redémarré pour les appliquer. Copie de secours : {}",
        backup_path.display()
    ))
}

pub fn pi_game_settings(
    host: &str,
    username: &str,
    server_folder: &str,
) -> Result<GameSettings, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Adresse ou dossier du serveur Pi invalide.".into());
    }
    let command = format!(
        "set -eu; d=\"$HOME/{server_folder}\"; test -d \"$d\" && test ! -L \"$d\"; test -f \"$d/monde-ensemble-profile.json\" && test ! -L \"$d/monde-ensemble-profile.json\"; test -f \"$d/server.properties\" && test ! -L \"$d/server.properties\"; test \"$(wc -c < \"$d/server.properties\")\" -le 1048576; python3 -c 'import json,sys; p=json.load(open(sys.argv[1])); assert p.get(\"application\") == \"Monde Ensemble\"' \"$d/monde-ensemble-profile.json\"; cat -- \"$d/server.properties\""
    );
    let properties = run_ssh(&format!("{username}@{host}"), &command)?;
    parse_game_settings(&properties)
}

pub fn save_pi_game_settings(
    host: &str,
    username: &str,
    server_folder: &str,
    settings: &GameSettings,
) -> Result<String, String> {
    validate_game_settings(settings)?;
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Adresse ou dossier du serveur Pi invalide.".into());
    }
    if inspect_pi_server(host, username, server_folder)?.running {
        return Err(
            "Arrête le serveur Pi depuis son tableau de bord avant de modifier ses réglages."
                .into(),
        );
    }
    let settings_json = serde_json::to_string(settings).map_err(|error| error.to_string())?;
    let backup_id = Uuid::new_v4().simple().to_string();
    let command = format!(
        r#"set -eu
umask 077
d="$HOME/{server_folder}"
test -d "$d" && test ! -L "$d"
test -f "$d/monde-ensemble-profile.json" && test ! -L "$d/monde-ensemble-profile.json"
test -f "$d/server.properties" && test ! -L "$d/server.properties"
pid="$(cat "$d/serveur.pid" 2>/dev/null || true)"
case "$pid" in ''|*[!0-9]*) pid=;; esac
if test -n "$pid" && kill -0 "$pid" 2>/dev/null && test "$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)" = "$(readlink -f "$d")" && tr -d '\000' < "/proc/$pid/cmdline" 2>/dev/null | grep -q 'paper.jar'; then echo SERVER_RUNNING; exit 0; fi
python3 - "$d" {settings_json} {backup_id} <<'PYCODE'
import json, os, pathlib, shutil, sys, tempfile
root=pathlib.Path(sys.argv[1]); settings=json.loads(sys.argv[2]); suffix=sys.argv[3]
profile=root/'monde-ensemble-profile.json'; props=root/'server.properties'
data=json.loads(profile.read_text(encoding='utf-8'))
if data.get('application') != 'Monde Ensemble': raise SystemExit('PROFILE_NOT_MANAGED')
if props.stat().st_size > 1048576: raise SystemExit('PROPERTIES_TOO_LARGE')
lines=props.read_text(encoding='utf-8').splitlines(); keys={{'motd':'motd','gamemode':'gamemode','difficulty':'difficulty','max_players':'max-players','view_distance':'view-distance','simulation_distance':'simulation-distance','pvp':'pvp','allow_nether':'allow-nether'}}
values={{key:str(settings[field]).lower() if isinstance(settings[field], bool) else str(settings[field]) for field,key in keys.items()}}
backup=root/f'server.properties.before-settings-{{suffix}}'
with backup.open('xb') as out, props.open('rb') as source: shutil.copyfileobj(source,out); out.flush(); os.fsync(out.fileno())
seen=set(); result=[]
for line in lines:
    key=line.split('=',1)[0]
    if key in values:
        if key not in seen: result.append(f'{{key}}={{values[key]}}'); seen.add(key)
    else: result.append(line)
for key,value in values.items():
    if key not in seen: result.append(f'{{key}}={{value}}')
fd,temp=tempfile.mkstemp(prefix='.server.properties.',dir=root)
try:
    with os.fdopen(fd,'w',encoding='utf-8',newline='\n') as out:
        out.write('\n'.join(result)+'\n'); out.flush(); os.fsync(out.fileno())
    os.chmod(temp,0o600); os.replace(temp,props); os.chmod(props,0o600)
except BaseException:
    try: os.unlink(temp)
    except FileNotFoundError: pass
    shutil.copy2(backup,props)
    raise
print('SETTINGS_SAVED')
print(f'BACKUP={{backup.name}}')
PYCODE"#
    );
    let command = command.replace(&settings_json, &shell_single_quote(&settings_json));
    let output = run_ssh(&format!("{username}@{host}"), &command)?;
    if output.lines().any(|line| line == "SERVER_RUNNING") {
        return Err(
            "Le serveur Pi a redémarré entre les contrôles. Rien n’a été modifié; arrête-le puis réessaie.".into(),
        );
    }
    if !output.lines().any(|line| line == "SETTINGS_SAVED") {
        return Err(format!(
            "Le Pi n’a pas confirmé l’enregistrement. Le réglage d’origine est conservé. {}",
            output.trim()
        ));
    }
    let backup = output
        .lines()
        .find_map(|line| line.strip_prefix("BACKUP="))
        .unwrap_or("copie de secours créée sur le Pi");
    Ok(format!(
        "Réglages enregistrés sur le Pi. Redémarre le serveur pour les appliquer. Copie de secours : {backup}"
    ))
}

/// Installs a Bedrock-format resource pack in Geyser's documented packs
/// directory. This does not convert Java packs or claim client compatibility.
pub fn install_local_bedrock_pack(
    server_dir: &Path,
    source: &Path,
) -> Result<BedrockPackInstallResult, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier du serveur local enregistré est introuvable.".to_string())?;
    local_profile(&server_dir)?;
    let pack = inspect_bedrock_resource_pack(source)?;
    let status = inspect_local_server(&server_dir)?;
    if status.running
        || status.java_port_open
        || status.bedrock_tcp_port_open
        || status.bedrock_udp_port_open
    {
        return Err("Arrête le serveur local depuis son tableau de bord avant d’ajouter un pack Bedrock. Aucun fichier n’a été ajouté.".into());
    }
    ensure_backup_ports_closed(&server_dir)?;
    let geyser_dir = server_dir.join("plugins/Geyser-Spigot");
    require_real_directory(&server_dir.join("plugins"), "Le dossier plugins")?;
    require_real_directory(&geyser_dir, "Le dossier Geyser")?;
    let geyser_jar = server_dir.join("plugins/Geyser-Spigot.jar");
    if hash_regular_file(&geyser_jar).is_none() {
        return Err(
            "Le plugin Geyser géré est absent ou invalide; le pack n’a pas été ajouté.".into(),
        );
    }
    let packs_dir = geyser_dir.join("packs");
    create_or_require_real_directory(&packs_dir, "Le dossier des packs Geyser")?;
    let destination = packs_dir.join(&pack.filename);
    if destination.exists() {
        return Err(format!(
            "Un fichier nommé {} existe déjà dans Geyser. Renomme le pack pour garder les deux copies.",
            pack.filename
        ));
    }
    let staging = packs_dir.join(format!(".monde-ensemble-{}.part", Uuid::new_v4().simple()));
    let _cleanup = RemoveFileOnDrop(staging.clone());
    copy_verified_pack(&pack, &staging)?;
    ensure_backup_ports_closed(&server_dir)?;
    fs::hard_link(&staging, &destination).map_err(|error| {
        format!(
            "Le pack n’a pas été publié sans écraser un fichier. Il reste dans son emplacement d’origine. {error}"
        )
    })?;
    let _ = fs::remove_file(&staging);
    Ok(BedrockPackInstallResult {
        name: pack.name,
        filename: pack.filename,
        minimum_engine_version: pack.minimum_engine_version,
        sha256: pack.sha256,
        destination: destination.display().to_string(),
    })
}

/// Transfers a validated Bedrock pack to the existing Geyser packs directory
/// on a managed Pi. A hidden staging file is checked and linked into place only
/// after the transfer hash and stopped-server checks pass.
pub fn install_pi_bedrock_pack(
    host: &str,
    username: &str,
    server_folder: &str,
    source: &Path,
) -> Result<BedrockPackInstallResult, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Adresse ou dossier du serveur Pi invalide.".into());
    }
    let pack = inspect_bedrock_resource_pack(source)?;
    let initial_status = inspect_pi_server(host, username, server_folder)?;
    if initial_status.running
        || initial_status.java_port_open
        || initial_status.bedrock_tcp_port_open
        || initial_status.bedrock_port_open
    {
        return Err("Arrête le serveur Pi depuis son tableau de bord avant d’ajouter un pack Bedrock. Aucun fichier n’a été transféré.".into());
    }

    let staging_name = format!(".monde-ensemble-pack-{}.part", Uuid::new_v4().simple());
    let preflight = format!(
        "set -eu; d=\"$HOME/{server_folder}\"; test -d \"$d\" && test ! -L \"$d\"; test -f \"$d/monde-ensemble-profile.json\" && test ! -L \"$d/monde-ensemble-profile.json\"; python3 -c 'import json,sys; assert json.load(open(sys.argv[1])).get(\"application\") == \"Monde Ensemble\"' \"$d/monde-ensemble-profile.json\"; test -d \"$d/plugins\" && test ! -L \"$d/plugins\"; test -d \"$d/plugins/Geyser-Spigot\" && test ! -L \"$d/plugins/Geyser-Spigot\"; test -s \"$d/plugins/Geyser-Spigot.jar\" && test ! -L \"$d/plugins/Geyser-Spigot.jar\"; mkdir -p \"$d/plugins/Geyser-Spigot/packs\"; test ! -L \"$d/plugins/Geyser-Spigot/packs\"; test ! -e \"$d/plugins/Geyser-Spigot/packs/{}\"; pid=\"$(cat \"$d/serveur.pid\" 2>/dev/null || true)\"; case \"$pid\" in ''|*[!0-9]*) pid=;; esac; if test -n \"$pid\" && kill -0 \"$pid\" 2>/dev/null && test \"$(readlink -f \"/proc/$pid/cwd\" 2>/dev/null || true)\" = \"$(readlink -f \"$d\")\" && tr -d '\\000' < \"/proc/$pid/cmdline\" 2>/dev/null | grep -q 'paper.jar'; then echo SERVER_RUNNING; exit 1; fi",
        pack.filename
    );
    let destination_ssh = format!("{username}@{host}");
    run_ssh(&destination_ssh, &preflight)?;

    let remote_stage =
        format!("{destination_ssh}:~/{server_folder}/plugins/Geyser-Spigot/packs/{staging_name}");
    let scp = scp_command()
        .args([
            "-p",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "PasswordAuthentication=no",
        ])
        .arg(&pack.source)
        .arg(&remote_stage)
        .output()
        .map_err(|_| {
            "L’outil de transfert sécurisé n’est pas disponible sur cet ordinateur.".to_string()
        })?;
    if !scp.status.success() {
        let cleanup = format!(
            "rm -f -- \"$HOME/{server_folder}/plugins/Geyser-Spigot/packs/{staging_name}\""
        );
        let _ = run_ssh(&destination_ssh, &cleanup);
        return Err("Le transfert du pack Bedrock a échoué. Le serveur n’a pas été modifié; vérifie la connexion au Pi et réessaie.".into());
    }

    let publish = format!(
        "set -eu; d=\"$HOME/{server_folder}\"; p=\"$d/plugins/Geyser-Spigot/packs\"; test -d \"$d\" && test ! -L \"$d\"; test -d \"$d/plugins\" && test ! -L \"$d/plugins\"; test -d \"$d/plugins/Geyser-Spigot\" && test ! -L \"$d/plugins/Geyser-Spigot\"; test -s \"$d/plugins/Geyser-Spigot.jar\" && test ! -L \"$d/plugins/Geyser-Spigot.jar\"; test -d \"$p\" && test ! -L \"$p\"; test ! -e \"$p/{}\"; test -f \"$p/{staging_name}\" && test ! -L \"$p/{staging_name}\"; test \"$(sha256sum \"$p/{staging_name}\" | cut -d' ' -f1)\" = '{}'; pid=\"$(cat \"$d/serveur.pid\" 2>/dev/null || true)\"; case \"$pid\" in ''|*[!0-9]*) pid=;; esac; if test -n \"$pid\" && kill -0 \"$pid\" 2>/dev/null && test \"$(readlink -f \"/proc/$pid/cwd\" 2>/dev/null || true)\" = \"$(readlink -f \"$d\")\" && tr -d '\\000' < \"/proc/$pid/cmdline\" 2>/dev/null | grep -q 'paper.jar'; then echo SERVER_RUNNING; exit 1; fi; ln \"$p/{staging_name}\" \"$p/{}\"; rm -- \"$p/{staging_name}\" || true; echo PACK_INSTALLED",
        pack.filename, pack.sha256, pack.filename
    );
    let output = match run_ssh(&destination_ssh, &publish) {
        Ok(output) => output,
        Err(error) => {
            let cleanup = format!(
                "rm -f -- \"$HOME/{server_folder}/plugins/Geyser-Spigot/packs/{staging_name}\""
            );
            let _ = run_ssh(&destination_ssh, &cleanup);
            return Err(format!(
                "Le Pi n’a pas confirmé l’installation du pack. Le monde et les autres fichiers restent en place. {error}"
            ));
        }
    };
    if !output.lines().any(|line| line == "PACK_INSTALLED") {
        return Err("Le Pi n’a pas confirmé l’installation du pack Bedrock. Le fichier est conservé dans sa zone de préparation pour diagnostic.".into());
    }
    let result_destination = format!(
        "~/{server_folder}/plugins/Geyser-Spigot/packs/{}",
        pack.filename
    );
    Ok(BedrockPackInstallResult {
        name: pack.name,
        filename: pack.filename,
        minimum_engine_version: pack.minimum_engine_version,
        sha256: pack.sha256,
        destination: result_destination,
    })
}

struct ValidatedBedrockPack {
    source: PathBuf,
    name: String,
    filename: String,
    minimum_engine_version: String,
    sha256: String,
}

fn inspect_bedrock_resource_pack(path: &Path) -> Result<ValidatedBedrockPack, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "Choisis un fichier .mcpack ou .zip accessible.".to_string())?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > 512 * 1024 * 1024
    {
        return Err("Le pack doit être un fichier normal de 1 octet à 512 Mo maximum.".into());
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Le nom du pack n’est pas lisible.".to_string())?
        .to_string();
    if !valid_bedrock_pack_filename(&filename) {
        return Err(
            "Le fichier doit se terminer par .mcpack ou .zip et son nom doit rester simple.".into(),
        );
    }

    let file = File::open(path).map_err(io_error)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|_| "Ce fichier n’est pas une archive de pack Bedrock reconnue.".to_string())?;
    if archive
        .file_names()
        .filter(|name| *name == "manifest.json")
        .count()
        != 1
    {
        return Err("Le pack doit contenir un seul manifest.json à sa racine.".into());
    }
    let mut manifest_file = archive
        .by_name("manifest.json")
        .map_err(|_| "Le manifeste du pack est absent.".to_string())?;
    if manifest_file.size() > 256 * 1024 {
        return Err("Le manifeste du pack dépasse 256 Ko; installation bloquée.".into());
    }
    let mut manifest_text = String::new();
    manifest_file
        .read_to_string(&mut manifest_text)
        .map_err(|_| "Le manifeste du pack n’est pas un texte UTF-8 lisible.".to_string())?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|_| "Le manifeste du pack n’est pas un JSON valide.".to_string())?;
    if manifest
        .get("format_version")
        .and_then(serde_json::Value::as_u64)
        != Some(2)
    {
        return Err("Seuls les manifests Bedrock version 2 sont acceptés.".into());
    }
    let header = manifest
        .get("header")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "Le manifeste Bedrock ne contient pas d’en-tête.".to_string())?;
    let name = header
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty() && name.chars().count() <= 100)
        .ok_or_else(|| "Le pack doit avoir un nom lisible de 100 caractères maximum.".to_string())?
        .to_string();
    let uuid = header
        .get("uuid")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "L’en-tête Bedrock ne contient pas d’identifiant valide.".to_string())?;
    Uuid::parse_str(uuid).map_err(|_| "L’identifiant du pack Bedrock est invalide.".to_string())?;
    validate_pack_version(header.get("version"))?;
    let minimum_engine_version =
        parse_pack_version(header.get("min_engine_version")).ok_or_else(|| {
            "Le manifeste ne déclare pas une version minimale Bedrock reconnue.".to_string()
        })?;
    let modules = manifest
        .get("modules")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Le manifeste ne contient pas de modules Bedrock.".to_string())?;
    let resource_module = modules.iter().any(|module| {
        module.get("type").and_then(serde_json::Value::as_str) == Some("resources")
            && module
                .get("uuid")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|uuid| Uuid::parse_str(uuid).is_ok())
            && validate_pack_version(module.get("version")).is_ok()
    });
    if !resource_module {
        return Err("Ce fichier n’est pas un pack de ressources Bedrock valide.".into());
    }
    let source = path.canonicalize().map_err(io_error)?;
    Ok(ValidatedBedrockPack {
        source,
        name,
        filename,
        minimum_engine_version,
        sha256: hash_file(path)?,
    })
}

fn validate_pack_version(value: Option<&serde_json::Value>) -> Result<(), String> {
    parse_pack_version(value)
        .map(|_| ())
        .ok_or_else(|| "Le manifeste contient une version de pack invalide.".into())
}

fn parse_pack_version(value: Option<&serde_json::Value>) -> Option<String> {
    let parts = value?.as_array()?;
    if parts.len() != 3 {
        return None;
    }
    let numbers = parts
        .iter()
        .map(serde_json::Value::as_u64)
        .collect::<Option<Vec<_>>>()?;
    Some(format!("{}.{}.{}", numbers[0], numbers[1], numbers[2]))
}

fn valid_bedrock_pack_filename(filename: &str) -> bool {
    let lowercase = filename.to_ascii_lowercase();
    (lowercase.ends_with(".mcpack") || lowercase.ends_with(".zip"))
        && filename.len() <= 160
        && filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-+ ".contains(&byte))
}

fn copy_verified_pack(pack: &ValidatedBedrockPack, destination: &Path) -> Result<(), String> {
    let mut source = File::open(&pack.source).map_err(io_error)?;
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(io_error)?;
    io::copy(&mut source, &mut target).map_err(io_error)?;
    target.sync_all().map_err(io_error)?;
    drop(target);
    if hash_file(destination)? != pack.sha256 {
        let _ = fs::remove_file(destination);
        return Err(
            "Le contrôle SHA-256 du pack Bedrock a échoué; aucun pack n’a été installé.".into(),
        );
    }
    Ok(())
}

fn require_real_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| format!("{label} est absent ou illisible; le serveur n’a pas été modifié."))?;
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "{label} n’est pas un dossier normal; opération bloquée."
        ));
    }
    Ok(())
}

fn create_or_require_real_directory(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(format!(
            "{label} n’est pas un dossier normal; opération bloquée."
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(io_error)
        }
        Err(_) => Err(format!("{label} est illisible; opération bloquée.")),
    }
}

fn read_server_properties(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "Le fichier server.properties est absent ou illisible.".to_string())?;
    if !metadata.file_type().is_file() || metadata.len() > 1024 * 1024 {
        return Err(
            "Le fichier server.properties n’est pas un fichier normal ou dépasse 1 Mo.".into(),
        );
    }
    fs::read_to_string(path)
        .map_err(|_| "Le fichier server.properties n’est pas un texte lisible.".into())
}

fn parse_game_settings(properties: &str) -> Result<GameSettings, String> {
    let value = |name: &str| {
        properties.lines().rev().find_map(|line| {
            line.split_once('=')
                .filter(|(key, _)| *key == name)
                .map(|(_, value)| value.trim())
        })
    };
    let parse_number = |name: &str, fallback: &str| -> Result<u16, String> {
        value(name)
            .unwrap_or(fallback)
            .parse::<u16>()
            .map_err(|_| format!("Le réglage {name} dans server.properties est invalide."))
    };
    let parse_bool = |name: &str, fallback: bool| -> Result<bool, String> {
        match value(name).unwrap_or(if fallback { "true" } else { "false" }) {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(format!(
                "Le réglage {name} dans server.properties est invalide."
            )),
        }
    };
    let settings = GameSettings {
        motd: value("motd").unwrap_or("A Minecraft Server").to_string(),
        gamemode: value("gamemode").unwrap_or("survival").to_string(),
        difficulty: value("difficulty").unwrap_or("easy").to_string(),
        max_players: parse_number("max-players", "20")?,
        view_distance: u8::try_from(parse_number("view-distance", "10")?)
            .map_err(|_| "La distance d’affichage Minecraft est invalide.".to_string())?,
        simulation_distance: u8::try_from(parse_number("simulation-distance", "10")?)
            .map_err(|_| "La distance de simulation Minecraft est invalide.".to_string())?,
        pvp: parse_bool("pvp", true)?,
        allow_nether: parse_bool("allow-nether", true)?,
        java_resource_pack_url: value("resource-pack").unwrap_or("").to_string(),
        java_resource_pack_sha1: value("resource-pack-sha1").unwrap_or("").to_string(),
        java_resource_pack_required: parse_bool("require-resource-pack", false)?,
    };
    validate_game_settings(&settings)?;
    Ok(settings)
}

fn validate_game_settings(settings: &GameSettings) -> Result<(), String> {
    if settings.motd.trim().is_empty()
        || settings.motd.len() > 120
        || settings.motd.chars().any(char::is_control)
    {
        return Err(
            "Le nom affiché doit contenir de 1 à 120 caractères sur une seule ligne.".into(),
        );
    }
    if !matches!(
        settings.gamemode.as_str(),
        "survival" | "creative" | "adventure" | "spectator"
    ) {
        return Err("Choisis un mode de jeu Minecraft reconnu.".into());
    }
    if !matches!(
        settings.difficulty.as_str(),
        "peaceful" | "easy" | "normal" | "hard"
    ) {
        return Err("Choisis une difficulté Minecraft reconnue.".into());
    }
    if !(1..=1000).contains(&settings.max_players) {
        return Err("Le nombre maximal de joueurs doit être compris entre 1 et 1000.".into());
    }
    if !(2..=16).contains(&settings.view_distance)
        || !(2..=16).contains(&settings.simulation_distance)
    {
        return Err("Les distances d’affichage et de simulation doivent être comprises entre 2 et 16 chunks.".into());
    }
    if settings.java_resource_pack_url.len() > 2048
        || settings
            .java_resource_pack_url
            .chars()
            .any(char::is_control)
    {
        return Err("L’adresse du pack Java doit faire 2048 caractères maximum et ne pas contenir de commande.".into());
    }
    if !settings.java_resource_pack_url.is_empty() {
        let url = reqwest::Url::parse(&settings.java_resource_pack_url)
            .map_err(|_| "Saisis une adresse HTTPS directe vers le pack Java.".to_string())?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(
                "Le pack Java doit utiliser une URL HTTPS directe, sans identifiant ni fragment."
                    .into(),
            );
        }
    } else if !settings.java_resource_pack_sha1.is_empty() || settings.java_resource_pack_required {
        return Err(
            "Ajoute d’abord une URL de pack Java avant son empreinte ou son obligation.".into(),
        );
    }
    if !settings.java_resource_pack_sha1.is_empty()
        && (settings.java_resource_pack_sha1.len() != 40
            || !settings
                .java_resource_pack_sha1
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(
            "L’empreinte SHA-1 Java doit contenir exactement 40 caractères hexadécimaux.".into(),
        );
    }
    Ok(())
}

fn apply_game_settings(properties: &str, settings: &GameSettings) -> Result<String, String> {
    validate_game_settings(settings)?;
    let updates = [
        ("motd", settings.motd.clone()),
        ("gamemode", settings.gamemode.clone()),
        ("difficulty", settings.difficulty.clone()),
        ("max-players", settings.max_players.to_string()),
        ("view-distance", settings.view_distance.to_string()),
        (
            "simulation-distance",
            settings.simulation_distance.to_string(),
        ),
        ("pvp", settings.pvp.to_string()),
        ("allow-nether", settings.allow_nether.to_string()),
        ("resource-pack", settings.java_resource_pack_url.clone()),
        (
            "resource-pack-sha1",
            settings.java_resource_pack_sha1.clone(),
        ),
        (
            "require-resource-pack",
            settings.java_resource_pack_required.to_string(),
        ),
    ];
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for line in properties.lines() {
        let key = line.split_once('=').map(|(key, _)| key).unwrap_or_default();
        if let Some((_, value)) = updates.iter().find(|(candidate, _)| *candidate == key) {
            if seen.insert(key.to_string()) {
                output.push(format!("{key}={value}"));
            }
        } else {
            output.push(line.to_string());
        }
    }
    for (key, value) in updates {
        if seen.insert(key.to_string()) {
            output.push(format!("{key}={value}"));
        }
    }
    Ok(format!("{}\n", output.join("\n")))
}

fn write_new_private_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(io_error)?;
    set_private_file_permissions(path)?;
    file.write_all(contents).map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    set_private_file_permissions(path)
}

fn set_server_property_values(properties: &str, updates: &[(&str, &str)]) -> String {
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for line in properties.lines() {
        let key = line.split_once('=').map(|(key, _)| key).unwrap_or_default();
        if let Some((_, value)) = updates.iter().find(|(candidate, _)| *candidate == key) {
            seen.insert(key.to_string());
            output.push(format!("{key}={value}"));
        } else {
            output.push(line.to_string());
        }
    }
    for (key, value) in updates {
        if seen.insert((*key).to_string()) {
            output.push(format!("{key}={value}"));
        }
    }
    format!("{}\n", output.join("\n"))
}

/// Gameplay options exposed in expert mode. Ports, authentication, RCON,
/// whitelist and world identity remain app-managed because other controls rely
/// on them for secure cross-play and safe updates.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GameSettings {
    pub motd: String,
    pub gamemode: String,
    pub difficulty: String,
    pub max_players: u16,
    pub view_distance: u8,
    pub simulation_distance: u8,
    pub pvp: bool,
    pub allow_nether: bool,
    pub java_resource_pack_url: String,
    pub java_resource_pack_sha1: String,
    pub java_resource_pack_required: bool,
}

#[derive(Serialize)]
pub struct LocalStartResult {
    pub server_path: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct LocalWhitelistStatus {
    pub enabled: bool,
    pub entries: String,
}

/// A Modrinth Paper plugin release whose metadata exactly matches the
/// currently managed Minecraft version. This is discovery only: metadata
/// cannot prove runtime behavior or Bedrock client compatibility.
#[derive(Serialize)]
pub struct PaperPluginSuggestion {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub version: String,
    pub downloads: u64,
    pub project_url: String,
    pub required_dependencies: usize,
}

#[derive(Serialize)]
pub struct PaperPluginInstallResult {
    pub server_path: String,
    pub backup_path: String,
    pub backup_sha256: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub sha512: String,
}

#[derive(Serialize)]
pub struct BedrockPackInstallResult {
    pub name: String,
    pub filename: String,
    pub minimum_engine_version: String,
    pub sha256: String,
    pub destination: String,
}

#[derive(Serialize)]
pub struct PiWhitelistStatus {
    pub enabled: bool,
    pub entries: String,
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
    sha256: String,
}

#[derive(Deserialize)]
struct RuntimeAsset {
    binary: RuntimeBinary,
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

struct RemoveDirOnDrop(PathBuf);

impl Drop for RemoveDirOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct RemoveDirectoryOnDrop(PathBuf);

impl Drop for RemoveDirectoryOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct RemoveDirectoryUnlessKept {
    path: PathBuf,
    keep: bool,
}

impl Drop for RemoveDirectoryUnlessKept {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub fn compatibility() -> Result<CompatibilityReport, String> {
    let client = client()?;
    let supported = supported_versions(&client)?;
    let paper = paper_build(&client, &supported.java)?;
    let geyser = bridge_build(&client, "geyser", "spigot")?;
    let floodgate = bridge_build(&client, "floodgate", "spigot")?;
    if geyser.build < MIN_GEYSER_NETHERNET_BUILD {
        return Err(format!(
            "Geyser build {} ne prend pas en charge le transport Bedrock NetherNet requis. Installation et mise à jour coordonnées bloquées.",
            geyser.build
        ));
    }

    Ok(CompatibilityReport {
        paper_version: format!("{} · build {}", supported.java, paper.id),
        paper_build_id: paper.id,
        geyser_version: format!("{} · build {}", geyser.version, geyser.build),
        geyser_build_id: geyser.build,
        floodgate_version: format!("{} · build {}", floodgate.version, floodgate.build),
        floodgate_build_id: floodgate.build,
        bedrock_versions: supported.bedrock,
        java_version: supported.java,
        checked_at: Utc::now().to_rfc3339(),
        source: "PaperMC et GeyserMC officiels".into(),
    })
}

/// Reads official, verifiable 64-bit images that support unattended first boot.
/// The default remains the newest Raspberry Pi OS Lite recommendation; expert
/// options are limited to catalog entries compatible with Pi 3, 4, or 5.
pub fn pi_os_catalog() -> Result<PiOsCatalog, String> {
    const MANIFEST_URL: &str = "https://downloads.raspberrypi.com/os_list_imagingutility_v4.json";
    let client = client()?;
    let manifest: RaspberryPiOsManifest = get_json(&client, MANIFEST_URL)?;
    let mut entries = Vec::new();
    flatten_pi_os_entries(manifest.os_list, &mut entries);
    let mut options: Vec<_> = entries
        .into_iter()
        .filter_map(pi_os_entry_to_option)
        .collect();
    options.sort_by(|left, right| {
        right
            .release_date
            .cmp(&left.release_date)
            .then_with(|| left.name.cmp(&right.name))
    });
    let recommended = options
        .iter()
        .find(|option| {
            option
                .name
                .to_ascii_lowercase()
                .starts_with("raspberry pi os lite (64-bit)")
                && [3, 4, 5].into_iter().all(|generation| {
                    option.devices.iter().any(|device| {
                        pi_os_device_generation(device) == Some(generation)
                    })
                })
        })
        .cloned()
        .ok_or_else(|| {
        "Le catalogue officiel ne fournit pas d’image Raspberry Pi OS Lite 64 bits avec configuration de premier démarrage vérifiable. Préparation bloquée.".to_string()
    })?;
    Ok(PiOsCatalog {
        recommended,
        options: std::mem::take(&mut options),
    })
}

fn pi_os_entry_to_option(image: RaspberryPiOsEntry) -> Option<PiOsRecommendation> {
    let name = image.name;
    let url = image.url?;
    if !url.starts_with("https://downloads.raspberrypi.com/") {
        return None;
    }
    let image_url = url;
    let image_sha256 = image.extract_sha256.filter(|hash| valid_sha256(hash))?;
    let download_size = image.image_download_size.filter(|size| *size > 0)?;
    let image_size = image.extract_size.filter(|size| *size > 0)?;
    let release_date = image
        .release_date
        .filter(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok())?;
    let devices = image.devices?;
    let supported_devices: Vec<_> = devices
        .into_iter()
        .filter(|device| pi_os_device_generation(device).is_some())
        .collect();
    if supported_devices.is_empty() || image.init_format.as_deref() != Some("cloudinit-rpi") {
        return None;
    }
    let lower_name = name.to_ascii_lowercase();
    let minimum_server_memory_gb = if lower_name.contains("lite") || lower_name.contains("server") {
        1.0
    } else {
        4.0
    };
    Some(PiOsRecommendation {
        name,
        release_date,
        image_url,
        image_sha256,
        download_sha256: image
            .image_download_sha256
            .filter(|hash| valid_sha256(hash)),
        download_size_gb: (download_size as f64 / 1_000_000_000.0 * 10.0).round() / 10.0,
        image_size_gb: (image_size as f64 / 1_000_000_000.0 * 10.0).round() / 10.0,
        devices: supported_devices,
        init_format: image.init_format.unwrap_or_default(),
        minimum_server_memory_gb,
        initial_server_storage_gb: INITIAL_SERVER_STORAGE_GB,
        checked_at: Utc::now().to_rfc3339(),
    })
}

pub fn pi_os_device_generation(device: &str) -> Option<u8> {
    if !device.to_ascii_lowercase().contains("64bit") {
        return None;
    }
    [3, 4, 5].into_iter().find(|generation| {
        device
            .to_ascii_lowercase()
            .starts_with(&format!("pi{generation}"))
    })
}

/// Searches Modrinth without changing a server. Results must be a listed
/// plugin project with an exact Minecraft version, a Paper release, and a
/// server-side environment declaration. Required dependencies remain visible
/// as a count so the UI can avoid implying that these releases are one-click
/// installable.
pub fn search_paper_plugins(
    minecraft_version: &str,
    query: &str,
) -> Result<Vec<PaperPluginSuggestion>, String> {
    let valid_version = Regex::new(r"^\d+\.\d+(?:\.\d+)?$").unwrap();
    if !valid_version.is_match(minecraft_version) {
        return Err(
            "La version Minecraft du serveur n’est pas reconnue; catalogue non consulté.".into(),
        );
    }
    let query = query.trim();
    if query.chars().count() > 80 || query.chars().any(char::is_control) {
        return Err("La recherche doit faire 80 caractères maximum.".into());
    }

    let client = client()?;
    let facets = serde_json::to_string(&[
        vec![format!("versions:{minecraft_version}")],
        vec!["all_project_types:plugin".to_string()],
    ])
    .map_err(|error| error.to_string())?;
    let response: serde_json::Value = client
        .get("https://api.modrinth.com/v2/search")
        .query(&[
            ("query", query),
            ("facets", facets.as_str()),
            ("limit", "20"),
            ("index", "downloads"),
        ])
        .send()
        .map_err(network_error)?
        .error_for_status()
        .map_err(network_error)?
        .json()
        .map_err(|error| format!("Réponse du catalogue Modrinth illisible : {error}"))?;
    let hits = response
        .get("hits")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            "Le catalogue Modrinth n’a pas renvoyé de résultats exploitables.".to_string()
        })?;

    let mut suggestions = Vec::new();
    for hit in hits {
        let Some(project_id) = hit.get("project_id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if project_id.len() > 64 || !project_id.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            continue;
        }
        if !hit
            .get("all_project_types")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|types| types.iter().any(|kind| kind.as_str() == Some("plugin")))
        {
            continue;
        }
        let versions: Vec<serde_json::Value> = client
            .get(format!(
                "https://api.modrinth.com/v2/project/{project_id}/version"
            ))
            .query(&[
                ("loaders", "[\"paper\"]"),
                (
                    "game_versions",
                    format!("[\"{minecraft_version}\"]").as_str(),
                ),
                ("include_changelog", "false"),
            ])
            .send()
            .map_err(network_error)?
            .error_for_status()
            .map_err(network_error)?
            .json()
            .map_err(|error| format!("Versions de plugin illisibles : {error}"))?;
        let Some(version) = versions.into_iter().find(|version| {
            version.get("status").and_then(serde_json::Value::as_str) == Some("listed")
                && version
                    .get("version_type")
                    .and_then(serde_json::Value::as_str)
                    == Some("release")
                && version
                    .get("loaders")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("paper")))
                && version
                    .get("game_versions")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|items| {
                        items
                            .iter()
                            .any(|item| item.as_str() == Some(minecraft_version))
                    })
                && matches!(
                    version
                        .get("environment")
                        .and_then(serde_json::Value::as_str),
                    Some("server_only" | "server_only_client_optional" | "dedicated_server_only")
                )
        }) else {
            continue;
        };
        let required_dependencies = version
            .get("dependencies")
            .and_then(serde_json::Value::as_array)
            .map(|dependencies| {
                dependencies
                    .iter()
                    .filter(|dependency| {
                        dependency
                            .get("dependency_type")
                            .and_then(serde_json::Value::as_str)
                            == Some("required")
                    })
                    .count()
            })
            .unwrap_or_default();
        suggestions.push(PaperPluginSuggestion {
            project_id: project_id.to_string(),
            title: hit
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Plugin sans titre")
                .to_string(),
            description: hit
                .get("description")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            version: version
                .get("version_number")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("version inconnue")
                .to_string(),
            downloads: hit
                .get("downloads")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            project_url: format!("https://modrinth.com/plugin/{project_id}"),
            required_dependencies,
        });
        if suggestions.len() == 10 {
            break;
        }
    }
    Ok(suggestions)
}

/// Adds a Modrinth plugin to a complete sibling copy of a stopped, managed
/// local server. The original folder is never changed. Only exact-version,
/// listed Paper releases without required dependencies are accepted.
pub fn install_paper_plugin_copy(
    server_dir: &Path,
    project_id: &str,
    version_number: &str,
    progress: impl Fn(&str, u8),
) -> Result<PaperPluginInstallResult, String> {
    if project_id.is_empty()
        || project_id.len() > 64
        || !project_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
        || version_number.is_empty()
        || version_number.len() > 80
        || version_number.chars().any(char::is_control)
    {
        return Err("Le plugin sélectionné n’a pas une référence valide.".into());
    }
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier de serveur local enregistré est introuvable.".to_string())?;
    let profile = local_profile(&server_dir)?;
    require_local_rcon(&profile)?;
    if rcon_command(&profile, "list").is_ok() {
        return Err("Arrête le serveur depuis son tableau de bord avant d’ajouter un plugin. Les joueurs et le monde restent connectés.".into());
    }
    let java_port = profile
        .get("java_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .unwrap_or_default();
    let bedrock_port = profile
        .get("bedrock_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .unwrap_or_default();
    if java_port == 0
        || bedrock_port == 0
        || !tcp_port_available(java_port)
        || !tcp_port_available(bedrock_port)
        || !udp_port_available(bedrock_port)
    {
        return Err(
            "Un port Minecraft est actif. L’installation est bloquée sans toucher au serveur."
                .into(),
        );
    }
    let minecraft_version = profile
        .get("minecraft_java")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "La version Java du serveur est inconnue; ajout bloqué.".to_string())?
        .to_string();
    let valid_version = Regex::new(r"^\d+\.\d+(?:\.\d+)?$").unwrap();
    if !valid_version.is_match(&minecraft_version) {
        return Err("La version Java du serveur n’est pas reconnue; ajout bloqué.".into());
    }

    let plugins_dir = server_dir.join("plugins");
    let managed = profile
        .get("managed_plugins")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if profile.get("managed_plugins").is_some()
        && !profile
            .get("managed_plugins")
            .is_some_and(serde_json::Value::is_array)
    {
        return Err("Le manifeste des plugins gérés est illisible; ajout bloqué.".into());
    }
    let managed_files: HashSet<String> = managed
        .iter()
        .filter_map(|entry| entry.get("filename")?.as_str().map(str::to_string))
        .collect();
    for entry in &managed {
        let filename = entry
            .get("filename")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                "Le profil des plugins gérés est incomplet; le serveur reste intact.".to_string()
            })?;
        if !valid_plugin_filename(filename) {
            return Err("Le profil contient un nom de plugin invalide; ajout bloqué sans modifier le serveur.".into());
        }
    }
    for entry in fs::read_dir(&plugins_dir).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        if entry.file_type().map_err(io_error)?.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "jar")
        {
            let filename = entry.file_name().to_string_lossy().to_string();
            if !matches!(
                filename.as_str(),
                "Geyser-Spigot.jar" | "Floodgate-Spigot.jar"
            ) && !managed_files.contains(&filename)
            {
                return Err(format!(
                    "Le dossier contient déjà le plugin non géré {filename}; il reste intact et l’ajout est bloqué."
                ));
            }
        }
    }
    for entry in &managed {
        let filename = entry
            .get("filename")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                "Le profil des plugins gérés est incomplet; le serveur reste intact.".to_string()
            })?;
        let expected = entry
            .get("sha512")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                "L’empreinte d’un plugin géré manque; le serveur reste intact.".to_string()
            })?;
        if hash_file_sha512(&plugins_dir.join(filename)).as_deref() != Some(expected) {
            return Err(format!(
                "Le plugin {filename} a changé depuis son installation; ajout bloqué pour protéger ce serveur."
            ));
        }
    }

    progress("Vérification de la version exacte du plugin…", 5);
    let client = client()?;
    let versions: Vec<serde_json::Value> = client
        .get(format!(
            "https://api.modrinth.com/v2/project/{project_id}/version"
        ))
        .query(&[
            ("loaders", "[\"paper\"]"),
            (
                "game_versions",
                format!("[\"{minecraft_version}\"]").as_str(),
            ),
            ("include_changelog", "false"),
        ])
        .send()
        .map_err(network_error)?
        .error_for_status()
        .map_err(network_error)?
        .json()
        .map_err(|error| format!("Réponse de version Modrinth illisible : {error}"))?;
    let version = versions
        .into_iter()
        .find(|version| {
            version.get("version_number").and_then(serde_json::Value::as_str)
                == Some(version_number)
                && version.get("status").and_then(serde_json::Value::as_str) == Some("listed")
                && version.get("version_type").and_then(serde_json::Value::as_str) == Some("release")
                && version.get("loaders").and_then(serde_json::Value::as_array).is_some_and(|items| items.iter().any(|item| item.as_str() == Some("paper")))
                && version.get("game_versions").and_then(serde_json::Value::as_array).is_some_and(|items| items.iter().any(|item| item.as_str() == Some(minecraft_version.as_str())))
                && matches!(version.get("environment").and_then(serde_json::Value::as_str), Some("server_only" | "server_only_client_optional" | "dedicated_server_only"))
        })
        .ok_or_else(|| "Le catalogue ne confirme plus cette version exacte de Paper/Minecraft. Aucun fichier n’a été modifié.".to_string())?;
    if version
        .get("dependencies")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|dependencies| {
            dependencies.iter().any(|dependency| {
                dependency
                    .get("dependency_type")
                    .and_then(serde_json::Value::as_str)
                    == Some("required")
            })
        })
    {
        return Err("Cette version réclame une dépendance obligatoire qui n’est pas encore gérée par l’application; aucun fichier n’a été modifié.".into());
    }
    let files = version
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Le catalogue n’a pas fourni le fichier du plugin.".to_string())?;
    let file = files
        .iter()
        .find(|file| file.get("primary").and_then(serde_json::Value::as_bool) == Some(true))
        .or_else(|| files.first())
        .ok_or_else(|| "Le catalogue n’a pas fourni de fichier principal.".to_string())?;
    let filename = file
        .get("filename")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Le nom du fichier du plugin est invalide.".to_string())?;
    let url = file
        .get("url")
        .and_then(serde_json::Value::as_str)
        .filter(|url| url.starts_with("https://cdn.modrinth.com/"))
        .ok_or_else(|| "Le lien de téléchargement Modrinth n’est pas vérifiable.".to_string())?;
    let expected_sha512 = file
        .get("hashes")
        .and_then(|hashes| hashes.get("sha512"))
        .and_then(serde_json::Value::as_str)
        .filter(|hash| hash.len() == 128 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "Le catalogue ne fournit pas d’empreinte SHA-512 valide.".to_string())?;
    let size = file
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    if !valid_plugin_filename(filename) || size == 0 || size > 100 * 1024 * 1024 {
        return Err("Le fichier proposé n’est pas un plugin JAR de taille sûre; aucun fichier n’a été modifié.".into());
    }
    let destination_name = filename.to_string();
    if managed_files.contains(&destination_name) || plugins_dir.join(&destination_name).exists() {
        return Err("Un fichier porte déjà ce nom. Le serveur reste intact; aucun remplacement n’est autorisé.".into());
    }

    let parent = server_dir
        .parent()
        .ok_or_else(|| "Le dossier serveur n’a pas d’emplacement de copie valide.".to_string())?;
    let source_size = folder_size(&server_dir)?;
    let free = fs2::available_space(parent).map_err(io_error)?;
    if free
        < source_size
            .saturating_mul(2)
            .saturating_add(size)
            .saturating_add(512 * 1024 * 1024)
    {
        return Err("L’espace libre ne suffit pas pour garder l’original, une sauvegarde et une nouvelle copie complète. Aucun fichier n’a été modifié.".into());
    }
    let id = Uuid::new_v4().simple().to_string();
    let backup_path = parent
        .join("sauvegardes-monde-ensemble")
        .join(format!("avant-plugin-{id}.tar.gz"));
    progress("Sauvegarde complète avant ajout…", 12);
    create_backup(&server_dir, &backup_path, true, |step, percent| {
        progress(step, 12 + percent.saturating_mul(20) / 100)
    })?;

    progress("Téléchargement et contrôle SHA-512…", 35);
    let response = client
        .get(url)
        .send()
        .map_err(network_error)?
        .error_for_status()
        .map_err(network_error)?;
    if !response.url().scheme().eq("https")
        || !response
            .url()
            .host_str()
            .is_some_and(|host| host == "cdn.modrinth.com")
    {
        return Err("Le téléchargement a été redirigé hors du CDN officiel Modrinth; aucun plugin installé.".into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > size)
    {
        return Err("Le téléchargement dépasse la taille annoncée; aucun plugin installé.".into());
    }
    let mut bytes = Vec::with_capacity(size as usize);
    response
        .take(size.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 != size {
        return Err(
            "La taille téléchargée ne correspond pas au catalogue; aucun plugin installé.".into(),
        );
    }
    let actual_sha512 = format!("{:x}", Sha512::digest(&bytes));
    if !actual_sha512.eq_ignore_ascii_case(expected_sha512) {
        return Err("Le contrôle SHA-512 du plugin a échoué; aucun plugin installé.".into());
    }
    let jar = zip::ZipArchive::new(io::Cursor::new(bytes.as_slice())).map_err(|_| {
        "Le fichier téléchargé n’est pas une archive JAR valide; aucun plugin installé.".to_string()
    })?;
    if !jar
        .file_names()
        .any(|name| name == "plugin.yml" || name == "paper-plugin.yml")
    {
        return Err(
            "Le JAR ne contient pas de déclaration Paper reconnue; aucun plugin installé.".into(),
        );
    }

    progress("Préparation d’une nouvelle copie du serveur…", 62);
    let staging = parent.join(format!(".Monde-Ensemble-{id}.preparing"));
    fs::create_dir(&staging).map_err(io_error)?;
    let _cleanup_staging = RemoveDirectoryOnDrop(staging.clone());
    copy_server_tree(&server_dir, &staging, &progress)?;
    if hash_regular_file(&server_dir.join("monde-ensemble-profile.json"))
        != hash_regular_file(&staging.join("monde-ensemble-profile.json"))
        || hash_regular_file(&server_dir.join("paper.jar"))
            != hash_regular_file(&staging.join("paper.jar"))
    {
        return Err("Le serveur a changé pendant la préparation; la copie candidate est annulée et l’original reste intact.".into());
    }
    let staged_plugin = staging.join("plugins").join(&destination_name);
    let mut plugin_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged_plugin)
        .map_err(io_error)?;
    plugin_file.write_all(&bytes).map_err(io_error)?;
    plugin_file.sync_all().map_err(io_error)?;
    let staged_hash = hash_file_sha512(&staged_plugin)
        .ok_or_else(|| "Le contrôle du plugin dans la nouvelle copie a échoué.".to_string())?;
    if !staged_hash.eq_ignore_ascii_case(expected_sha512) {
        return Err(
            "Le contrôle du plugin dans la nouvelle copie a échoué; l’original reste intact."
                .into(),
        );
    }
    let mut next_profile = profile;
    let object = next_profile
        .as_object_mut()
        .ok_or_else(|| "Le profil serveur est invalide.".to_string())?;
    let mut next_managed = managed;
    next_managed.push(serde_json::json!({
        "project_id": project_id,
        "version": version_number,
        "filename": destination_name,
        "sha512": actual_sha512,
        "minecraft_java": minecraft_version,
        "installed_at": Utc::now().to_rfc3339()
    }));
    object.insert(
        "managed_plugins".into(),
        serde_json::Value::Array(next_managed),
    );
    fs::write(
        staging.join("monde-ensemble-profile.json"),
        serde_json::to_vec_pretty(&next_profile).map_err(|error| error.to_string())?,
    )
    .map_err(io_error)?;
    if staging.join("serveur.pid").exists() {
        fs::remove_file(staging.join("serveur.pid")).map_err(io_error)?;
    }
    let published = parent.join(format!("Monde-Ensemble-avec-plugin-{id}"));
    fs::create_dir(&published).map_err(|error| {
        format!("Impossible de réserver un nouveau dossier candidat sans écraser un dossier existant : {error}")
    })?;
    let mut cleanup_published = RemoveDirectoryUnlessKept {
        path: published.clone(),
        keep: false,
    };
    for entry in fs::read_dir(&staging).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let target = published.join(entry.file_name());
        fs::rename(entry.path(), target).map_err(|error| {
            format!("La copie candidate n’a pas pu être publiée entièrement; l’original reste intact : {error}")
        })?;
    }
    fs::set_permissions(
        &published,
        fs::metadata(&server_dir).map_err(io_error)?.permissions(),
    )
    .map_err(io_error)?;
    fs::remove_dir(&staging).map_err(io_error)?;
    cleanup_published.keep = true;
    progress(
        "Copie candidate publiée; le serveur d’origine reste intact.",
        100,
    );
    Ok(PaperPluginInstallResult {
        server_path: published.display().to_string(),
        backup_path: backup_path.display().to_string(),
        backup_sha256: hash_file(&backup_path)?,
        plugin_name: destination_name,
        plugin_version: version_number.to_string(),
        sha512: actual_sha512,
    })
}

/// Adds one verified Paper plugin to a new managed Pi server folder. The
/// original server is only read and archived; a new sibling profile is
/// published after the full copy and plugin checksum have been verified.
pub fn install_pi_paper_plugin_copy(
    host: &str,
    username: &str,
    server_folder: &str,
    project_id: &str,
    version_number: &str,
    progress: impl Fn(&str, u8),
) -> Result<PaperPluginInstallResult, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
        || project_id.is_empty()
        || project_id.len() > 64
        || !project_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
        || version_number.is_empty()
        || version_number.len() > 80
        || !version_number
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._+-".contains(&byte))
    {
        return Err("Le serveur ou le plugin sélectionné n’a pas une référence valide.".into());
    }
    let status = inspect_pi_server(host, username, server_folder)?;
    if status.running {
        return Err("Arrête le serveur Pi depuis son tableau de bord avant d’ajouter un plugin. L’original reste en place.".into());
    }
    let minecraft_version = status.minecraft_java;
    let valid_version = Regex::new(r"^\d+\.\d+(?:\.\d+)?$").unwrap();
    if !valid_version.is_match(&minecraft_version) {
        return Err("La version Java du serveur Pi est inconnue; ajout bloqué.".into());
    }
    let client = client()?;
    progress("Vérification de la version exacte du plugin…", 5);
    let versions: Vec<serde_json::Value> = client
        .get(format!(
            "https://api.modrinth.com/v2/project/{project_id}/version"
        ))
        .query(&[
            ("loaders", "[\"paper\"]"),
            (
                "game_versions",
                format!("[\"{minecraft_version}\"]").as_str(),
            ),
            ("include_changelog", "false"),
        ])
        .send()
        .map_err(network_error)?
        .error_for_status()
        .map_err(network_error)?
        .json()
        .map_err(|error| format!("Réponse de version Modrinth illisible : {error}"))?;
    let version = versions
        .into_iter()
        .find(|version| {
            version.get("version_number").and_then(serde_json::Value::as_str)
                == Some(version_number)
                && version.get("status").and_then(serde_json::Value::as_str) == Some("listed")
                && version.get("version_type").and_then(serde_json::Value::as_str) == Some("release")
                && version.get("loaders").and_then(serde_json::Value::as_array).is_some_and(|items| items.iter().any(|item| item.as_str() == Some("paper")))
                && version.get("game_versions").and_then(serde_json::Value::as_array).is_some_and(|items| items.iter().any(|item| item.as_str() == Some(minecraft_version.as_str())))
                && matches!(version.get("environment").and_then(serde_json::Value::as_str), Some("server_only" | "server_only_client_optional" | "dedicated_server_only"))
        })
        .ok_or_else(|| "Le catalogue ne confirme plus cette version exacte de Paper/Minecraft. Le Pi n’a pas été modifié.".to_string())?;
    if version
        .get("dependencies")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|dependencies| {
            dependencies.iter().any(|dependency| {
                dependency
                    .get("dependency_type")
                    .and_then(serde_json::Value::as_str)
                    == Some("required")
            })
        })
    {
        return Err("Cette version réclame une dépendance obligatoire qui n’est pas encore gérée. Le Pi n’a pas été modifié.".into());
    }
    let files = version
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Le catalogue n’a pas fourni le fichier du plugin.".to_string())?;
    let file = files
        .iter()
        .find(|file| file.get("primary").and_then(serde_json::Value::as_bool) == Some(true))
        .or_else(|| files.first())
        .ok_or_else(|| "Le catalogue n’a pas fourni de fichier principal.".to_string())?;
    let filename = file
        .get("filename")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Le nom du fichier du plugin est invalide.".to_string())?;
    if !valid_plugin_filename(filename) {
        return Err("Le fichier proposé n’est pas un plugin JAR reconnu.".into());
    }
    let url = file
        .get("url")
        .and_then(serde_json::Value::as_str)
        .filter(|url| url.starts_with("https://cdn.modrinth.com/"))
        .ok_or_else(|| "Le lien de téléchargement Modrinth n’est pas vérifiable.".to_string())?;
    let expected_sha512 = file
        .get("hashes")
        .and_then(|hashes| hashes.get("sha512"))
        .and_then(serde_json::Value::as_str)
        .filter(|hash| hash.len() == 128 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "Le catalogue ne fournit pas d’empreinte SHA-512 valide.".to_string())?;
    let size = file
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    if size == 0 || size > 100 * 1024 * 1024 {
        return Err(
            "Le fichier proposé dépasse la taille autorisée; le Pi n’a pas été modifié.".into(),
        );
    }
    progress("Téléchargement et contrôle SHA-512…", 12);
    let response = client
        .get(url)
        .send()
        .map_err(network_error)?
        .error_for_status()
        .map_err(network_error)?;
    if !response.url().scheme().eq("https")
        || response.url().host_str() != Some("cdn.modrinth.com")
        || response
            .content_length()
            .is_some_and(|length| length > size)
    {
        return Err("Le téléchargement Modrinth a un hôte ou une taille inattendus; le Pi n’a pas été modifié.".into());
    }
    let mut bytes = Vec::with_capacity(size as usize);
    response
        .take(size.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 != size {
        return Err(
            "La taille téléchargée ne correspond pas au catalogue; le Pi n’a pas été modifié."
                .into(),
        );
    }
    let actual_sha512 = format!("{:x}", Sha512::digest(&bytes));
    if !actual_sha512.eq_ignore_ascii_case(expected_sha512) {
        return Err("Le contrôle SHA-512 du plugin a échoué; le Pi n’a pas été modifié.".into());
    }
    let jar = zip::ZipArchive::new(io::Cursor::new(bytes.as_slice())).map_err(|_| {
        "Le fichier téléchargé n’est pas une archive JAR valide; le Pi n’a pas été modifié."
            .to_string()
    })?;
    if !jar
        .file_names()
        .any(|name| name == "plugin.yml" || name == "paper-plugin.yml")
    {
        return Err(
            "Le JAR ne contient pas de déclaration Paper reconnue; le Pi n’a pas été modifié."
                .into(),
        );
    }

    let destination = format!("{username}@{host}");
    let inventory = format!(
        "set -eu; d=\"$HOME/{server_folder}\"; test -s \"$d/monde-ensemble-profile.json\"; test -s \"$d/paper.jar\"; test -s \"$d/plugins/Geyser-Spigot.jar\"; test -s \"$d/plugins/Floodgate-Spigot.jar\"; printf 'PROFILE_START\\n'; cat \"$d/monde-ensemble-profile.json\"; printf '\\nPROFILE_END\\n'; printf 'JARS_START\\n'; find \"$d/plugins\" -maxdepth 1 -type f -name '*.jar' -printf '%f\\n' | sort; printf 'JARS_END\\n'; printf 'MODS_START\\n'; find \"$d/mods\" -type f -name '*.jar' -printf '%P\\n' 2>/dev/null || true; printf 'MODS_END\\n'; printf 'SYMLINK='; if find \"$d\" -type l -print -quit | grep -q .; then echo yes; else echo no; fi; printf 'PORTS_BUSY='; if ss -H -lntu 2>/dev/null | awk '{{print $5}}' | grep -qE '[:.](25565|25566|19132|19133)$'; then echo yes; else echo no; fi; printf 'TOTAL='; du -sB1 \"$d\" | awk '{{print $1}}'; printf 'FREE='; df -B1 --output=avail \"$HOME\" | tail -1 | tr -dc '0-9'; printf '\\nPYTHON='; command -v python3 >/dev/null 2>&1 && echo yes || echo no"
    );
    progress("Vérification du serveur Pi et de l’espace disponible…", 20);
    let output = run_ssh(&destination, &inventory)?;
    let (before_profile, profile_rest) = output
        .split_once("PROFILE_START\n")
        .ok_or_else(|| "Le Pi n’a pas renvoyé le profil du serveur.".to_string())?;
    let _ = before_profile;
    let (profile_text, rest) = profile_rest
        .split_once("\nPROFILE_END\n")
        .ok_or_else(|| "Le profil du serveur Pi est incomplet.".to_string())?;
    let (jars_rest, tail) = rest
        .split_once("JARS_START\n")
        .ok_or_else(|| "Le Pi n’a pas renvoyé sa liste de plugins.".to_string())?;
    let _ = jars_rest;
    let (jar_lines, tail) = tail
        .split_once("JARS_END\n")
        .ok_or_else(|| "La liste des plugins Pi est incomplète.".to_string())?;
    let (mod_lines, tail) = tail
        .split_once("MODS_START\n")
        .and_then(|(_, rest)| {
            rest.split_once("MODS_END\n")
                .map(|(mods, after)| (mods, after))
        })
        .ok_or_else(|| "La liste des mods Pi est incomplète.".to_string())?;
    let profile: serde_json::Value = serde_json::from_str(profile_text.trim())
        .map_err(|_| "Le profil du serveur Pi est illisible.".to_string())?;
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
    {
        return Err("Ce dossier n’a pas été créé par Monde Ensemble; le Pi reste intact.".into());
    }
    if profile.get("managed_plugins").is_some()
        && !profile
            .get("managed_plugins")
            .is_some_and(serde_json::Value::is_array)
    {
        return Err("Le manifeste des plugins du Pi est illisible; ajout bloqué.".into());
    }
    if profile
        .get("minecraft_java")
        .and_then(serde_json::Value::as_str)
        != Some(minecraft_version.as_str())
    {
        return Err("Le profil du Pi a changé pendant la vérification; ajout bloqué.".into());
    }
    let managed = profile
        .get("managed_plugins")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut managed_files = HashSet::new();
    for entry in &managed {
        let name = entry
            .get("filename")
            .and_then(serde_json::Value::as_str)
            .filter(|name| valid_plugin_filename(name))
            .ok_or_else(|| {
                "Le manifeste des plugins Pi est incomplet; ajout bloqué.".to_string()
            })?;
        let digest = entry
            .get("sha512")
            .and_then(serde_json::Value::as_str)
            .filter(|digest| {
                digest.len() == 128 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(|| {
                "L’empreinte d’un plugin Pi géré est invalide; ajout bloqué.".to_string()
            })?;
        managed_files.insert(name.to_string());
        if !tail.contains(&format!("{name}={digest}")) {
            let verify = format!(
                "set -eu; d=\"$HOME/{server_folder}\"; printf '{}='; sha512sum \"$d/plugins/{}\" | cut -d' ' -f1",
                name, name
            );
            let value = run_ssh(&destination, &verify)?;
            if !value.trim().ends_with(digest) {
                return Err(format!(
                    "Le plugin Pi {name} a changé; le serveur reste intact."
                ));
            }
        }
    }
    let listed_jars = jar_lines
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    for name in listed_jars {
        if !matches!(name, "Geyser-Spigot.jar" | "Floodgate-Spigot.jar")
            && !managed_files.contains(name)
        {
            return Err(format!(
                "Le serveur Pi contient déjà le plugin non géré {name}; ajout bloqué sans modifier le Pi."
            ));
        }
    }
    if mod_lines.lines().any(|name| !name.trim().is_empty()) {
        return Err("Des mods personnalisés sont présents sur le Pi; leur interaction avec ce plugin n’est pas vérifiée. Le serveur reste intact.".into());
    }
    if tail.contains("SYMLINK=yes") || tail.contains("PORTS_BUSY=yes") {
        return Err("Un lien symbolique ou un port Minecraft actif empêche la copie sûre. Le Pi n’a pas été modifié.".into());
    }
    if !tail.contains("PYTHON=yes") {
        return Err("Python 3 est nécessaire pour conserver le manifeste des plugins gérés; le Pi n’a pas été modifié.".into());
    }
    let total = tail
        .lines()
        .find_map(|line| line.strip_prefix("TOTAL="))
        .and_then(|value| value.trim().parse::<u64>().ok())
        .ok_or_else(|| "Taille du serveur Pi illisible.".to_string())?;
    let free = tail
        .lines()
        .find_map(|line| line.strip_prefix("FREE="))
        .and_then(|value| value.trim().parse::<u64>().ok())
        .ok_or_else(|| "Espace libre du Pi illisible.".to_string())?;
    if free
        < total
            .saturating_mul(2)
            .saturating_add(size)
            .saturating_add(512 * 1024 * 1024)
    {
        return Err("L’espace du Pi ne suffit pas pour l’archive et une nouvelle copie complète; le monde reste intact.".into());
    }

    let id = Uuid::new_v4().simple().to_string();
    let parent_folder = server_folder.strip_suffix("/server").unwrap_or_default();
    if parent_folder.is_empty() {
        return Err("Le dossier Pi enregistré n’est pas reconnu.".into());
    }
    let new_root = format!("Monde-Ensemble-{id}");
    let new_server_folder = format!("{new_root}/server");
    let backup_name = format!("Monde-Ensemble-plugin-backup-{id}.tar.gz");
    let upload_name = format!(".monde-ensemble-plugin-{id}.jar");
    let stage_name = format!(".Monde-Ensemble-{id}.preparing");
    let local_plugin = std::env::temp_dir().join(format!("monde-ensemble-plugin-{id}.jar"));
    let _cleanup_plugin = RemoveFileOnDrop(local_plugin.clone());
    let mut local_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&local_plugin)
        .map_err(io_error)?;
    local_file.write_all(&bytes).map_err(io_error)?;
    local_file.sync_all().map_err(io_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&local_plugin, fs::Permissions::from_mode(0o600)).map_err(io_error)?;
    }
    progress("Transfert chiffré du plugin vers le Pi…", 30);
    let scp = scp_command()
        .args([
            "-p",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "PasswordAuthentication=no",
            local_plugin.to_string_lossy().as_ref(),
            &format!("{destination}:{upload_name}"),
        ])
        .output()
        .map_err(|_| "L’outil de transfert sécurisé n’est pas disponible.".to_string())?;
    if !scp.status.success() {
        return Err(
            "Le transfert SSH du plugin a échoué; le serveur Pi n’a pas été modifié.".into(),
        );
    }
    let backup_hash_script = format!(
        r#"set -eu
umask 077
src="$HOME/{server_folder}"
backup_dir="$HOME/sauvegardes-monde-ensemble-pi"
backup="$backup_dir/{backup_name}"
stage="$HOME/{stage_name}"
published="$HOME/{new_root}"
upload="$HOME/{upload_name}"
stage_owned=no
published_owned=no
backup_ready=no
trap 'rm -f -- "$upload"; if test "$stage_owned" = yes; then rm -rf -- "$stage"; fi; if test "$published_owned" = yes; then rmdir "$published" 2>/dev/null || true; fi; if test "$backup_ready" != yes; then rm -f -- "$backup"; fi' EXIT
test -d "$src" && test ! -e "$backup" && test ! -e "$stage" && test ! -e "$published"
test "$(sha512sum "$upload" | cut -d' ' -f1)" = "{actual_sha512}"
if find "$src" -type l -print -quit | grep -q .; then exit 12; fi
if find "$src/mods" -type f -name '*.jar' -print -quit 2>/dev/null | grep -q .; then exit 13; fi
if ss -H -lntu 2>/dev/null | awk '{{print $5}}' | grep -qE '[:.](25565|25566|19132|19133)$'; then exit 14; fi
total="$(du -sB1 "$src" | awk '{{print $1}}')"
free="$(df -B1 --output=avail "$HOME" | tail -1 | tr -dc '0-9')"
required=$((total * 2 + {size} + 536870912))
test "$free" -ge "$required"
if test -L "$backup_dir" || {{ test -e "$backup_dir" && ! test -d "$backup_dir"; }}; then exit 15; fi
mkdir -p "$backup_dir"
test ! -e "$backup"
mkdir "$stage"
stage_owned=yes
tar -czf "$backup" -C "$HOME/{parent_folder}" server
tar -tzf "$backup" >/dev/null
archive_hash="$(sha256sum "$backup" | cut -d' ' -f1)"
backup_ready=yes
cp -a "$src" "$stage/server"
test "$(sha256sum "$src/monde-ensemble-profile.json" | cut -d' ' -f1)" = "$(sha256sum "$stage/server/monde-ensemble-profile.json" | cut -d' ' -f1)"
test "$(sha256sum "$src/paper.jar" | cut -d' ' -f1)" = "$(sha256sum "$stage/server/paper.jar" | cut -d' ' -f1)"
test "$(sha256sum "$src/plugins/Geyser-Spigot.jar" | cut -d' ' -f1)" = "$(sha256sum "$stage/server/plugins/Geyser-Spigot.jar" | cut -d' ' -f1)"
test "$(sha256sum "$src/plugins/Floodgate-Spigot.jar" | cut -d' ' -f1)" = "$(sha256sum "$stage/server/plugins/Floodgate-Spigot.jar" | cut -d' ' -f1)"
test ! -e "$stage/server/plugins/{filename}"
install -m 644 -- "$upload" "$stage/server/plugins/{filename}"
test "$(sha512sum "$stage/server/plugins/{filename}" | cut -d' ' -f1)" = "{actual_sha512}"
python3 - "$stage/server/monde-ensemble-profile.json" "{project_id}" "{version_number}" "{filename}" "{actual_sha512}" "{minecraft_version}" <<'PY'
import json, os, sys, tempfile
path, project, version, filename, digest, game = sys.argv[1:]
with open(path, encoding="utf-8") as source:
    profile = json.load(source)
entries = profile.setdefault("managed_plugins", [])
entries.append({{"project_id": project, "version": version, "filename": filename, "sha512": digest, "minecraft_java": game}})
fd, temporary = tempfile.mkstemp(prefix=".profile-", dir=os.path.dirname(path))
try:
    with os.fdopen(fd, "w", encoding="utf-8") as target:
        json.dump(profile, target, ensure_ascii=False, indent=2)
        target.write("\n")
        target.flush()
        os.fsync(target.fileno())
    os.chmod(temporary, 0o600)
    os.replace(temporary, path)
except Exception:
    try: os.unlink(temporary)
    except OSError: pass
    raise
PY
mkdir "$published"
published_owned=yes
mv "$stage/server" "$published/server"
stage_owned=no
published_owned=no
printf 'SERVER_FOLDER=%s\nBACKUP_SHA256=%s\n' '{new_server_folder}' "$archive_hash"
"#
    );
    progress("Sauvegarde complète et copie sûre sur le Pi…", 45);
    let result = run_ssh_timeout(&destination, &backup_hash_script, 240)?;
    let get_field = |key: &str| {
        result
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .map(str::trim)
    };
    let returned_folder = get_field("SERVER_FOLDER=")
        .ok_or_else(|| "Le Pi n’a pas confirmé le dossier candidat.".to_string())?;
    let backup_sha256 = get_field("BACKUP_SHA256=")
        .filter(|value| valid_sha256(value))
        .ok_or_else(|| "Le Pi n’a pas renvoyé une empreinte de sauvegarde valide.".to_string())?;
    if returned_folder != new_server_folder {
        return Err(
            "Le Pi a renvoyé un dossier candidat inattendu; l’original reste conservé.".into(),
        );
    }
    progress(
        "Nouvelle copie du serveur Pi prête; l’original reste conservé.",
        100,
    );
    Ok(PaperPluginInstallResult {
        server_path: returned_folder.to_string(),
        backup_path: format!("~/sauvegardes-monde-ensemble-pi/{backup_name}"),
        backup_sha256: backup_sha256.to_string(),
        plugin_name: filename.to_string(),
        plugin_version: version_number.to_string(),
        sha512: actual_sha512,
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

#[cfg(test)]
fn latest_supported_pi_image(os_list: Vec<RaspberryPiOsEntry>) -> Option<RaspberryPiOsEntry> {
    let mut entries = Vec::new();
    flatten_pi_os_entries(os_list, &mut entries);
    let mut candidates: Vec<_> = entries
        .into_iter()
        .filter(|entry| {
            entry
                .name
                .to_ascii_lowercase()
                .starts_with("raspberry pi os lite (64-bit)")
                && entry.init_format.as_deref() == Some("cloudinit-rpi")
                && entry
                    .url
                    .as_deref()
                    .is_some_and(|url| url.starts_with("https://downloads.raspberrypi.com/"))
                && entry.devices.as_ref().is_some_and(|devices| {
                    devices.iter().any(|device| {
                        ["pi3", "pi4", "pi5"]
                            .iter()
                            .any(|supported| device.starts_with(supported))
                    })
                })
                && entry.extract_sha256.as_deref().is_some_and(valid_sha256)
                && entry.extract_size.is_some_and(|size| size > 0)
                && entry.image_download_size.is_some_and(|size| size > 0)
                && entry
                    .release_date
                    .as_deref()
                    .is_some_and(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok())
        })
        .collect();
    candidates.sort_by(|left, right| right.release_date.cmp(&left.release_date));
    candidates.into_iter().next()
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
    parse_supported_versions_page(&page)
}

fn parse_supported_versions_page(page: &str) -> Result<SupportedVersions, String> {
    let plain = Regex::new(r"(?s)<[^>]+>")
        .map_err(|e| e.to_string())?
        .replace_all(page, " ");
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

fn paper_build_by_id(
    client: &Client,
    game_version: &str,
    build_id: u64,
) -> Result<PaperBuild, String> {
    if !safe_version(game_version) {
        return Err("La version Paper du profil est illisible.".into());
    }
    let url = format!("https://fill.papermc.io/v3/projects/paper/versions/{game_version}/builds");
    let builds: Vec<PaperBuild> = get_json(client, &url)?;
    builds
        .into_iter()
        .find(|build| {
            build.id == build_id
                && build.channel.eq_ignore_ascii_case("STABLE")
                && build.downloads.contains_key("server:default")
        })
        .ok_or_else(|| {
            "Le build Paper d’origine n’est plus vérifiable dans le catalogue officiel.".into()
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

fn bridge_build_by_id(
    client: &Client,
    project: &str,
    version: &str,
    build_id: u64,
    flavor: &str,
) -> Result<BridgeBuild, String> {
    if !matches!(project, "geyser" | "floodgate")
        || version.is_empty()
        || version.len() > 64
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err(format!("La version {project} du profil est illisible."));
    }
    let url = format!(
        "https://download.geysermc.org/v2/projects/{project}/versions/{version}/builds/{build_id}"
    );
    let build: BridgeBuild = get_json(client, &url)?;
    if build.version != version || build.build != build_id {
        return Err(format!(
            "Le build {project} d’origine ne correspond plus au catalogue officiel."
        ));
    }
    if !build.downloads.contains_key(flavor) {
        return Err(format!(
            "Le téléchargement officiel {project} / {flavor} est absent."
        ));
    }
    Ok(build)
}

fn legacy_stack_checksums(
    client: &Client,
    profile: &serde_json::Value,
) -> Result<[String; 3], String> {
    let java = profile
        .get("minecraft_java")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "La version Java du profil est absente.".to_string())?;
    let paper_build_id = profile
        .get("paper_build_id")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "Le build Paper du profil est absent.".to_string())?;
    let paper = paper_build_by_id(client, java, paper_build_id)?;
    let paper_sha256 = paper
        .downloads
        .get("server:default")
        .and_then(|download| download.checksums.get("sha256"))
        .filter(|checksum| valid_sha256(checksum))
        .cloned()
        .ok_or_else(|| {
            "Paper ne publie pas l’empreinte SHA-256 de son build d’origine.".to_string()
        })?;

    let mut hashes = [paper_sha256, String::new(), String::new()];
    for (index, (project, version_key, build_key)) in [
        ("geyser", "geyser", "geyser_build_id"),
        ("floodgate", "floodgate", "floodgate_build_id"),
    ]
    .into_iter()
    .enumerate()
    {
        let version = profile
            .get(version_key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("La version {project} du profil est absente."))?;
        let build_id = profile
            .get(build_key)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("Le build {project} du profil est absent."))?;
        let build = bridge_build_by_id(client, project, version, build_id, "spigot")?;
        hashes[index + 1] = build
            .downloads
            .get("spigot")
            .map(|download| download.sha256.clone())
            .filter(|checksum| valid_sha256(checksum))
            .ok_or_else(|| {
                format!("{project} ne publie pas l’empreinte SHA-256 de son build d’origine.")
            })?;
    }
    Ok(hashes)
}

fn artifact_hash_matches(
    label: &str,
    actual: &str,
    recorded: Option<&str>,
    official_legacy: Option<&str>,
) -> Result<(), String> {
    if !valid_sha256(actual) {
        return Err(format!("L’empreinte du fichier {label} est illisible."));
    }
    let (expected, source) = match recorded {
        Some(value) if valid_sha256(value) => (value, "son profil d’installation"),
        Some(_) => return Err(format!("L’empreinte enregistrée de {label} est invalide.")),
        None => match official_legacy {
            Some(value) if valid_sha256(value) => (value, "son build officiel d’origine"),
            _ => {
                return Err(format!(
                    "Le build officiel d’origine de {label} ne peut pas être vérifié; la mise à jour automatique est bloquée."
                ));
            }
        },
    };
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "Le fichier {label} ne correspond pas à {source}; la mise à jour automatique est bloquée."
        ))
    }
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
    let heap_mb = (((memory_gb * 0.4).floor() as u16).clamp(1, 6)) * 1024;
    let java_port = available_game_port(JAVA_DEFAULT_PORT, JAVA_ALTERNATE_PORT, false)?;
    let bedrock_port = available_game_port(BEDROCK_DEFAULT_PORT, BEDROCK_ALTERNATE_PORT, true)?;
    let root = prepare_root(destination_root)?;
    let staging = root.join(format!(".monde-ensemble-{}.preparing", Uuid::new_v4()));
    fs::create_dir(&staging).map_err(io_error)?;
    let result = build_fresh_server(
        &staging,
        heap_mb,
        false,
        java_port,
        bedrock_port,
        None,
        &progress,
    );
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
        heap_mb,
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
    heap_mb: u16,
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
            initial_heap_argument(heap_mb),
            format!("-Xmx{heap_mb}M"),
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
            if !tcp_port_open(bedrock_port) {
                terminate_local_process(&mut child);
                let _ = fs::remove_file(&pid_file);
                return Err(format!(
                    "Le serveur Java est prêt, mais le signalement Bedrock TCP {bedrock_port} ne répond pas."
                ));
            }
            if udp_port_available(bedrock_port) {
                terminate_local_process(&mut child);
                let _ = fs::remove_file(&pid_file);
                return Err(format!(
                    "Le serveur Java est prêt, mais le port Bedrock UDP {bedrock_port} n’est pas ouvert."
                ));
            }
            if !geyser_nethernet_ready(server_dir) {
                terminate_local_process(&mut child);
                let _ = fs::remove_file(&pid_file);
                return Err("Geyser n’a pas démarré avec le transport Bedrock NetherNet attendu. Le serveur n’est pas déclaré prêt.".into());
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
    UdpSocket::bind(("0.0.0.0", port)).is_ok()
}

fn tcp_port_available(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
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

/// Reads only a server profile created by this application and confirms the
/// live process by authenticating to its loopback-only RCON endpoint.
pub fn inspect_local_server(server_dir: &Path) -> Result<LocalServerStatus, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier de serveur local enregistré est introuvable.".to_string())?;
    let profile_path = server_dir.join("monde-ensemble-profile.json");
    let profile: serde_json::Value = serde_json::from_slice(
        &fs::read(&profile_path)
            .map_err(|_| "Le profil du serveur local est illisible.".to_string())?,
    )
    .map_err(|_| "Le profil du serveur local est illisible.".to_string())?;
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
    {
        return Err("Ce dossier n’a pas été créé par Monde Ensemble.".into());
    }
    if profile
        .get("rcon_port")
        .and_then(serde_json::Value::as_u64)
        .is_none()
        || profile
            .get("rcon_password")
            .and_then(serde_json::Value::as_str)
            .is_none()
    {
        return Err("Ce serveur a été créé par une version antérieure et ne possède pas le contrôle local sécurisé. Il reste intact; utilise son lanceur dans le dossier indiqué.".into());
    }
    let text = |key: &str| {
        profile
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("inconnue")
            .to_string()
    };
    let number = |key: &str| {
        profile
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or_default()
    };
    let java_port = number("java_port");
    let bedrock_port = number("bedrock_port");
    let players = rcon_command(&profile, "list").ok();
    let running = players.is_some();
    let log = fs::read_to_string(server_dir.join("serveur.log"))
        .map(|contents| {
            contents
                .lines()
                .rev()
                .take(200)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let ready = running
        && log.contains("Done (")
        && java_port > 0
        && bedrock_port > 0
        && tcp_port_open(java_port)
        && tcp_port_open(bedrock_port)
        && !udp_port_available(bedrock_port)
        && geyser_nethernet_ready(&server_dir);
    Ok(LocalServerStatus {
        running,
        ready,
        java_port_open: tcp_port_open(java_port),
        minecraft_java: text("minecraft_java"),
        minecraft_bedrock: text("minecraft_bedrock"),
        java_port,
        bedrock_port,
        bedrock_tcp_port_open: tcp_port_open(bedrock_port),
        bedrock_udp_port_open: !udp_port_available(bedrock_port),
        bedrock_nethernet_ready: geyser_nethernet_ready(&server_dir),
        address: local_network_address().unwrap_or_else(|| "127.0.0.1".into()),
        players: players.unwrap_or_default(),
        log,
    })
}

/// Read-only compatibility gate for an application-managed local server.
/// Unknown extensions, custom resource packs, modified Geyser configuration,
/// links, or an unrecognized runtime prevent an automatic update.
pub fn check_local_update(server_dir: &Path) -> Result<PiUpdateCheck, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier du serveur local enregistré est introuvable.".to_string())?;
    let latest = compatibility()?;
    let profile_path = server_dir.join("monde-ensemble-profile.json");
    let profile: serde_json::Value = serde_json::from_slice(
        &fs::read(&profile_path)
            .map_err(|_| "Le profil du serveur local est illisible.".to_string())?,
    )
    .map_err(|_| "Le profil du serveur local est illisible.".to_string())?;
    let mut reasons = Vec::new();
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
    {
        reasons.push("Cette installation n’a pas un profil Monde Ensemble reconnu.".into());
    }

    let profile_sha256 = hash_regular_file(&profile_path).unwrap_or_default();
    let paper_path = server_dir.join("paper.jar");
    let geyser_path = server_dir.join("plugins/Geyser-Spigot.jar");
    let floodgate_path = server_dir.join("plugins/Floodgate-Spigot.jar");
    let paper_sha256 = hash_regular_file(&paper_path).unwrap_or_default();
    let geyser_sha256 = hash_regular_file(&geyser_path).unwrap_or_default();
    let floodgate_sha256 = hash_regular_file(&floodgate_path).unwrap_or_default();
    if !valid_sha256(&profile_sha256) {
        reasons.push("Le fichier profil est absent ou son empreinte est illisible.".into());
    }
    let hash_fields = ["paper_sha256", "geyser_sha256", "floodgate_sha256"];
    let legacy_hashes = if hash_fields
        .iter()
        .any(|field| profile.get(*field).is_none())
    {
        client()
            .and_then(|client| legacy_stack_checksums(&client, &profile))
            .ok()
    } else {
        None
    };
    for (index, (name, field, actual)) in [
        ("Paper", hash_fields[0], paper_sha256.as_str()),
        ("Geyser", hash_fields[1], geyser_sha256.as_str()),
        ("Floodgate", hash_fields[2], floodgate_sha256.as_str()),
    ]
    .into_iter()
    .enumerate()
    {
        let recorded = profile.get(field).and_then(serde_json::Value::as_str);
        let official_legacy = legacy_hashes.as_ref().map(|hashes| hashes[index].as_str());
        if let Err(reason) = artifact_hash_matches(name, actual, recorded, official_legacy) {
            reasons.push(reason);
        }
    }

    let mut has_symlink = false;
    for entry in WalkDir::new(&server_dir).follow_links(false) {
        match entry {
            Ok(entry) if entry.file_type().is_symlink() => has_symlink = true,
            Err(_) => reasons.push(
                "Un élément du dossier serveur est illisible; la mise à jour est bloquée.".into(),
            ),
            _ => {}
        }
    }
    if has_symlink {
        reasons.push(
            "Le serveur contient un lien symbolique; la mise à jour automatique est bloquée."
                .into(),
        );
    }

    let plugin_jars = WalkDir::new(server_dir.join("plugins"))
        .max_depth(2)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "jar")
        })
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter(|name| !matches!(name.as_str(), "Geyser-Spigot.jar" | "Floodgate-Spigot.jar"))
        .collect::<Vec<_>>();
    if !plugin_jars.is_empty() {
        reasons.push(format!(
            "Extensions non vérifiées : {}. Elles bloquent la mise à jour automatique pour protéger le monde.",
            plugin_jars.join(", ")
        ));
    }
    let mod_jars = WalkDir::new(server_dir.join("mods"))
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "jar")
        })
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(server_dir.join("mods"))
                .ok()
                .map(|path| path.display().to_string())
        })
        .collect::<Vec<_>>();
    if !mod_jars.is_empty() {
        reasons.push(format!(
            "Mods présents : {}. Leur compatibilité n’est pas vérifiée par l’application.",
            mod_jars.join(", ")
        ));
    }

    let properties = match fs::read_to_string(server_dir.join("server.properties")) {
        Ok(properties) => properties,
        Err(_) => {
            reasons.push(
                "Les réglages du serveur sont illisibles; la mise à jour est bloquée.".into(),
            );
            String::new()
        }
    };
    let level_name = properties
        .lines()
        .find_map(|line| line.strip_prefix("level-name="))
        .filter(|value| !value.is_empty())
        .unwrap_or("world");
    let level_path = Path::new(level_name);
    if level_path.components().count() != 1
        || !matches!(level_path.components().next(), Some(Component::Normal(_)))
    {
        reasons.push(
            "Le nom du monde sort du dossier serveur attendu; la mise à jour est bloquée.".into(),
        );
    } else {
        let datapacks = server_dir.join(level_path).join("datapacks");
        match fs::read_dir(&datapacks) {
            Ok(entries) => {
                let mut found_custom_pack = false;
                for entry in entries {
                    match entry {
                        Ok(entry) if !entry.file_name().to_string_lossy().starts_with('.') => {
                            found_custom_pack = true;
                        }
                        Err(_) => reasons.push(
                            "La liste des packs de données est illisible; la mise à jour est bloquée.".into(),
                        ),
                        _ => {}
                    }
                }
                if found_custom_pack {
                    reasons.push("Des packs de données personnalisés sont présents; leur compatibilité doit être vérifiée avant une mise à jour.".into());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => reasons.push(
                "Le dossier des packs de données est illisible; la mise à jour est bloquée.".into(),
            ),
        }
    }

    let resource_pack = properties
        .lines()
        .find_map(|line| line.strip_prefix("resource-pack="))
        .unwrap_or_default();
    if !resource_pack.trim().is_empty() {
        reasons.push("Un pack de ressources personnalisé est configuré; sa compatibilité doit être vérifiée avant une mise à jour.".into());
    }

    if !geyser_config_matches_profile(&server_dir, &profile) {
        reasons.push("La configuration Geyser a été modifiée ou ne peut pas être reconnue comme configuration standard; la mise à jour automatique est bloquée.".into());
    }
    let bedrock_transport_migration = !geyser_nethernet_ready(&server_dir);
    let bedrock_packs = server_dir.join("plugins/Geyser-Spigot/packs");
    match fs::read_dir(&bedrock_packs) {
        Ok(entries) => {
            let mut custom_pack_found = false;
            for entry in entries {
                match entry {
                    Ok(entry) if !entry.file_name().to_string_lossy().starts_with('.') => {
                        custom_pack_found = true;
                    }
                    Err(_) => reasons.push(
                        "Le dossier des packs Bedrock est illisible; la mise à jour est bloquée."
                            .into(),
                    ),
                    _ => {}
                }
            }
            if custom_pack_found {
                reasons.push("Des packs Bedrock personnalisés sont présents; leur compatibilité doit être vérifiée avant la mise à jour.".into());
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => reasons
            .push("Le dossier des packs Bedrock est illisible; la mise à jour est bloquée.".into()),
    }

    let runtime_name = if cfg!(windows) { "java.exe" } else { "java" };
    let runtime_present = WalkDir::new(server_dir.join("runtime"))
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| entry.file_type().is_file() && entry.file_name() == runtime_name);
    if !runtime_present {
        reasons.push(
            "Le runtime Java inclus est absent; la mise à jour automatique est bloquée.".into(),
        );
    }
    match java_major_for(&latest.java_version) {
        Ok(required)
            if profile
                .get("java_runtime")
                .and_then(serde_json::Value::as_u64)
                != Some(required as u64) =>
        {
            reasons.push(format!(
                "La nouvelle version demande Java {required}, différent du runtime inclus. La mise à jour attend une évolution sûre du runtime."
            ));
        }
        Err(error) => reasons.push(format!(
            "La compatibilité du runtime Java n’a pas pu être établie : {error}"
        )),
        _ => {}
    }

    let latest_geyser = latest
        .geyser_version
        .split(" · build ")
        .next()
        .unwrap_or(&latest.geyser_version)
        .to_string();
    let latest_floodgate = latest
        .floodgate_version
        .split(" · build ")
        .next()
        .unwrap_or(&latest.floodgate_version)
        .to_string();
    let current_java = profile
        .get("minecraft_java")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("inconnue")
        .to_string();
    let current_bedrock = profile
        .get("minecraft_bedrock")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("inconnue")
        .to_string();
    let current_geyser = profile
        .get("geyser")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("inconnue")
        .to_string();
    let current_floodgate = profile
        .get("floodgate")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("inconnue")
        .to_string();
    let update_available = bedrock_transport_migration
        || current_java != latest.java_version
        || current_bedrock != latest.bedrock_versions
        || current_geyser != latest_geyser
        || current_floodgate != latest_floodgate
        || profile
            .get("paper_build_id")
            .and_then(serde_json::Value::as_u64)
            != Some(latest.paper_build_id)
        || profile
            .get("geyser_build_id")
            .and_then(serde_json::Value::as_u64)
            != Some(latest.geyser_build_id)
        || profile
            .get("floodgate_build_id")
            .and_then(serde_json::Value::as_u64)
            != Some(latest.floodgate_build_id);

    Ok(PiUpdateCheck {
        ready: reasons.is_empty(),
        update_available,
        bedrock_transport_migration,
        current_java,
        current_bedrock,
        current_geyser,
        current_floodgate,
        latest_java: latest.java_version,
        latest_bedrock: latest.bedrock_versions,
        latest_geyser,
        latest_geyser_build: latest.geyser_build_id,
        latest_floodgate,
        latest_floodgate_build: latest.floodgate_build_id,
        latest_paper_build: latest.paper_build_id,
        profile,
        profile_sha256,
        paper_sha256,
        geyser_sha256,
        floodgate_sha256,
        reasons,
        checked_at: latest.checked_at,
    })
}

/// Gracefully stops a managed local server with Minecraft's authenticated
/// loopback RCON command. No process is killed if RCON cannot confirm identity.
pub fn stop_local_server(server_dir: &Path) -> Result<String, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier de serveur local enregistré est introuvable.".to_string())?;
    let profile: serde_json::Value = serde_json::from_slice(
        &fs::read(server_dir.join("monde-ensemble-profile.json"))
            .map_err(|_| "Le profil du serveur local est illisible.".to_string())?,
    )
    .map_err(|_| "Le profil du serveur local est illisible.".to_string())?;
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
    {
        return Err("Ce dossier n’a pas été créé par Monde Ensemble.".into());
    }
    if profile
        .get("rcon_port")
        .and_then(serde_json::Value::as_u64)
        .is_none()
        || profile
            .get("rcon_password")
            .and_then(serde_json::Value::as_str)
            .is_none()
    {
        return Err("Ce serveur a été créé par une version antérieure et ne possède pas le contrôle local sécurisé. Il reste intact.".into());
    }
    if rcon_command(&profile, "stop").is_err() {
        return Err("Le serveur n’a pas confirmé son identité ou son arrêt sécurisé. Aucun processus n’a été forcé à quitter.".into());
    }
    let pid_path = server_dir.join("serveur.pid");
    for _ in 0..60 {
        if !tcp_port_open(
            profile
                .get("java_port")
                .and_then(serde_json::Value::as_u64)
                .and_then(|port| u16::try_from(port).ok())
                .unwrap_or_default(),
        ) {
            let _ = fs::remove_file(pid_path);
            return Ok("Serveur arrêté proprement. Le monde a été enregistré.".into());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err("Le serveur a reçu l’ordre de sauvegarder et de s’arrêter, mais son port reste actif. Aucun arrêt forcé n’a été tenté.".into())
}

pub fn local_whitelist_status(server_dir: &Path) -> Result<LocalWhitelistStatus, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier du serveur local enregistré est introuvable.".to_string())?;
    let profile = local_profile(&server_dir)?;
    require_local_rcon(&profile)?;
    let properties_path = server_dir.join("server.properties");
    if !fs::symlink_metadata(&properties_path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
    {
        return Err(
            "Les réglages du serveur sont absents ou ne sont pas un fichier normal.".into(),
        );
    }
    let properties = fs::read_to_string(properties_path).map_err(io_error)?;
    let enabled = properties
        .lines()
        .find_map(|line| line.strip_prefix("white-list="))
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"));
    let entries = rcon_command(&profile, "whitelist list")?;
    Ok(LocalWhitelistStatus { enabled, entries })
}

pub fn change_local_whitelist(
    server_dir: &Path,
    platform: &str,
    action: &str,
    player_name: &str,
) -> Result<String, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier du serveur local enregistré est introuvable.".to_string())?;
    let profile = local_profile(&server_dir)?;
    require_local_rcon(&profile)?;
    let canonical_name = match platform {
        "java"
            if Regex::new(r"^[A-Za-z0-9_]{3,16}$")
                .is_ok_and(|pattern| pattern.is_match(player_name)) =>
        {
            player_name.to_string()
        }
        "bedrock"
            if !player_name.trim().is_empty()
                && player_name.len() <= 16
                && player_name.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | ' ')
                }) =>
        {
            player_name.split_whitespace().collect::<Vec<_>>().join("_")
        }
        "java" => {
            return Err(
                "Nom Java invalide. Utilise 3 à 16 lettres, chiffres ou tirets bas.".into(),
            );
        }
        "bedrock" => {
            return Err("Gamertag Bedrock invalide. Pour cette première version, saisis jusqu’à 16 lettres, chiffres, espaces ou tirets bas.".into());
        }
        _ => return Err("Choix d’édition invalide.".into()),
    };
    if !matches!(action, "add" | "remove") {
        return Err("Action de liste d’accès invalide.".into());
    }
    let command = if platform == "bedrock" {
        format!("fwhitelist {action} {canonical_name}")
    } else {
        format!("whitelist {action} {canonical_name}")
    };
    let response = rcon_command(&profile, &command)?;
    Ok(response.trim().to_string())
}

pub fn set_local_whitelist_enabled(server_dir: &Path, enabled: bool) -> Result<String, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier du serveur local enregistré est introuvable.".to_string())?;
    let profile = local_profile(&server_dir)?;
    require_local_rcon(&profile)?;
    if enabled {
        let entries = rcon_command(&profile, "whitelist list")?;
        let count = Regex::new(r"(?i)There are\s+(\d+)\s+whitelisted player")
            .ok()
            .and_then(|pattern| pattern.captures(&entries))
            .and_then(|captures| captures.get(1))
            .and_then(|value| value.as_str().parse::<u32>().ok())
            .ok_or_else(|| "Le serveur n’a pas confirmé le nombre de joueurs autorisés. Ajoute d’abord un joueur et actualise la liste.".to_string())?;
        if count == 0 {
            return Err("Ajoute au moins un joueur avant d’activer la liste, pour éviter de te verrouiller dehors.".into());
        }
    }
    let response = rcon_command(
        &profile,
        if enabled {
            "whitelist on"
        } else {
            "whitelist off"
        },
    )?;
    Ok(response.trim().to_string())
}

pub fn prepare_pi_whitelist_migration(
    host: &str,
    username: &str,
    server_folder: &str,
) -> Result<String, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Adresse ou dossier du serveur Pi invalide.".into());
    }
    let destination = format!("{username}@{host}");
    let port_check = run_ssh(
        &destination,
        "for p in 25575 25576; do if ss -H -lntu 2>/dev/null | awk '{print $5}' | grep -qE '[:.]$p$'; then echo PORT_$p=busy; else echo PORT_$p=free; fi; done",
    )?;
    let values = port_check
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect::<std::collections::HashMap<_, _>>();
    let port = if values.get("PORT_25575") == Some(&"free") {
        25575
    } else if values.get("PORT_25576") == Some(&"free") {
        25576
    } else {
        return Err("Les deux ports internes de contrôle sont déjà utilisés sur le Pi. Rien n’a été modifié.".into());
    };
    let password = Uuid::new_v4().simple().to_string();
    let backup_id = Uuid::new_v4().simple().to_string();
    let command = format!(
        r#"set -eu
umask 077
d="$HOME/{server_folder}"
test -d "$d" && test ! -L "$d"
test -f "$d/monde-ensemble-profile.json" && test ! -L "$d/monde-ensemble-profile.json"
test -f "$d/server.properties" && test ! -L "$d/server.properties"
pid="$(cat "$d/serveur.pid" 2>/dev/null || true)"
case "$pid" in ''|*[!0-9]*) pid=;; esac
if test -n "$pid" && kill -0 "$pid" 2>/dev/null && test "$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)" = "$(readlink -f "$d")" && tr -d '\000' < "/proc/$pid/cmdline" 2>/dev/null | grep -q 'paper.jar'; then echo 'SERVER_RUNNING'; exit 9; fi
python3 - "$d" "{backup_id}" "{port}" "{password}" <<'PYCODE'
import json, os, pathlib, shutil, sys, tempfile
root=pathlib.Path(sys.argv[1]); suffix=sys.argv[2]; port=int(sys.argv[3]); password=sys.argv[4]
profile=root/'monde-ensemble-profile.json'; props=root/'server.properties'
with profile.open(encoding='utf-8') as f: data=json.load(f)
if data.get('application') != 'Monde Ensemble': raise SystemExit('PROFILE_NOT_MANAGED')
if 'rcon_port' in data and 'rcon_password' in data: raise SystemExit('ALREADY_CONFIGURED')
profile_backup=root/f'monde-ensemble-profile.json.before-rcon-{{suffix}}'
props_backup=root/f'server.properties.before-rcon-{{suffix}}'
shutil.copy2(profile, profile_backup); shutil.copy2(props, props_backup)
try:
    lines=props.read_text(encoding='utf-8').splitlines()
    lines=[line for line in lines if not line.startswith(('enable-rcon=','rcon.port=','rcon.password=','rcon.ip='))]
    lines += ['enable-rcon=true', f'rcon.port={port}', f'rcon.password={password}', 'rcon.ip=127.0.0.1']
    data['rcon_port']=port; data['rcon_password']=password
    fd, props_tmp=tempfile.mkstemp(prefix='.server.properties.', dir=root); os.close(fd)
    fd, profile_tmp=tempfile.mkstemp(prefix='.monde-ensemble-profile.', dir=root); os.close(fd)
    try:
        pathlib.Path(props_tmp).write_text('\n'.join(lines)+'\n', encoding='utf-8'); os.chmod(props_tmp, 0o600)
        pathlib.Path(profile_tmp).write_text(json.dumps(data, ensure_ascii=False, indent=2)+'\n', encoding='utf-8'); os.chmod(profile_tmp, 0o600)
        os.replace(props_tmp, props); os.replace(profile_tmp, profile)
    finally:
        for temp in (props_tmp, profile_tmp):
            try: os.unlink(temp)
            except FileNotFoundError: pass
    os.chmod(props, 0o600); os.chmod(profile, 0o600)
except BaseException:
    shutil.copy2(props_backup, props); shutil.copy2(profile_backup, profile)
    raise
print('MIGRATION_READY')
print(f'BACKUP=server.properties.before-rcon-{{suffix}}')
print(f'BACKUP=monde-ensemble-profile.json.before-rcon-{{suffix}}')
PYCODE"#
    );
    let output = run_ssh(&destination, &command).map_err(|error| {
        if error.contains("SERVER_RUNNING") {
            "Arrête d’abord le serveur depuis le tableau de bord. Aucun fichier n’a été modifié."
                .to_string()
        } else {
            format!("Migration Pi impossible; les mondes n’ont pas été touchés. {error}")
        }
    })?;
    if !output.lines().any(|line| line == "MIGRATION_READY") {
        return Err("Le Pi n’a pas confirmé la préparation du contrôle sécurisé. Les sauvegardes des réglages restent conservées.".into());
    }
    Ok("Réglages sécurisés préparés avec copies de sauvegarde. Redémarre maintenant le serveur depuis le tableau de bord pour activer la gestion des joueurs.".into())
}

pub fn pi_whitelist_status(
    host: &str,
    username: &str,
    server_folder: &str,
) -> Result<PiWhitelistStatus, String> {
    let (destination, profile, properties) =
        read_pi_management_state(host, username, server_folder)?;
    let enabled = properties
        .lines()
        .find_map(|line| line.strip_prefix("white-list="))
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"));
    let entries = with_pi_rcon(&destination, &profile, |tunnel_profile| {
        rcon_command(tunnel_profile, "whitelist list")
    })?;
    Ok(PiWhitelistStatus { enabled, entries })
}

pub fn change_pi_whitelist(
    host: &str,
    username: &str,
    server_folder: &str,
    platform: &str,
    action: &str,
    player_name: &str,
) -> Result<String, String> {
    if !matches!(action, "add" | "remove") {
        return Err("Action de liste d’accès invalide.".into());
    }
    let canonical_name = match platform {
        "java" if Regex::new(r"^[A-Za-z0-9_]{3,16}$").is_ok_and(|pattern| pattern.is_match(player_name)) => player_name.to_string(),
        "bedrock" if !player_name.trim().is_empty() && player_name.len() <= 16
            && player_name.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | ' ')) =>
                player_name.split_whitespace().collect::<Vec<_>>().join("_"),
        "java" => return Err("Nom Java invalide. Utilise 3 à 16 lettres, chiffres ou tirets bas.".into()),
        "bedrock" => return Err("Gamertag Bedrock invalide. Utilise jusqu’à 16 lettres, chiffres, espaces ou tirets bas.".into()),
        _ => return Err("Choix d’édition invalide.".into()),
    };
    let (destination, profile, _) = read_pi_management_state(host, username, server_folder)?;
    let command = if platform == "bedrock" {
        format!("fwhitelist {action} {canonical_name}")
    } else {
        format!("whitelist {action} {canonical_name}")
    };
    with_pi_rcon(&destination, &profile, |tunnel_profile| {
        rcon_command(tunnel_profile, &command)
    })
    .map(|response| response.trim().to_string())
}

pub fn set_pi_whitelist_enabled(
    host: &str,
    username: &str,
    server_folder: &str,
    enabled: bool,
) -> Result<String, String> {
    let (destination, profile, _) = read_pi_management_state(host, username, server_folder)?;
    with_pi_rcon(&destination, &profile, |tunnel_profile| {
        if enabled {
            let entries = rcon_command(tunnel_profile, "whitelist list")?;
            let count = Regex::new(r"(?i)There are\s+(\d+)\s+whitelisted player").ok()
                .and_then(|pattern| pattern.captures(&entries)).and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<u32>().ok())
                .ok_or_else(|| "Le serveur n’a pas confirmé le nombre de joueurs autorisés. Ajoute d’abord un joueur et actualise la liste.".to_string())?;
            if count == 0 {
                return Err("Ajoute au moins un joueur avant d’activer la liste, pour éviter de te verrouiller dehors.".into());
            }
        }
        rcon_command(
            tunnel_profile,
            if enabled {
                "whitelist on"
            } else {
                "whitelist off"
            },
        )
        .map(|response| response.trim().to_string())
    })
}

fn read_pi_management_state(
    host: &str,
    username: &str,
    server_folder: &str,
) -> Result<(String, serde_json::Value, String), String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Adresse ou dossier du serveur Pi invalide.".into());
    }
    let destination = format!("{username}@{host}");
    let command = format!(
        "set -eu; d=\"$HOME/{server_folder}\"; test -d \"$d\" && test ! -L \"$d\"; test -f \"$d/monde-ensemble-profile.json\" && test ! -L \"$d/monde-ensemble-profile.json\"; test -f \"$d/server.properties\" && test ! -L \"$d/server.properties\"; printf 'PROFILE_START:'; cat \"$d/monde-ensemble-profile.json\"; printf ':PROFILE_END:PROPERTIES_START:'; cat \"$d/server.properties\"; printf ':PROPERTIES_END:'"
    );
    let output = run_ssh(&destination, &command)?;
    let profile_text = output
        .split_once("PROFILE_START:")
        .and_then(|(_, rest)| rest.split_once(":PROFILE_END:PROPERTIES_START:"))
        .map(|(profile, _)| profile)
        .ok_or_else(|| "Le profil du serveur Pi est illisible.".to_string())?;
    let properties = output
        .split_once(":PROFILE_END:PROPERTIES_START:")
        .and_then(|(_, rest)| rest.split_once(":PROPERTIES_END:"))
        .map(|(properties, _)| properties.to_string())
        .ok_or_else(|| "Les réglages du serveur Pi sont illisibles.".to_string())?;
    let profile: serde_json::Value = serde_json::from_str(profile_text)
        .map_err(|_| "Le profil du serveur Pi est illisible.".to_string())?;
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
    {
        return Err("Ce dossier Pi n’a pas été créé par Monde Ensemble.".into());
    }
    if profile
        .get("rcon_port")
        .and_then(serde_json::Value::as_u64)
        .is_none()
        || profile
            .get("rcon_password")
            .and_then(serde_json::Value::as_str)
            .is_none()
    {
        return Err("Cette installation Pi est antérieure à la gestion sécurisée des joueurs. Arrête le serveur puis choisis « Préparer la gestion des joueurs »; deux copies des réglages seront conservées.".into());
    }
    Ok((destination, profile, properties))
}

fn with_pi_rcon<T>(
    destination: &str,
    profile: &serde_json::Value,
    operation: impl FnOnce(&serde_json::Value) -> Result<T, String>,
) -> Result<T, String> {
    require_local_rcon(profile)?;
    let remote_port = profile
        .get("rcon_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
        .ok_or_else(|| "Le port du contrôle sécurisé Pi est invalide.".to_string())?;
    let password = profile
        .get("rcon_password")
        .and_then(serde_json::Value::as_str)
        .filter(|password| !password.is_empty())
        .ok_or_else(|| "Le mot de passe du contrôle Pi est absent.".to_string())?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(io_error)?;
    let local_port = listener.local_addr().map_err(io_error)?.port();
    drop(listener);
    let forward = format!("127.0.0.1:{local_port}:127.0.0.1:{remote_port}");
    let mut child = ssh_command()
        .args([
            "-N",
            "-L",
            &forward,
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "ExitOnForwardFailure=yes",
            destination,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "L’outil SSH est indisponible sur cet ordinateur.".to_string())?;
    let ready = (0..50).any(|_| {
        if child.try_wait().ok().flatten().is_some() {
            return false;
        }
        if TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], local_port)),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
        false
    });
    if !ready {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Le tunnel sécurisé vers le contrôle du Pi n’a pas pu démarrer. Vérifie l’accès SSH au serveur.".into());
    }
    let mut tunnel_profile = profile.clone();
    if let Some(object) = tunnel_profile.as_object_mut() {
        object.insert("rcon_port".into(), serde_json::json!(local_port));
        object.insert("rcon_password".into(), serde_json::json!(password));
    }
    let result = operation(&tunnel_profile);
    let _ = child.kill();
    let _ = child.wait();
    result
}

pub fn export_local_server_log(server_dir: &Path, output_file: &Path) -> Result<String, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier de serveur local enregistré est introuvable.".to_string())?;
    let profile: serde_json::Value = serde_json::from_slice(
        &fs::read(server_dir.join("monde-ensemble-profile.json"))
            .map_err(|_| "Le profil du serveur local est illisible.".to_string())?,
    )
    .map_err(|_| "Le profil du serveur local est illisible.".to_string())?;
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
    {
        return Err("Ce dossier n’a pas été créé par Monde Ensemble.".into());
    }
    let source = server_dir.join("serveur.log");
    if !fs::symlink_metadata(&source)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
    {
        return Err("Le journal du serveur n’est pas un fichier normal; export bloqué.".into());
    }
    let mut input = File::open(&source)
        .map_err(|_| "Le journal complet du serveur local est introuvable.".to_string())?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_file)
        .map_err(|error| match error.kind() {
            io::ErrorKind::AlreadyExists => {
                "Ce fichier existe déjà. Choisis un autre nom pour ne rien écraser.".to_string()
            }
            _ => io_error(error),
        })?;
    io::copy(&mut input, &mut output).map_err(io_error)?;
    output.sync_all().map_err(io_error)?;
    Ok(format!(
        "Journal complet enregistré · {}",
        output_file.display()
    ))
}

pub fn export_pi_server_log(
    host: &str,
    username: &str,
    server_folder: &str,
    output_file: &Path,
) -> Result<String, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Le serveur Pi enregistré n’est pas reconnu.".into());
    }
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_file)
        .map_err(|error| match error.kind() {
            io::ErrorKind::AlreadyExists => {
                "Ce fichier existe déjà. Choisis un autre nom pour ne rien écraser.".to_string()
            }
            _ => io_error(error),
        })?;
    let mut cleanup = RemoveFileOnDrop(output_file.to_path_buf());
    let destination = format!("{username}@{host}");
    let command = format!(
        "p=\"$HOME/{server_folder}/serveur.log\"; test -f \"$p\" && test ! -L \"$p\" && cat -- \"$p\""
    );
    let mut child = ssh_command()
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "PasswordAuthentication=no",
            &destination,
            &command,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(output))
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "L’outil d’accès sécurisé SSH n’est pas disponible.".to_string())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(status) = child.try_wait().map_err(io_error)? {
            if !status.success() {
                let detail = child
                    .stderr
                    .take()
                    .and_then(|mut stream| {
                        let mut value = String::new();
                        stream.read_to_string(&mut value).ok().map(|_| value)
                    })
                    .unwrap_or_default();
                return Err(if detail.is_empty() {
                    "Le Pi n’a pas pu fournir son journal complet. Aucun fichier existant n’a été modifié.".into()
                } else {
                    format!(
                        "Le journal du Pi n’a pas été exporté : {}",
                        detail.chars().take(500).collect::<String>()
                    )
                });
            }
            cleanup.0 = PathBuf::new();
            return Ok(format!(
                "Journal complet enregistré · {}",
                output_file.display()
            ));
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "L’export du journal dépasse 45 secondes. Le fichier incomplet a été retiré."
                    .into(),
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Restarts only an application-managed local server and waits for its
/// authenticated RCON endpoint, Java port, and startup log before reporting ready.
pub fn start_local_server(
    server_dir: &Path,
    progress: impl Fn(&str, u8),
) -> Result<LocalStartResult, String> {
    start_local_server_impl(server_dir, &progress, true)
}

fn start_updated_local_copy_with_fallback(
    original_dir: &Path,
    updated_dir: &Path,
    backup_path: &Path,
    candidate_result: Result<LocalStartResult, String>,
    stop_candidate: impl FnOnce(&Path) -> Result<String, String>,
    start_original: impl FnOnce(&Path) -> Result<LocalStartResult, String>,
    progress: &impl Fn(&str, u8),
) -> Result<LocalStartResult, String> {
    match candidate_result {
        Ok(result) => Ok(LocalStartResult {
            server_path: result.server_path,
            message: format!(
                "Mise à jour coordonnée démarrée. L’ancien serveur reste conservé. Sauvegarde : {} · {}",
                backup_path.display(),
                result.message
            ),
        }),
        Err(candidate_error) => {
            progress(
                "La nouvelle copie n’a pas démarré; retour à la version précédente…",
                84,
            );
            let stop_result = stop_candidate(updated_dir);
            match start_original(original_dir) {
                Ok(result) => Ok(LocalStartResult {
                    server_path: result.server_path,
                    message: format!(
                        "Mise à jour reportée : la nouvelle copie n’a pas démarré. L’ancienne version a été relancée et vérifiée. Sauvegarde : {} · Copie candidate conservée pour diagnostic : {} · {}",
                        backup_path.display(),
                        updated_dir.display(),
                        result.message
                    ),
                }),
                Err(original_error) => {
                    let stop_detail = stop_result
                        .err()
                        .map(|error| format!(" Arrêt sécurisé de la copie candidate : {error}."))
                        .unwrap_or_default();
                    Err(format!(
                        "La mise à jour n’a pas démarré. L’ancienne version et sa sauvegarde restent intactes, mais le démarrage de l’ancienne version a aussi échoué. Sauvegarde : {} · Copie candidate : {} · Échec de la nouvelle copie : {}.{} Détail de l’ancienne version : {}",
                        backup_path.display(),
                        updated_dir.display(),
                        candidate_error,
                        stop_detail,
                        original_error
                    ))
                }
            }
        }
    }
}

fn start_local_server_impl(
    server_dir: &Path,
    progress: &impl Fn(&str, u8),
    allow_updates: bool,
) -> Result<LocalStartResult, String> {
    let server_dir = server_dir
        .canonicalize()
        .map_err(|_| "Le dossier de serveur local enregistré est introuvable.".to_string())?;
    let profile: serde_json::Value = serde_json::from_slice(
        &fs::read(server_dir.join("monde-ensemble-profile.json"))
            .map_err(|_| "Le profil du serveur local est illisible.".to_string())?,
    )
    .map_err(|_| "Le profil du serveur local est illisible.".to_string())?;
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
    {
        return Err("Ce dossier n’a pas été créé par Monde Ensemble.".into());
    }
    if profile
        .get("rcon_port")
        .and_then(serde_json::Value::as_u64)
        .is_none()
        || profile
            .get("rcon_password")
            .and_then(serde_json::Value::as_str)
            .is_none()
    {
        return Err("Ce serveur a été créé par une version antérieure et ne possède pas le contrôle local sécurisé. Il reste intact.".into());
    }
    if rcon_command(&profile, "list").is_ok() {
        return Ok(LocalStartResult {
            server_path: server_dir.display().to_string(),
            message: "Le serveur local est déjà démarré.".into(),
        });
    }
    let mut update_note = String::new();
    if allow_updates {
        match check_local_update(&server_dir) {
            Ok(check) if check.ready && check.update_available => {
                progress("Compatibilité prête; création d’une copie sauvegardée…", 5);
                match prepare_local_update_copy(&server_dir, &check, progress) {
                    Ok((updated_dir, backup)) => {
                        let candidate_result =
                            start_local_server_impl(&updated_dir, progress, false);
                        return start_updated_local_copy_with_fallback(
                            &server_dir,
                            &updated_dir,
                            &backup,
                            candidate_result,
                            stop_local_server,
                            |original| start_local_server_impl(original, progress, false),
                            progress,
                        );
                    }
                    Err(error) => {
                        update_note = format!(
                            "Mise à jour reportée; le serveur d’origine reste intact. {error} "
                        );
                    }
                }
            }
            Ok(check) if !check.ready => {
                update_note = format!(
                    "Mise à jour reportée pour protéger ce monde : {}. La version actuelle reste disponible. ",
                    check.reasons.join(" ")
                );
            }
            Ok(_) => {}
            Err(error) => {
                update_note = format!(
                    "Catalogue indisponible; aucune mise à jour appliquée. La version actuelle reste disponible. {error}. "
                );
            }
        }
    }
    let java_port = profile
        .get("java_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .ok_or_else(|| "Le port Java du serveur est illisible.".to_string())?;
    let bedrock_port = profile
        .get("bedrock_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .ok_or_else(|| "Le port Bedrock du serveur est illisible.".to_string())?;
    if !tcp_port_available(java_port)
        || !tcp_port_available(bedrock_port)
        || !udp_port_available(bedrock_port)
    {
        return Err("Un autre service utilise déjà un port réservé au serveur. Aucun processus n’a été démarré.".into());
    }
    let runtime_name = if cfg!(windows) { "java.exe" } else { "java" };
    let java = WalkDir::new(server_dir.join("runtime"))
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| entry.file_type().is_file() && entry.file_name() == runtime_name)
        .map(|entry| entry.into_path())
        .ok_or_else(|| "Le runtime Java inclus est introuvable.".to_string())?;
    let heap_mb = profile_heap_megabytes(&profile)
        .filter(|value| (128..=6144).contains(value))
        .ok_or_else(|| "La mémoire Java du profil est illisible.".to_string())?;
    let log_path = server_dir.join("serveur.log");
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(io_error)?;
    let stdout = log.try_clone().map_err(io_error)?;
    let mut command = Command::new(java);
    command
        .current_dir(&server_dir)
        .args([
            initial_heap_argument(heap_mb),
            format!("-Xmx{heap_mb}M"),
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
        command.creation_flags(0x00000008 | 0x00000200);
    }
    let child = command
        .spawn()
        .map_err(|error| format!("Java n’a pas pu démarrer : {error}"))?;
    fs::write(server_dir.join("serveur.pid"), child.id().to_string()).map_err(io_error)?;
    progress("Minecraft redémarre et contrôle les connexions…", 85);
    for attempt in 0..60 {
        let log_ready = fs::read_to_string(&log_path)
            .map(|contents| contents.contains("Done ("))
            .unwrap_or(false);
        if log_ready
            && tcp_port_open(java_port)
            && tcp_port_open(bedrock_port)
            && !udp_port_available(bedrock_port)
            && rcon_command(&profile, "list").is_ok()
        {
            progress("Serveur local prêt et contrôlé.", 100);
            return Ok(LocalStartResult {
                server_path: server_dir.display().to_string(),
                message: format!(
                    "{}Serveur démarré et vérifié · Java {} · Bedrock compatible {} · adresse {} · Java TCP {} · Bedrock TCP/UDP {}",
                    update_note,
                    profile
                        .get("minecraft_java")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("inconnue"),
                    profile
                        .get("minecraft_bedrock")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("inconnue"),
                    local_network_address().unwrap_or_else(|| "127.0.0.1".into()),
                    java_port,
                    bedrock_port
                ),
            });
        }
        progress(
            &format!("Minecraft démarre… vérification {}/60", attempt + 1),
            85 + attempt / 10,
        );
        std::thread::sleep(Duration::from_secs(2));
    }
    Err(format!(
        "Le serveur n’a pas confirmé son démarrage après deux minutes. Il n’a pas été arrêté de force. Consulte le journal : {}",
        log_path.display()
    ))
}

fn local_profile(server_dir: &Path) -> Result<serde_json::Value, String> {
    let profile: serde_json::Value = serde_json::from_slice(
        &fs::read(server_dir.join("monde-ensemble-profile.json"))
            .map_err(|_| "Le profil du serveur local est illisible.".to_string())?,
    )
    .map_err(|_| "Le profil du serveur local est illisible.".to_string())?;
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
    {
        return Err("Ce dossier n’a pas été créé par Monde Ensemble.".into());
    }
    Ok(profile)
}

fn require_local_rcon(profile: &serde_json::Value) -> Result<(), String> {
    if profile
        .get("rcon_port")
        .and_then(serde_json::Value::as_u64)
        .is_none()
        || profile
            .get("rcon_password")
            .and_then(serde_json::Value::as_str)
            .is_none()
    {
        return Err("Ce serveur a été créé par une version antérieure et ne possède pas le contrôle local sécurisé. Il reste intact.".into());
    }
    Ok(())
}

fn prepare_local_update_copy(
    server_dir: &Path,
    check: &PiUpdateCheck,
    progress: &impl Fn(&str, u8),
) -> Result<(PathBuf, PathBuf), String> {
    let profile = local_profile(server_dir)?;
    if rcon_command(&profile, "list").is_ok() {
        return Err(
            "Le serveur est encore actif; arrête-le proprement avant de le mettre à jour.".into(),
        );
    }
    let java_port = profile
        .get("java_port")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default() as u16;
    let bedrock_port = profile
        .get("bedrock_port")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default() as u16;
    if java_port == 0
        || bedrock_port == 0
        || !tcp_port_available(java_port)
        || !tcp_port_available(bedrock_port)
        || !udp_port_available(bedrock_port)
    {
        return Err("Un port Minecraft est utilisé. Aucune mise à jour n’a été appliquée.".into());
    }
    if !check.ready || !check.update_available {
        return Err("Les vérifications ne permettent pas cette mise à jour.".into());
    }
    for (relative, expected) in [
        ("monde-ensemble-profile.json", check.profile_sha256.as_str()),
        ("paper.jar", check.paper_sha256.as_str()),
        ("plugins/Geyser-Spigot.jar", check.geyser_sha256.as_str()),
        (
            "plugins/Floodgate-Spigot.jar",
            check.floodgate_sha256.as_str(),
        ),
    ] {
        if hash_regular_file(&server_dir.join(relative)).as_deref() != Some(expected) {
            return Err("Le serveur a changé depuis le contrôle de compatibilité; relance le contrôle. L’ancien dossier reste intact.".into());
        }
    }

    let parent = server_dir.parent().ok_or_else(|| {
        "Le dossier serveur n’a pas d’emplacement de sauvegarde valide.".to_string()
    })?;
    let id = Uuid::new_v4().simple().to_string();
    let backup_path = parent
        .join("sauvegardes-monde-ensemble")
        .join(format!("mise-a-jour-{id}.tar.gz"));
    progress("Archive complète du monde avant mise à jour…", 12);
    create_backup(server_dir, &backup_path, true, |step, percent| {
        progress(step, 12 + percent.saturating_mul(30) / 100)
    })?;

    let source_size = folder_size(server_dir)?;
    let free = fs2::available_space(parent).map_err(io_error)?;
    if free < source_size.saturating_add(512 * 1024 * 1024) {
        return Err(format!(
            "La sauvegarde est conservée dans {}, mais l’espace libre ne suffit pas pour préparer une copie complète. Le serveur d’origine reste intact.",
            backup_path.display()
        ));
    }

    progress(
        "Relecture des catalogues officiels avant téléchargement…",
        46,
    );
    let client = client()?;
    let supported = supported_versions(&client)?;
    let runtime = java_major_for(&supported.java)?;
    if supported.java != check.latest_java
        || supported.bedrock != check.latest_bedrock
        || profile
            .get("java_runtime")
            .and_then(serde_json::Value::as_u64)
            != Some(runtime as u64)
    {
        return Err("La compatibilité a changé ou demande un runtime Java différent. Le serveur actuel reste intact.".into());
    }
    let paper = paper_build(&client, &supported.java)?;
    let geyser = bridge_build(&client, "geyser", "spigot")?;
    let floodgate = bridge_build(&client, "floodgate", "spigot")?;
    if paper.id != check.latest_paper_build
        || geyser.build != check.latest_geyser_build
        || floodgate.build != check.latest_floodgate_build
    {
        return Err("Les builds officiels ont changé pendant la préparation. Relance le contrôle; l’ancien serveur reste intact.".into());
    }
    let paper_download = paper
        .downloads
        .get("server:default")
        .ok_or_else(|| "Le fichier Paper officiel est absent.".to_string())?;
    let geyser_download = geyser
        .downloads
        .get("spigot")
        .ok_or_else(|| "Le fichier Geyser officiel est absent.".to_string())?;
    let floodgate_download = floodgate
        .downloads
        .get("spigot")
        .ok_or_else(|| "Le fichier Floodgate officiel est absent.".to_string())?;
    let artifacts = [
        Artifact {
            name: "paper.jar".into(),
            version: format!("Paper {} build {}", supported.java, paper.id),
            url: paper_download.url.clone(),
            checksum: paper_download.checksums.get("sha256").cloned().ok_or_else(|| "L’empreinte Paper SHA-256 est absente.".to_string())?,
        },
        Artifact {
            name: "Geyser-Spigot.jar".into(),
            version: format!("Geyser {} build {}", geyser.version, geyser.build),
            url: "https://download.geysermc.org/v2/projects/geyser/versions/latest/builds/latest/downloads/spigot".into(),
            checksum: geyser_download.sha256.clone(),
        },
        Artifact {
            name: "Floodgate-Spigot.jar".into(),
            version: format!("Floodgate {} build {}", floodgate.version, floodgate.build),
            url: "https://download.geysermc.org/v2/projects/floodgate/versions/latest/builds/latest/downloads/spigot".into(),
            checksum: floodgate_download.sha256.clone(),
        },
    ];
    let artifact_stage = std::env::temp_dir().join(format!("monde-ensemble-local-update-{id}"));
    fs::create_dir(&artifact_stage).map_err(io_error)?;
    let _cleanup_artifacts = RemoveDirectoryOnDrop(artifact_stage.clone());
    for (index, artifact) in artifacts.iter().enumerate() {
        progress(
            &format!("Téléchargement et vérification de {}…", artifact.version),
            49 + index as u8 * 7,
        );
        download_verified(&client, artifact, &artifact_stage.join(&artifact.name))?;
    }

    let staging = parent.join(format!(".Monde-Ensemble-{id}.preparing"));
    fs::create_dir(&staging).map_err(io_error)?;
    let _cleanup_staging = RemoveDirectoryOnDrop(staging.clone());
    copy_server_tree(server_dir, &staging, progress)?;
    let staged_check = check_local_update(&staging)?;
    if !staged_check.ready
        || staged_check.latest_java != check.latest_java
        || staged_check.latest_bedrock != check.latest_bedrock
        || staged_check.latest_paper_build != check.latest_paper_build
        || staged_check.latest_geyser_build != check.latest_geyser_build
        || staged_check.latest_floodgate_build != check.latest_floodgate_build
        || staged_check.profile_sha256 != check.profile_sha256
        || staged_check.paper_sha256 != check.paper_sha256
        || staged_check.geyser_sha256 != check.geyser_sha256
        || staged_check.floodgate_sha256 != check.floodgate_sha256
    {
        return Err("Le serveur ou les catalogues ont changé pendant la copie. La mise à jour est annulée; le dossier d’origine reste intact.".into());
    }
    for artifact in &artifacts {
        let destination = match artifact.name.as_str() {
            "paper.jar" => staging.join("paper.jar"),
            "Geyser-Spigot.jar" => staging.join("plugins/Geyser-Spigot.jar"),
            "Floodgate-Spigot.jar" => staging.join("plugins/Floodgate-Spigot.jar"),
            _ => return Err("Un fichier de mise à jour inattendu a été reçu.".into()),
        };
        fs::copy(artifact_stage.join(&artifact.name), destination).map_err(io_error)?;
    }

    let geyser_jar = File::open(staging.join("plugins/Geyser-Spigot.jar")).map_err(io_error)?;
    let mut geyser_archive = zip::ZipArchive::new(geyser_jar).map_err(|error| {
        format!("Le nouveau fichier Geyser n’est pas une archive valide : {error}")
    })?;
    let template = geyser_config_from_archive(&mut geyser_archive)?;
    let config = managed_geyser_config(&template, bedrock_port);
    let config_path = staging.join("plugins/Geyser-Spigot/config.yml");
    fs::create_dir_all(config_path.parent().expect("parent de config Geyser")).map_err(io_error)?;
    fs::write(&config_path, config).map_err(io_error)?;

    let mut updated_profile = profile;
    record_managed_artifact_hashes(&mut updated_profile, &staging)?;
    let object = updated_profile
        .as_object_mut()
        .ok_or_else(|| "Le profil du serveur est invalide.".to_string())?;
    object.insert("minecraft_java".into(), serde_json::json!(supported.java));
    object.insert(
        "minecraft_bedrock".into(),
        serde_json::json!(supported.bedrock),
    );
    object.insert("paper_build_id".into(), serde_json::json!(paper.id));
    object.insert("geyser".into(), serde_json::json!(geyser.version));
    object.insert("geyser_build_id".into(), serde_json::json!(geyser.build));
    object.insert("floodgate".into(), serde_json::json!(floodgate.version));
    object.insert(
        "floodgate_build_id".into(),
        serde_json::json!(floodgate.build),
    );
    object.insert(
        "geyser_config_sha256".into(),
        serde_json::json!(hash_file(
            &staging.join("plugins/Geyser-Spigot/config.yml")
        )?),
    );
    object.insert("bedrock_transport".into(), serde_json::json!("nethernet"));
    object.insert(
        "updated_at".into(),
        serde_json::json!(Utc::now().to_rfc3339()),
    );
    fs::write(
        staging.join("monde-ensemble-profile.json"),
        serde_json::to_vec_pretty(&updated_profile).map_err(|error| error.to_string())?,
    )
    .map_err(io_error)?;

    let log_path = staging.join("serveur.log");
    if log_path.exists() {
        fs::rename(
            &log_path,
            staging.join(format!("serveur-avant-mise-a-jour-{id}.log")),
        )
        .map_err(io_error)?;
    }
    let pid_path = staging.join("serveur.pid");
    if pid_path.exists() {
        fs::remove_file(pid_path).map_err(io_error)?;
    }

    let updated_path = unique_folder(parent, "Monde-Ensemble");
    fs::rename(&staging, &updated_path).map_err(|error| {
        format!(
            "La copie mise à jour n’a pas pu être publiée; le serveur d’origine reste intact : {error}"
        )
    })?;
    progress(
        "Nouvelle copie créée; l’ancien dossier et sa sauvegarde restent conservés.",
        82,
    );
    Ok((updated_path, backup_path))
}

fn copy_server_tree(
    source: &Path,
    destination: &Path,
    progress: &impl Fn(&str, u8),
) -> Result<(), String> {
    let entries = WalkDir::new(source)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Impossible de lire le serveur à copier : {error}"))?;
    let count = entries.len().max(1);
    for (index, entry) in entries.iter().enumerate() {
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| error.to_string())?;
        let target = destination.join(relative);
        if entry.file_type().is_symlink() {
            return Err(format!(
                "Le serveur contient un lien symbolique ({}); copie bloquée.",
                entry.path().display()
            ));
        }
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(io_error)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(io_error)?;
            }
            fs::copy(entry.path(), &target).map_err(io_error)?;
        }
        if index % 64 == 0 || index + 1 == count {
            progress(
                "Copie vérifiée du serveur vers un nouveau dossier…",
                68 + ((index + 1) * 12 / count) as u8,
            );
        }
    }
    for directory in entries
        .iter()
        .rev()
        .filter(|entry| entry.file_type().is_dir())
    {
        let relative = directory
            .path()
            .strip_prefix(source)
            .map_err(|error| error.to_string())?;
        fs::set_permissions(
            destination.join(relative),
            fs::metadata(directory.path())
                .map_err(io_error)?
                .permissions(),
        )
        .map_err(io_error)?;
    }
    fs::set_permissions(
        destination,
        fs::metadata(source).map_err(io_error)?.permissions(),
    )
    .map_err(io_error)?;
    Ok(())
}

fn rcon_command(profile: &serde_json::Value, command: &str) -> Result<String, String> {
    let port = profile
        .get("rcon_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
        .ok_or_else(|| "Le port de contrôle sécurisé est absent.".to_string())?;
    let password = profile
        .get("rcon_password")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Le contrôle sécurisé de ce serveur n’est pas configuré.".to_string())?;
    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(2),
    )
    .map_err(|_| "Le serveur ne répond pas au contrôle sécurisé.".to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(io_error)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(io_error)?;
    write_rcon_packet(&mut stream, 1, 3, password)?;
    let (auth_id, _, _) = read_rcon_packet(&mut stream)?;
    if auth_id != 1 {
        return Err("Le serveur n’a pas confirmé son identité.".into());
    }
    write_rcon_packet(&mut stream, 2, 2, command)?;
    let (response_id, _, response) = read_rcon_packet(&mut stream)?;
    if response_id != 2 {
        return Err("Le serveur n’a pas confirmé la commande.".into());
    }
    Ok(response)
}

fn write_rcon_packet(
    stream: &mut TcpStream,
    id: i32,
    kind: i32,
    payload: &str,
) -> Result<(), String> {
    let mut packet = Vec::with_capacity(payload.len() + 14);
    packet.extend_from_slice(&id.to_le_bytes());
    packet.extend_from_slice(&kind.to_le_bytes());
    packet.extend_from_slice(payload.as_bytes());
    packet.push(0);
    packet.push(0);
    let length = i32::try_from(packet.len())
        .map_err(|_| "La commande de contrôle est trop longue.".to_string())?;
    stream.write_all(&length.to_le_bytes()).map_err(io_error)?;
    stream.write_all(&packet).map_err(io_error)
}

fn read_rcon_packet(stream: &mut TcpStream) -> Result<(i32, i32, String), String> {
    let mut size = [0; 4];
    stream.read_exact(&mut size).map_err(io_error)?;
    let size = i32::from_le_bytes(size);
    if !(10..=4096).contains(&size) {
        return Err("Le serveur a répondu avec un paquet de contrôle invalide.".into());
    }
    let mut packet = vec![0; size as usize];
    stream.read_exact(&mut packet).map_err(io_error)?;
    let id = i32::from_le_bytes([packet[0], packet[1], packet[2], packet[3]]);
    let kind = i32::from_le_bytes([packet[4], packet[5], packet[6], packet[7]]);
    let payload = String::from_utf8_lossy(&packet[8..packet.len().saturating_sub(2)]).to_string();
    Ok((id, kind, payload))
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
    let inspect = run_ssh(&destination, &remote_probe_command())?;
    let facts = parse_remote_facts(&inspect)?;
    if facts.architecture != "aarch64" {
        return Err(format!(
            "Ce Pi utilise {}. Le profil sécurisé demande Raspberry Pi OS 64 bits sur Raspberry Pi 3 ou plus récent.",
            facts.architecture
        ));
    }
    if facts.memory_gb < MINIMUM_PI_MEMORY_GB {
        return Err(format!(
            "{} n’a que {:.1} Go de mémoire détectée. Il faut un Pi 3B+ ou plus récent avec 1 Go de mémoire.",
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
    let rcon_port = select_remote_port(
        facts.rcon_default_busy,
        facts.rcon_alternate_busy,
        25575,
        25576,
    )?;
    let heap_mb = pi_heap_megabytes(facts.memory_gb);
    let id = Uuid::new_v4().simple().to_string();
    let local_stage = std::env::temp_dir().join(format!("monde-ensemble-{id}"));
    fs::create_dir(&local_stage).map_err(io_error)?;
    let _cleanup_stage = RemoveDirectoryOnDrop(local_stage.clone());
    progress("Préparation du profil Linux ARM64 compatible…", 8);
    let profile = build_fresh_server(
        &local_stage,
        heap_mb,
        true,
        java_port,
        bedrock_port,
        Some(rcon_port),
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

    // macOS may resolve a lowercase filename as though it were the expected
    // uppercase name. Inspect the archive's actual member names before sending
    // it to Linux, where those two paths are different files.
    validate_pi_server_package(&package_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&package_path, fs::Permissions::from_mode(0o600)).map_err(io_error)?;
    }
    let remote_archive = format!("Monde-Ensemble-{id}.tar.gz");
    progress("Envoi sécurisé des fichiers vérifiés au Pi…", 84);
    let scp = scp_command()
        .args([
            "-p",
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
        "set -eu; umask 077; chmod 600 \"$HOME/{remote_archive}\"; trap 'rm -f \"$HOME/{remote_archive}\"' EXIT; d=\"$HOME/Monde-Ensemble-{id}\"; mkdir \"$d\"; tar -xzf \"$HOME/{remote_archive}\" -C \"$d\"; rm \"$HOME/{remote_archive}\"; trap - EXIT; cd \"$d/server\"; {REMOTE_SERVER_ARTIFACT_CHECK} chmod 600 server.properties monde-ensemble-profile.json; AUTOSTART=no; if command -v crontab >/dev/null 2>&1 && (systemctl is-enabled --quiet cron 2>/dev/null || systemctl is-enabled --quiet crond 2>/dev/null); then t=\"$(mktemp)\"; (crontab -l 2>/dev/null || true) | grep -Fv '# MONDE_ENSEMBLE_AUTO_START' > \"$t\" || true; printf '%s\\n' '@reboot \"$HOME/Monde-Ensemble-{id}/server/demarrer-au-redemarrage.sh\" # MONDE_ENSEMBLE_AUTO_START' >> \"$t\"; crontab \"$t\"; rm -f \"$t\"; if crontab -l 2>/dev/null | grep -Fq '# MONDE_ENSEMBLE_AUTO_START'; then AUTOSTART=yes; fi; fi; nohup ./demarrer.sh > serveur.log 2>&1 < /dev/null & echo $! > serveur.pid; printf 'AUTOSTART=%s\\n' \"$AUTOSTART\""
    );
    run_ssh(&destination, &prepare_command).map_err(|error| {
        format!("Les fichiers ont été transférés, mais leur préparation a échoué dans ~/Monde-Ensemble-{id}/server. Rien n’a été écrit dans les autres dossiers du Pi. Détail : {error}")
    })?;
    // A Pi 3B+ can take more than seven minutes on its first world generation
    // and Geyser bootstrap. Keep polling long enough for that real startup,
    // while still bounding a genuinely failed launch.
    let startup_attempts = if heap_mb <= COMPACT_PI_HEAP_MB {
        360
    } else {
        180
    };
    let mut started = false;
    for attempt in 0..startup_attempts {
        let check_command = format!(
            "cd \"$HOME/Monde-Ensemble-{id}/server\"; if grep -Eq '{PAPER_READY_LOG_PATTERN}' serveur.log; then echo READY; elif kill -0 \"$(cat serveur.pid)\" 2>/dev/null; then echo STARTING; else tail -n 60 serveur.log; exit 3; fi"
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
            92 + ((attempt * 7) / startup_attempts) as u8,
        );
        std::thread::sleep(Duration::from_secs(2));
    }
    if !started {
        return Err(format!(
            "Les fichiers du serveur ont été installés, mais le Pi n’a pas confirmé son démarrage dans le délai prévu. Consulte `~/Monde-Ensemble-{id}/server/serveur.log` sur le Pi."
        ));
    }
    progress("Contrôle du pont Bedrock et de NetherNet…", 98);
    let mut bedrock_ready = false;
    for attempt in 0..60 {
        let check_command = format!(
            "cd \"$HOME/Monde-Ensemble-{id}/server\"; if grep -q 'Done (' serveur.log && grep -Eq '^[[:space:]]*transport:[[:space:]]*nethernet([[:space:]#]|$)' plugins/Geyser-Spigot/config.yml && ss -H -lnt 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.]({bedrock_port})$' && ss -H -lnu 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.]({bedrock_port})$'; then echo BEDROCK_READY; else echo BEDROCK_STARTING; fi"
        );
        let status = run_ssh(&destination, &check_command).map_err(|error| {
            format!("Paper a démarré, mais Geyser n’a pas encore confirmé NetherNet. Le monde reste sur le Pi. Détail : {error}")
        })?;
        if status.lines().any(|line| line == "BEDROCK_READY") {
            bedrock_ready = true;
            break;
        }
        progress(
            &format!(
                "Geyser prépare la connexion Bedrock… vérification {}/60",
                attempt + 1
            ),
            98,
        );
        std::thread::sleep(Duration::from_secs(2));
    }
    if !bedrock_ready {
        return Err(format!(
            "Paper a démarré, mais Geyser n’a pas confirmé NetherNet sur le port Bedrock {bedrock_port} en TCP et UDP. Le monde et les fichiers installés restent sur le Pi; consulte `~/Monde-Ensemble-{id}/server/serveur.log`."
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
    let network_identity = match (facts.network_interface.as_str(), facts.mac_address.as_str()) {
        (interface, mac_address) if !mac_address.is_empty() => {
            format!(" · interface {interface} · adresse matérielle {mac_address}")
        }
        (interface, _) if !interface.is_empty() => format!(" · interface {interface}"),
        _ => String::new(),
    };
    progress("Serveur démarré et contrôlé sur le Raspberry Pi.", 100);
    Ok(OperationResult {
        path: format!("{username}@{host}:~/Monde-Ensemble-{id}/server"),
        detail: format!(
            "Serveur démarré et prêt · démarrage automatique après redémarrage {} · {} · Minecraft Java {} · Bedrock compatible {} · Geyser {} · Floodgate {} · Java {} · mémoire serveur {} Go · Chez toi, PC/Mac Java : {ip}:{} · mobile/tablette Bedrock : {ip}:{} · Depuis l’extérieur, le routeur doit aussi autoriser ces ports.{network_identity}",
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
            format_heap(heap_mb),
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
    // First-boot setup gives this card a random, app-owned hostname. Try it
    // directly before waiting for DNS-SD; some home routers resolve `.local`
    // names but do not advertise the SSH service consistently.
    if let Some(expected) = expected_hostname {
        let host = format!("{expected}.local");
        if let Ok(output) = run_ssh_timeout_policy(
            &format!("{username}@{host}"),
            &remote_probe_command(),
            3,
            true,
        ) {
            if let Ok(facts) = parse_remote_facts(&output) {
                return Ok(vec![DiscoveredPi {
                    host,
                    address: facts.ip,
                    model: facts.model,
                }]);
            }
        }
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
                &remote_probe_command(),
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
                address: if facts.ip.is_empty() {
                    address
                } else {
                    facts.ip
                },
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
/// Keeps the app-managed reboot entry pointed at the selected server copy.
/// Other entries in the user's crontab are preserved byte-for-byte by line.
fn configure_pi_autostart(host: &str, username: &str, server_folder: &str) -> Result<bool, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Ok(false);
    }
    let destination = format!("{username}@{host}");
    let command = format!(
        "set -eu; command -v crontab >/dev/null 2>&1 || {{ echo AUTOSTART=no; exit 0; }}; if ! (systemctl is-enabled --quiet cron 2>/dev/null || systemctl is-enabled --quiet crond 2>/dev/null); then echo AUTOSTART=no; exit 0; fi; t=\"$(mktemp)\"; trap 'rm -f -- \"$t\"' EXIT; (crontab -l 2>/dev/null || true) | grep -Fv '# MONDE_ENSEMBLE_AUTO_START' > \"$t\" || true; printf '%s\\n' '@reboot \"$HOME/{server_folder}/demarrer-au-redemarrage.sh\" # MONDE_ENSEMBLE_AUTO_START' >> \"$t\"; crontab \"$t\"; if crontab -l 2>/dev/null | grep -Fq '@reboot \"$HOME/{server_folder}/demarrer-au-redemarrage.sh\" # MONDE_ENSEMBLE_AUTO_START'; then echo AUTOSTART=yes; else echo AUTOSTART=no; fi"
    );
    let output = run_ssh(&destination, &command)?;
    Ok(output.lines().any(|line| line.trim() == "AUTOSTART=yes"))
}

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
        "d=\"$HOME/{server_folder}\"; test -d \"$d\" || {{ echo 'DOSSIER_ABSENT'; exit 5; }}; printf 'RUNNING='; pid=; if test -r \"$d/serveur.pid\"; then pid=\"$(cat \"$d/serveur.pid\")\"; fi; case \"$pid\" in ''|*[!0-9]*) pid=;; esac; if test -n \"$pid\" && kill -0 \"$pid\" 2>/dev/null && test \"$(awk '{{print $3}}' \"/proc/$pid/stat\" 2>/dev/null || true)\" != Z && test \"$(readlink -f \"/proc/$pid/cwd\" 2>/dev/null || true)\" = \"$(readlink -f \"$d\")\" && tr -d '\\000' < \"/proc/$pid/cmdline\" 2>/dev/null | grep -q 'paper.jar'; then echo yes; else echo no; fi; printf 'READY='; if grep -q 'Done (' \"$d/serveur.log\" 2>/dev/null; then echo yes; else echo no; fi; printf 'JAVA_PORT='; if ss -H -lnt 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.](25565|25566)$'; then echo yes; else echo no; fi; printf 'BEDROCK_TCP='; if ss -H -lnt 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.](19132|19133)$'; then echo yes; else echo no; fi; printf 'BEDROCK_PORT='; if ss -H -lnu 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.](19132|19133)$'; then echo yes; else echo no; fi; printf 'BEDROCK_NETHERNET='; if grep -Eq '^[[:space:]]*transport:[[:space:]]*nethernet([[:space:]#]|$)' \"$d/plugins/Geyser-Spigot/config.yml\" 2>/dev/null; then echo yes; else echo no; fi; printf 'AUTOSTART='; if crontab -l 2>/dev/null | grep -Fq '@reboot \"$HOME/{server_folder}/demarrer-au-redemarrage.sh\" # MONDE_ENSEMBLE_AUTO_START'; then echo yes; else echo no; fi; @NETWORK_IDENTITY@; printf 'PROFILE_START\\n'; cat \"$d/monde-ensemble-profile.json\" 2>/dev/null || true; printf '\\nPROFILE_END\\n'; printf 'LOG_START\\n'; tail -n 200 \"$d/serveur.log\" 2>/dev/null || true"
    );
    let command = command.replace("@NETWORK_IDENTITY@", REMOTE_NETWORK_IDENTITY_COMMAND);
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
        ready: get("RUNNING=") == "yes"
            && get("READY=") == "yes"
            && get("JAVA_PORT=") == "yes"
            && get("BEDROCK_TCP=") == "yes"
            && get("BEDROCK_PORT=") == "yes"
            && get("BEDROCK_NETHERNET=") == "yes",
        java_port_open: get("JAVA_PORT=") == "yes",
        bedrock_tcp_port_open: get("BEDROCK_TCP=") == "yes",
        bedrock_port_open: get("BEDROCK_PORT=") == "yes",
        bedrock_nethernet_ready: get("BEDROCK_NETHERNET=") == "yes",
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
        paper_build_id: profile
            .get("paper_build_id")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        geyser_version: profile
            .get("geyser")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        geyser_build_id: profile
            .get("geyser_build_id")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        floodgate_version: profile
            .get("floodgate")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        floodgate_build_id: profile
            .get("floodgate_build_id")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        java_port: profile
            .get("java_port")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(JAVA_ALTERNATE_PORT as u64) as u16,
        bedrock_port: profile
            .get("bedrock_port")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(BEDROCK_ALTERNATE_PORT as u64) as u16,
        address: get("IP=").to_string(),
        network_interface: get("NETWORK_INTERFACE=").to_string(),
        mac_address: get("MAC_ADDRESS=").to_string(),
        log: log.to_string(),
    })
}

/// Read-only update gate for a server created by Monde Ensemble. It only
/// approves the managed Paper/Geyser/Floodgate stack; any unrecognized plugin,
/// mod jar, symlink, or custom client pack blocks automatic updates.
pub fn check_pi_update(
    host: &str,
    username: &str,
    server_folder: &str,
) -> Result<PiUpdateCheck, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Le dossier enregistré du serveur n’est pas reconnu.".into());
    }
    let latest = compatibility()?;
    let destination = format!("{username}@{host}");
    let command = format!(
        "set -eu; d=\"$HOME/{server_folder}\"; test -s \"$d/monde-ensemble-profile.json\"; printf 'PROFILE_START\\n'; cat \"$d/monde-ensemble-profile.json\"; printf '\\nPROFILE_END\\nPROFILE_SHA256='; sha256sum \"$d/monde-ensemble-profile.json\" | cut -d' ' -f1; printf 'FILES_PAPER='; test -s \"$d/paper.jar\" && echo yes || echo no; printf 'FILES_GEYSER='; test -s \"$d/plugins/Geyser-Spigot.jar\" && echo yes || echo no; printf 'FILES_FLOODGATE='; test -s \"$d/plugins/Floodgate-Spigot.jar\" && echo yes || echo no; printf 'PAPER_SHA256='; sha256sum \"$d/paper.jar\" 2>/dev/null | cut -d' ' -f1 || true; printf 'GEYSER_SHA256='; sha256sum \"$d/plugins/Geyser-Spigot.jar\" 2>/dev/null | cut -d' ' -f1 || true; printf 'FLOODGATE_SHA256='; sha256sum \"$d/plugins/Floodgate-Spigot.jar\" 2>/dev/null | cut -d' ' -f1 || true; printf 'GEYSER_CONFIG_SHA256='; sha256sum \"$d/plugins/Geyser-Spigot/config.yml\" 2>/dev/null | cut -d' ' -f1 || true; printf 'GEYSER_NETHERNET='; if grep -Eq '^[[:space:]]*transport:[[:space:]]*nethernet([[:space:]#]|$)' \"$d/plugins/Geyser-Spigot/config.yml\" 2>/dev/null; then echo yes; else echo no; fi; printf 'JARS_START\\n'; find \"$d/plugins\" -maxdepth 1 -type f -name '*.jar' -printf '%f\\n' 2>/dev/null | sort || true; printf 'JARS_END\\nMODS_START\\n'; find \"$d/mods\" -type f -name '*.jar' -printf '%P\\n' 2>/dev/null | sort || true; printf 'MODS_END\\nDATAPACKS_START\\n'; find \"$d/world/datapacks\" -type f -not -name '.*' -printf '%P\\n' 2>/dev/null | sort || true; printf 'DATAPACKS_END\\nGEYSER_PACKS='; if find \"$d/plugins/Geyser-Spigot/packs\" -type f -not -name '.*' -print -quit 2>/dev/null | grep -q .; then echo yes; else echo no; fi; printf 'SYMLINKS='; if find \"$d\" -type l -print -quit | grep -q .; then echo yes; else echo no; fi; printf 'RESOURCE_PACK='; sed -n 's/^resource-pack=//p' \"$d/server.properties\" | head -1"
    );
    let output = run_ssh(&destination, &command)?;
    let (_, profile_block) = output
        .split_once("PROFILE_START\n")
        .ok_or_else(|| "Le profil du serveur n’a pas été reçu.".to_string())?;
    let (profile_text, tail) = profile_block
        .split_once("\nPROFILE_END\n")
        .ok_or_else(|| "Le profil du serveur est incomplet.".to_string())?;
    let (_, jars_block) = tail
        .split_once("JARS_START\n")
        .ok_or_else(|| "La liste des extensions n’a pas été reçue.".to_string())?;
    let (jars, tail) = jars_block
        .split_once("JARS_END\n")
        .ok_or_else(|| "La liste des extensions est incomplète.".to_string())?;
    let (_, mods_block) = tail
        .split_once("MODS_START\n")
        .ok_or_else(|| "La liste des mods n’a pas été reçue.".to_string())?;
    let (mods, tail) = mods_block
        .split_once("MODS_END\n")
        .ok_or_else(|| "La liste des mods est incomplète.".to_string())?;
    let (_, datapacks_block) = tail
        .split_once("DATAPACKS_START\n")
        .ok_or_else(|| "La liste des packs du monde n’a pas été reçue.".to_string())?;
    let (datapacks, tail) = datapacks_block
        .split_once("DATAPACKS_END\n")
        .ok_or_else(|| "La liste des packs du monde est incomplète.".to_string())?;
    let profile: serde_json::Value = serde_json::from_str(profile_text.trim())
        .map_err(|_| "Le profil des versions Minecraft est illisible.".to_string())?;
    let field_text = |name: &str| {
        profile
            .get(name)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("inconnue")
            .to_string()
    };
    let current_java = field_text("minecraft_java");
    let current_bedrock = field_text("minecraft_bedrock");
    let current_geyser = field_text("geyser");
    let current_floodgate = field_text("floodgate");
    let latest_geyser = latest
        .geyser_version
        .split(" · build ")
        .next()
        .unwrap_or(&latest.geyser_version)
        .to_string();
    let latest_floodgate = latest
        .floodgate_version
        .split(" · build ")
        .next()
        .unwrap_or(&latest.floodgate_version)
        .to_string();
    let current_paper_build = profile
        .get("paper_build_id")
        .and_then(serde_json::Value::as_u64);
    let current_geyser_build = profile
        .get("geyser_build_id")
        .and_then(serde_json::Value::as_u64);
    let current_floodgate_build = profile
        .get("floodgate_build_id")
        .and_then(serde_json::Value::as_u64);
    let output_field = |key: &str| {
        output
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let profile_sha256 = output_field("PROFILE_SHA256=");
    let paper_sha256 = output_field("PAPER_SHA256=");
    let geyser_sha256 = output_field("GEYSER_SHA256=");
    let floodgate_sha256 = output_field("FLOODGATE_SHA256=");
    let geyser_config_sha256 = output_field("GEYSER_CONFIG_SHA256=");
    let bedrock_transport_migration = output_field("GEYSER_NETHERNET=") != "yes";
    let update_available = bedrock_transport_migration
        || current_java != latest.java_version
        || current_bedrock != latest.bedrock_versions
        || current_geyser != latest_geyser
        || current_floodgate != latest_floodgate
        || current_paper_build != Some(latest.paper_build_id)
        || current_geyser_build != Some(latest.geyser_build_id)
        || current_floodgate_build != Some(latest.floodgate_build_id);

    let mut reasons = Vec::new();
    match profile
        .get("geyser_config_sha256")
        .and_then(serde_json::Value::as_str)
    {
        Some(expected) if valid_sha256(expected) && expected == geyser_config_sha256 => {}
        Some(_) => reasons.push("La configuration Geyser du Pi a changé depuis son enregistrement; aucune mise à jour automatique ne la remplacera.".into()),
        None => reasons.push("La configuration Geyser de cette ancienne installation ne possède pas d’empreinte vérifiable; elle reste intacte.".into()),
    }
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
    {
        reasons.push("Cette installation n’a pas un profil Monde Ensemble reconnu.".into());
    }
    if !valid_sha256(&profile_sha256) {
        reasons.push("L’empreinte du profil n’a pas pu être vérifiée.".into());
    }
    let hash_fields = ["paper_sha256", "geyser_sha256", "floodgate_sha256"];
    let legacy_hashes = if hash_fields
        .iter()
        .any(|field| profile.get(*field).is_none())
    {
        client()
            .and_then(|client| legacy_stack_checksums(&client, &profile))
            .ok()
    } else {
        None
    };
    for (index, (name, field, actual)) in [
        ("Paper", hash_fields[0], paper_sha256.as_str()),
        ("Geyser", hash_fields[1], geyser_sha256.as_str()),
        ("Floodgate", hash_fields[2], floodgate_sha256.as_str()),
    ]
    .into_iter()
    .enumerate()
    {
        let recorded = profile.get(field).and_then(serde_json::Value::as_str);
        let official_legacy = legacy_hashes.as_ref().map(|hashes| hashes[index].as_str());
        if let Err(reason) = artifact_hash_matches(name, actual, recorded, official_legacy) {
            reasons.push(reason);
        }
    }
    for (key, label) in [
        ("FILES_PAPER=yes", "Paper"),
        ("FILES_GEYSER=yes", "Geyser"),
        ("FILES_FLOODGATE=yes", "Floodgate"),
    ] {
        if !output.lines().any(|line| line.trim() == key) {
            reasons.push(format!("Le fichier géré {label} est absent ou illisible."));
        }
    }
    let plugin_jars = jars
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|name| !matches!(*name, "Geyser-Spigot.jar" | "Floodgate-Spigot.jar"))
        .collect::<Vec<_>>();
    if !plugin_jars.is_empty() {
        reasons.push(format!(
            "Extensions non vérifiées : {}. Elles bloquent la mise à jour automatique pour protéger le monde.",
            plugin_jars.join(", ")
        ));
    }
    let mod_jars = mods
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if !mod_jars.is_empty() {
        reasons.push(format!(
            "Mods présents : {}. Leur compatibilité n’est pas vérifiée par l’application.",
            mod_jars.join(", ")
        ));
    }
    let custom_datapacks = datapacks
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if !custom_datapacks.is_empty() {
        reasons.push(format!(
            "Packs de données présents : {}. Leur compatibilité n’est pas vérifiée.",
            custom_datapacks.join(", ")
        ));
    }
    if tail.contains("SYMLINKS=yes") {
        reasons
            .push("Le serveur contient des liens symboliques; la copie sûre est bloquée.".into());
    }
    if tail.contains("GEYSER_PACKS=yes") {
        reasons.push("Des packs Bedrock personnalisés sont présents; leur compatibilité doit être vérifiée avant la mise à jour.".into());
    }
    if tail
        .lines()
        .find_map(|line| line.strip_prefix("RESOURCE_PACK="))
        .is_some_and(|value| !value.trim().is_empty())
    {
        reasons.push("Un pack de ressources personnalisé est configuré; sa compatibilité doit être vérifiée avant toute mise à jour.".into());
    }

    Ok(PiUpdateCheck {
        ready: reasons.is_empty(),
        update_available,
        bedrock_transport_migration,
        current_java,
        current_bedrock,
        current_geyser,
        current_floodgate,
        latest_java: latest.java_version,
        latest_bedrock: latest.bedrock_versions,
        latest_geyser,
        latest_geyser_build: latest.geyser_build_id,
        latest_floodgate,
        latest_floodgate_build: latest.floodgate_build_id,
        latest_paper_build: latest.paper_build_id,
        profile,
        profile_sha256,
        paper_sha256,
        geyser_sha256,
        floodgate_sha256,
        reasons,
        checked_at: latest.checked_at,
    })
}

/// Applies the newest official Paper/Geyser/Floodgate bundle to an idle,
/// managed Pi server. The full server folder is archived before replacement;
/// the new server must pass startup checks or the archive is restored.
fn update_pi_server_if_ready(
    host: &str,
    username: &str,
    server_folder: &str,
    check: &PiUpdateCheck,
    progress: &impl Fn(&str, u8),
) -> Result<Option<String>, PiUpdateFailure> {
    if !check.ready || !check.update_available {
        return Ok(None);
    }

    progress(
        "Préparation des versions officielles et vérification des fichiers…",
        10,
    );
    let client = client()?;
    let supported = supported_versions(&client)?;
    if supported.java != check.latest_java || supported.bedrock != check.latest_bedrock {
        return Err("Le catalogue de compatibilité a changé pendant la préparation. Aucun fichier du serveur n’a été modifié; réessaie.".to_string().into());
    }
    let required_java_runtime = java_major_for(&supported.java).map_err(PiUpdateFailure::from)?;
    if check
        .profile
        .get("java_runtime")
        .and_then(serde_json::Value::as_u64)
        != Some(required_java_runtime as u64)
    {
        return Err(PiUpdateFailure::Safe(
            "Cette mise à jour demanderait un autre runtime Java. Le serveur actuel reste en place jusqu’à ce que l’application sache aussi mettre Java à niveau.".into(),
        ));
    }
    let paper = paper_build(&client, &supported.java)?;
    let geyser = bridge_build(&client, "geyser", "spigot")?;
    let floodgate = bridge_build(&client, "floodgate", "spigot")?;
    if paper.id != check.latest_paper_build
        || geyser.build != check.latest_geyser_build
        || floodgate.build != check.latest_floodgate_build
    {
        return Err("Les versions officielles ont changé pendant la préparation. Aucun fichier du serveur n’a été modifié; relance la vérification.".to_string().into());
    }
    let paper_download = paper.downloads.get("server:default").ok_or_else(|| {
        "Le fichier Paper officiel est introuvable; serveur inchangé.".to_string()
    })?;
    let geyser_download = geyser.downloads.get("spigot").ok_or_else(|| {
        "Le fichier Geyser officiel est introuvable; serveur inchangé.".to_string()
    })?;
    let floodgate_download = floodgate.downloads.get("spigot").ok_or_else(|| {
        "Le fichier Floodgate officiel est introuvable; serveur inchangé.".to_string()
    })?;
    let paper_checksum = paper_download.checksums.get("sha256").ok_or_else(|| {
        "Paper n’a pas publié d’empreinte de contrôle; serveur inchangé.".to_string()
    })?;

    let id = Uuid::new_v4().simple().to_string();
    let staging = std::env::temp_dir().join(format!("monde-ensemble-update-{id}"));
    fs::create_dir(&staging).map_err(io_error)?;
    let _cleanup_staging = RemoveDirectoryOnDrop(staging.clone());
    fs::create_dir(staging.join("plugins")).map_err(io_error)?;
    let artifacts = [
        Artifact {
            name: "paper.jar".into(),
            version: format!("Paper {} build {}", supported.java, paper.id),
            url: paper_download.url.clone(),
            checksum: paper_checksum.clone(),
        },
        Artifact {
            name: "Geyser-Spigot.jar".into(),
            version: format!("Geyser {} build {}", geyser.version, geyser.build),
            url: "https://download.geysermc.org/v2/projects/geyser/versions/latest/builds/latest/downloads/spigot".into(),
            checksum: geyser_download.sha256.clone(),
        },
        Artifact {
            name: "Floodgate-Spigot.jar".into(),
            version: format!("Floodgate {} build {}", floodgate.version, floodgate.build),
            url: "https://download.geysermc.org/v2/projects/floodgate/versions/latest/builds/latest/downloads/spigot".into(),
            checksum: floodgate_download.sha256.clone(),
        },
    ];
    let destinations = [
        staging.join("paper.jar"),
        staging.join("plugins/Geyser-Spigot.jar"),
        staging.join("plugins/Floodgate-Spigot.jar"),
    ];
    for (index, (artifact, destination)) in artifacts.iter().zip(&destinations).enumerate() {
        progress(
            &format!("Téléchargement et contrôle de {}…", artifact.version),
            15 + index as u8 * 15,
        );
        download_verified(&client, artifact, destination)?;
    }

    let geyser_jar = File::open(staging.join("plugins/Geyser-Spigot.jar")).map_err(io_error)?;
    let mut geyser_archive = zip::ZipArchive::new(geyser_jar).map_err(|error| {
        format!("Le nouveau fichier Geyser n’est pas une archive valide : {error}")
    })?;
    let template = geyser_config_from_archive(&mut geyser_archive)?;
    let bedrock_port = check
        .profile
        .get("bedrock_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
        .ok_or_else(|| {
            PiUpdateFailure::Safe(
                "Le port Bedrock du profil est invalide; le serveur actuel reste intact.".into(),
            )
        })?;
    let config_path = staging.join("plugins/Geyser-Spigot/config.yml");
    fs::create_dir_all(config_path.parent().expect("parent de config Geyser")).map_err(io_error)?;
    fs::write(&config_path, managed_geyser_config(&template, bedrock_port)).map_err(io_error)?;

    let mut profile = check.profile.clone();
    record_managed_artifact_hashes(&mut profile, &staging)?;
    let object = profile.as_object_mut().ok_or_else(|| {
        "Le profil du serveur n’est pas un objet lisible; serveur inchangé.".to_string()
    })?;
    object.insert("minecraft_java".into(), serde_json::json!(supported.java));
    object.insert(
        "minecraft_bedrock".into(),
        serde_json::json!(supported.bedrock),
    );
    object.insert("paper_build_id".into(), serde_json::json!(paper.id));
    object.insert("geyser".into(), serde_json::json!(geyser.version));
    object.insert("geyser_build_id".into(), serde_json::json!(geyser.build));
    object.insert("floodgate".into(), serde_json::json!(floodgate.version));
    object.insert(
        "floodgate_build_id".into(),
        serde_json::json!(floodgate.build),
    );
    object.insert(
        "geyser_config_sha256".into(),
        serde_json::json!(hash_file(&config_path)?),
    );
    object.insert("bedrock_transport".into(), serde_json::json!("nethernet"));
    object.insert(
        "updated_at".into(),
        serde_json::json!(Utc::now().to_rfc3339()),
    );
    fs::write(
        staging.join("monde-ensemble-profile.json"),
        serde_json::to_vec_pretty(&profile).map_err(|error| error.to_string())?,
    )
    .map_err(io_error)?;

    let mut checksums = String::new();
    for name in [
        "paper.jar",
        "plugins/Geyser-Spigot.jar",
        "plugins/Floodgate-Spigot.jar",
        "plugins/Geyser-Spigot/config.yml",
        "monde-ensemble-profile.json",
    ] {
        let checksum = hash_file(&staging.join(name))?;
        checksums.push_str(&format!("{checksum}  {name}\n"));
    }
    fs::write(staging.join("SHA256SUMS"), checksums).map_err(io_error)?;

    let archive_path = staging.with_extension("tar.gz");
    let _cleanup_archive = RemoveFileOnDrop(archive_path.clone());
    let archive_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&archive_path)
        .map_err(io_error)?;
    let mut archive = Builder::new(GzEncoder::new(archive_file, Compression::fast()));
    archive
        .append_dir_all("update", &staging)
        .map_err(io_error)?;
    archive
        .into_inner()
        .map_err(io_error)?
        .finish()
        .map_err(io_error)?
        .sync_all()
        .map_err(io_error)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&archive_path, fs::Permissions::from_mode(0o600)).map_err(io_error)?;
    }
    let destination = format!("{username}@{host}");
    let remote_archive = format!("Monde-Ensemble-update-{id}.tar.gz");
    progress("Envoi chiffré des fichiers contrôlés au Pi…", 65);
    let scp = scp_command()
        .args([
            "-p",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "PasswordAuthentication=no",
            archive_path.to_string_lossy().as_ref(),
            &format!("{destination}:{remote_archive}"),
        ])
        .output()
        .map_err(|_| "L’outil de transfert sécurisé n’est pas disponible.".to_string())?;
    if !scp.status.success() {
        return Err(
            "Le transfert de la mise à jour a échoué; le serveur n’a pas été modifié."
                .to_string()
                .into(),
        );
    }

    let parent_folder = server_folder
        .strip_suffix("/server")
        .ok_or_else(|| "Le dossier de serveur enregistré n’est pas reconnu.".to_string())?;
    let backup_name = format!("mise-a-jour-{id}.tar.gz");
    let profile_sha256 = &check.profile_sha256;
    let paper_sha256 = &check.paper_sha256;
    let geyser_sha256 = &check.geyser_sha256;
    let floodgate_sha256 = &check.floodgate_sha256;
    let geyser_config_sha256 = check
        .profile
        .get("geyser_config_sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            PiUpdateFailure::Safe(
                "L’empreinte de configuration Geyser est absente; serveur inchangé.".into(),
            )
        })?;
    let remote_script = format!(
        r#"set -eu
umask 077
d="$HOME/{server_folder}"
parent="$HOME/{parent_folder}"
upload="$HOME/{remote_archive}"
chmod 600 "$upload"
work="$parent/.update-{id}"
backup="$parent/backups/{backup_name}"
mkdir -p "$parent/backups"
test -d "$d" && test -s "$upload"
lock="$parent/.update.lock"
cleanup_lock() {{ rmdir "$lock" 2>/dev/null || true; }}
if ! mkdir "$lock" 2>/dev/null; then echo 'UPDATE_BUSY'; rm -f "$upload"; exit 0; fi
backup_ready=no
transaction_complete=no
on_exit() {{
  rc=$?
  trap - EXIT
  if [ "$backup_ready" = yes ] && [ "$transaction_complete" != yes ]; then rollback_on_error "$rc"; fi
  cleanup_lock
  if [ "$rc" -ne 0 ] && [ "$backup_ready" != yes ]; then rm -rf "$work"; rm -f "$upload" "$backup.preparing" "$backup.preparing.sha256"; echo 'UPDATE_PREP_FAILED'; exit 0; fi
  exit "$rc"
}}
trap on_exit EXIT
pid="$(cat "$d/serveur.pid" 2>/dev/null || true)"
case "$pid" in ''|*[!0-9]*) pid=;; esac
if test -n "$pid" && kill -0 "$pid" 2>/dev/null && test "$(awk '{{print $3}}' "/proc/$pid/stat" 2>/dev/null || true)" != Z && test "$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)" = "$(readlink -f "$d")" && tr -d '\000' < "/proc/$pid/cmdline" 2>/dev/null | grep -q 'paper.jar'; then echo 'UPDATE_SERVER_RUNNING'; rm -f "$upload"; exit 0; fi
if ss -H -lntu 2>/dev/null | awk '{{print $5}}' | grep -qE '[:.](25565|25566|19132|19133)$'; then echo 'UPDATE_PORTS_BUSY'; rm -f "$upload"; exit 0; fi
if find "$d" -type l -print -quit | grep -q .; then echo 'UPDATE_LINK_FOUND'; rm -f "$upload"; exit 0; fi
test "$(sha256sum "$d/monde-ensemble-profile.json" | cut -d' ' -f1)" = "{profile_sha256}" || {{ echo 'UPDATE_SERVER_CHANGED'; rm -f "$upload"; exit 0; }}
test "$(sha256sum "$d/paper.jar" | cut -d' ' -f1)" = "{paper_sha256}" || {{ echo 'UPDATE_SERVER_CHANGED'; rm -f "$upload"; exit 0; }}
test "$(sha256sum "$d/plugins/Geyser-Spigot.jar" | cut -d' ' -f1)" = "{geyser_sha256}" || {{ echo 'UPDATE_SERVER_CHANGED'; rm -f "$upload"; exit 0; }}
test "$(sha256sum "$d/plugins/Floodgate-Spigot.jar" | cut -d' ' -f1)" = "{floodgate_sha256}" || {{ echo 'UPDATE_SERVER_CHANGED'; rm -f "$upload"; exit 0; }}
test "$(sha256sum "$d/plugins/Geyser-Spigot/config.yml" | cut -d' ' -f1)" = "{geyser_config_sha256}" || {{ echo 'UPDATE_SERVER_CHANGED'; rm -f "$upload"; exit 0; }}
used="$(du -sb "$d" | awk '{{print $1}}')"
free="$(df -Pk "$HOME" | awk 'END {{print $4 * 1024}}')"
pkg="$(wc -c < "$upload" | tr -d ' ')"
need=$((2 * used + 2 * pkg + 536870912))
if [ "$free" -lt "$need" ]; then echo 'UPDATE_SPACE_LOW'; rm -f "$upload"; exit 0; fi
mkdir "$work"
tar -xzf "$upload" -C "$work"
(cd "$work/update" && sha256sum -c SHA256SUMS)
test -s "$work/update/paper.jar" && test -s "$work/update/plugins/Geyser-Spigot.jar" && test -s "$work/update/plugins/Floodgate-Spigot.jar"
rm -f "$upload"
if ! tar -czf "$backup.preparing" -C "$parent" server; then rm -f "$backup.preparing"; rm -rf "$work"; echo 'UPDATE_BACKUP_FAILED'; exit 0; fi
if ! backup_sha="$(sha256sum "$backup.preparing" | awk '{{print $1}}')"; then rm -f "$backup.preparing"; echo 'UPDATE_BACKUP_FAILED'; exit 0; fi
if ! printf '%s  %s\n' "$backup_sha" "$(basename "$backup")" > "$backup.preparing.sha256"; then rm -f "$backup.preparing" "$backup.preparing.sha256"; echo 'UPDATE_BACKUP_FAILED'; exit 0; fi
mv "$backup.preparing" "$backup"
mv "$backup.preparing.sha256" "$backup.sha256"
backup_ready=yes
transaction_complete=no
server_process_running() {{
  p="$(cat "$d/serveur.pid" 2>/dev/null || true)"
  case "$p" in ''|*[!0-9]*) return 1;; esac
  kill -0 "$p" 2>/dev/null && test "$(awk '{{print $3}}' "/proc/$p/stat" 2>/dev/null || true)" != Z && test "$(readlink -f "/proc/$p/cwd" 2>/dev/null || true)" = "$(readlink -f "$d")" && tr -d '\000' < "/proc/$p/cmdline" 2>/dev/null | grep -q 'paper.jar'
}}
start_game() {{
  rm -f "$d/serveur.pid"
  cd "$d"
  nohup ./demarrer.sh >> serveur.log 2>&1 < /dev/null &
  echo $! > "$d/serveur.pid"
}}
wait_ready() {{
  start_lines="$1"
  for i in $(seq 1 60); do
    if tail -n +$((start_lines + 1)) "$d/serveur.log" 2>/dev/null | grep -q 'Done (' && ss -H -lnt 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.](25565|25566)$' && ss -H -lnt 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.](19132|19133)$' && ss -H -lnu 2>/dev/null | awk '{{print $4}}' | grep -qE '[:.](19132|19133)$' && grep -Eq '^[[:space:]]*transport:[[:space:]]*nethernet([[:space:]#]|$)' "$d/plugins/Geyser-Spigot/config.yml"; then return 0; fi
    if ! server_process_running; then return 1; fi
    sleep 2
  done
  return 1
}}
rollback_on_error() {{
  rc="$1"
  trap - EXIT
  if [ "$backup_ready" != yes ] || [ "$transaction_complete" = yes ]; then cleanup_lock; exit "$rc"; fi
  if server_process_running; then
    p="$(cat "$d/serveur.pid")"
    kill -TERM "$p" || true
    for i in $(seq 1 45); do if ! server_process_running; then break; fi; sleep 1; done
    if server_process_running; then transaction_complete=yes; echo 'UPDATE_STOP_FAILED'; echo 'UPDATE_BACKUP='"$backup"; cleanup_lock; exit 0; fi
  fi
  tail -n 100 "$d/serveur.log" > "$parent/backups/echec-{id}.log" 2>/dev/null || true
  restore="$parent/.rollback-{id}"
  mkdir -p "$restore"
  if ! (cd "$(dirname "$backup")" && sha256sum -c "$(basename "$backup").sha256"); then transaction_complete=yes; echo 'UPDATE_ROLLBACK_FAILED'; echo 'UPDATE_BACKUP='"$backup"; cleanup_lock; exit 0; fi
  if ! tar -xzf "$backup" -C "$restore"; then transaction_complete=yes; echo 'UPDATE_ROLLBACK_FAILED'; echo 'UPDATE_BACKUP='"$backup"; cleanup_lock; exit 0; fi
  failed="$parent/backups/serveur-echec-{id}"
  if ! mv "$d" "$failed"; then transaction_complete=yes; echo 'UPDATE_ROLLBACK_FAILED'; echo 'UPDATE_BACKUP='"$backup"; cleanup_lock; exit 0; fi
  if ! mv "$restore/server" "$d"; then mv "$failed" "$d" 2>/dev/null || true; transaction_complete=yes; echo 'UPDATE_ROLLBACK_FAILED'; echo 'UPDATE_BACKUP='"$backup"; cleanup_lock; exit 0; fi
  rmdir "$restore" 2>/dev/null || true
  old_lines="$(wc -l < "$d/serveur.log" 2>/dev/null || echo 0)"
  start_game
  if wait_ready "$old_lines"; then transaction_complete=yes; echo 'UPDATE_ROLLED_BACK'; echo 'UPDATE_BACKUP='"$backup"; cleanup_lock; exit 0; fi
  transaction_complete=yes
  echo 'UPDATE_ROLLBACK_SERVER_NOT_READY'
  echo 'UPDATE_BACKUP='"$backup"
  cleanup_lock
  exit 0
}}
cp "$work/update/paper.jar" "$d/paper.jar.new"
cp "$work/update/plugins/Geyser-Spigot.jar" "$d/plugins/Geyser-Spigot.jar.new"
cp "$work/update/plugins/Floodgate-Spigot.jar" "$d/plugins/Floodgate-Spigot.jar.new"
cp "$work/update/plugins/Geyser-Spigot/config.yml" "$d/plugins/Geyser-Spigot/config.yml.new"
cp "$work/update/monde-ensemble-profile.json" "$d/monde-ensemble-profile.json.new"
mv -f "$d/paper.jar.new" "$d/paper.jar"
mv -f "$d/plugins/Geyser-Spigot.jar.new" "$d/plugins/Geyser-Spigot.jar"
mv -f "$d/plugins/Floodgate-Spigot.jar.new" "$d/plugins/Floodgate-Spigot.jar"
mv -f "$d/plugins/Geyser-Spigot/config.yml.new" "$d/plugins/Geyser-Spigot/config.yml"
mv -f "$d/monde-ensemble-profile.json.new" "$d/monde-ensemble-profile.json"
chmod 600 "$d/server.properties" "$d/monde-ensemble-profile.json"
old_lines="$(wc -l < "$d/serveur.log" 2>/dev/null || echo 0)"
start_game
if ! wait_ready "$old_lines"; then exit 1; fi
rm -rf "$work"
transaction_complete=yes
echo 'UPDATE_OK'
echo 'UPDATE_BACKUP='"$backup"
exit 0"#
    );
    progress(
        "Sauvegarde complète du serveur puis remplacement coordonné…",
        78,
    );
    let output = run_ssh_timeout(&destination, &remote_script, 240).map_err(|error| {
        PiUpdateFailure::Uncertain(format!(
            "La connexion a été interrompue pendant la transaction de mise à jour. Le serveur n’a pas été relancé automatiquement; vérifie son état avant toute nouvelle action. Détail : {error}"
        ))
    })?;
    let marker = output
        .lines()
        .find(|line| line.starts_with("UPDATE_"))
        .unwrap_or_default();
    if marker == "UPDATE_OK" {
        let backup = output
            .lines()
            .find_map(|line| line.strip_prefix("UPDATE_BACKUP="))
            .unwrap_or("sauvegarde automatique");
        progress("Mise à jour appliquée et démarrage contrôlé.", 100);
        return Ok(Some(format!(
            "Serveur mis à jour · démarrage Java et Bedrock confirmé · sauvegarde conservée : {backup}"
        )));
    }
    let message = match marker {
        "UPDATE_SERVER_RUNNING" => "Le serveur est encore actif; il n’a pas été arrêté ni modifié. Arrête-le proprement, puis relance le démarrage.".to_string(),
        "UPDATE_PORTS_BUSY" => "Un autre service utilise un port Minecraft; la mise à jour est reportée et le serveur n’a pas été modifié.".to_string(),
        "UPDATE_SERVER_CHANGED" => "Les fichiers du serveur ont changé depuis le contrôle. La mise à jour est annulée; relance le démarrage pour refaire l’inspection.".to_string(),
        "UPDATE_BUSY" => "Une autre opération est déjà en cours sur ce serveur. Attends qu’elle se termine avant de relancer.".to_string(),
        "UPDATE_PREP_FAILED" => "La préparation de la mise à jour a échoué avant la sauvegarde; le serveur existant reste intact.".to_string(),
        "UPDATE_LINK_FOUND" => "Un lien symbolique est apparu; la mise à jour est annulée sans modifier le serveur.".to_string(),
        "UPDATE_SPACE_LOW" => "Le Pi n’a pas assez d’espace pour une sauvegarde complète, la mise à jour et un retour arrière sûr; le serveur reste inchangé.".to_string(),
        "UPDATE_BACKUP_FAILED" => "La sauvegarde complète n’a pas pu être créée; le serveur reste inchangé.".to_string(),
        "UPDATE_ROLLED_BACK" => format!("Le nouveau serveur n’a pas passé le contrôle de démarrage. L’ancienne version a été restaurée et relancée. Sauvegarde conservée : {}", output.lines().find_map(|line| line.strip_prefix("UPDATE_BACKUP=")).unwrap_or("sur le Pi")),
        "UPDATE_ROLLBACK_SERVER_NOT_READY" => format!("La mise à jour a échoué et les fichiers précédents ont été restaurés, mais l’ancien serveur ne confirme pas son démarrage. La sauvegarde est conservée : {}", output.lines().find_map(|line| line.strip_prefix("UPDATE_BACKUP=")).unwrap_or("sur le Pi")),
        "UPDATE_ROLLBACK_FAILED" => return Err(PiUpdateFailure::Uncertain(format!("La mise à jour et le retour arrière n’ont pas abouti automatiquement. Ne relance pas le serveur; la sauvegarde est conservée à {}", output.lines().find_map(|line| line.strip_prefix("UPDATE_BACKUP=")).unwrap_or("l’emplacement de sauvegarde sur le Pi")))),
        "UPDATE_STOP_FAILED" => return Err(PiUpdateFailure::Uncertain(format!("Le serveur mis à jour ne s’est pas arrêté proprement; il n’a pas été forcé à quitter et le retour arrière n’a pas été exécuté. La sauvegarde complète est conservée à {}", output.lines().find_map(|line| line.strip_prefix("UPDATE_BACKUP=")).unwrap_or("sur le Pi")))),
        _ => return Err(PiUpdateFailure::Uncertain(format!("Le contrôle de mise à jour a renvoyé une réponse inconnue; le serveur n’a pas été démarré automatiquement. Détail : {}", output.chars().take(1000).collect::<String>()))),
    };
    Ok(Some(message))
}

/// Restarts only this app's saved installation, and only while both game
/// ports are free. The existing log is kept so the user can inspect history.
pub fn start_pi_server(
    host: &str,
    username: &str,
    server_folder: &str,
    progress: impl Fn(&str, u8),
) -> Result<String, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Le dossier enregistré du serveur n’est pas reconnu.".into());
    }
    let mut update_note = None;
    progress("Vérification des versions Java et Bedrock…", 5);
    match check_pi_update(host, username, server_folder) {
        Ok(check) if check.ready && check.update_available => {
            match update_pi_server_if_ready(host, username, server_folder, &check, &progress) {
                Ok(note) => update_note = note,
                Err(PiUpdateFailure::Safe(error)) => {
                    update_note = Some(format!(
                        "La mise à jour a été différée; le serveur existant reste intact. {error}"
                    ));
                }
                Err(PiUpdateFailure::Uncertain(error)) => return Err(error),
            }
        }
        Ok(check) if check.update_available => {
            update_note = Some(format!(
                "Mise à jour différée pour protéger le monde : {}",
                check.reasons.join(" ")
            ));
        }
        Ok(_) => {}
        Err(error) => {
            update_note = Some(format!(
                "Catalogue de versions momentanément indisponible; le serveur existant reste sur sa version actuelle. {error}"
            ));
        }
    }
    let destination = format!("{username}@{host}");
    let command = format!(
        "set -eu; d=\"$HOME/{server_folder}\"; test -x \"$d/demarrer.sh\" || {{ echo 'LANCEUR_ABSENT'; exit 0; }}; pid=; if test -r \"$d/serveur.pid\"; then pid=\"$(cat \"$d/serveur.pid\")\"; fi; case \"$pid\" in ''|*[!0-9]*) pid=;; esac; if test -n \"$pid\" && kill -0 \"$pid\" 2>/dev/null && test \"$(awk '{{print $3}}' \"/proc/$pid/stat\" 2>/dev/null || true)\" != Z && test \"$(readlink -f \"/proc/$pid/cwd\" 2>/dev/null || true)\" = \"$(readlink -f \"$d\")\" && tr -d '\\000' < \"/proc/$pid/cmdline\" 2>/dev/null | grep -q 'paper.jar'; then echo 'DEJA_LANCE'; exit 0; fi; rm -f \"$d/serveur.pid\"; if ss -H -lntu 2>/dev/null | awk '{{print $5}}' | grep -qE '[:.](25565|25566|19132|19133)$'; then echo 'PORT_OCCUPE'; exit 0; fi; cd \"$d\"; nohup ./demarrer.sh >> serveur.log 2>&1 < /dev/null & echo $! > serveur.pid; echo 'DEMARRAGE_LANCE'"
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
    let started_here = output.lines().any(|line| line == "DEMARRAGE_LANCE");
    if started_here {
        progress(
            "Démarrage de Minecraft et contrôle des connexions Java/Bedrock…",
            92,
        );
        for attempt in 0..40 {
            match inspect_pi_server(host, username, server_folder) {
                Ok(status)
                    if status.ready
                        && status.java_port_open
                        && status.bedrock_tcp_port_open
                        && status.bedrock_port_open =>
                {
                    let autostart_configured =
                        configure_pi_autostart(host, username, server_folder).unwrap_or(false);
                    progress("Serveur prêt et ports vérifiés.", 100);
                    let started = update_note.unwrap_or_else(|| {
                        format!(
                            "Serveur démarré et prêt · Java {} · Bedrock compatible {}.",
                            status.minecraft_java, status.minecraft_bedrock
                        )
                    });
                    return Ok(format!(
                        "{started} · Démarrage après redémarrage {}.",
                        if autostart_configured {
                            "configuré pour cette copie"
                        } else {
                            "non configuré sur ce Pi"
                        }
                    ));
                }
                Ok(status) if !status.running => {
                    return Err(format!(
                        "Le serveur s’est arrêté pendant le démarrage. Rien n’a été supprimé. Dernières lignes du journal :\n{}",
                        status.log
                    ));
                }
                Ok(_) => {}
                Err(error) if attempt == 39 => {
                    return Err(format!(
                        "Le serveur n’a pas confirmé ses ports après 80 secondes. Consulte le journal dans l’application. {error}"
                    ));
                }
                Err(_) => {}
            }
            progress(
                &format!("Minecraft démarre… vérification {}/40", attempt + 1),
                92 + (attempt as u8 / 5),
            );
            std::thread::sleep(Duration::from_secs(2));
        }
        return Err("Le serveur n’a pas confirmé ses ports Java et Bedrock dans le délai prévu. Consulte le journal avant de rejouer.".into());
    }
    Ok(update_note
        .unwrap_or_else(|| "Serveur démarré sur les versions compatibles déjà installées.".into()))
}

/// Gracefully stops only the Paper process launched from the saved server
/// directory. It never force-kills a process that does not exit cleanly.
pub fn stop_pi_server(host: &str, username: &str, server_folder: &str) -> Result<String, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Le dossier enregistré du serveur n’est pas reconnu.".into());
    }
    let destination = format!("{username}@{host}");
    let command = format!(
        "set -eu; d=\"$HOME/{server_folder}\"; test -d \"$d\" || {{ echo STOP_NO_FOLDER; exit 0; }}; pid=\"$(cat \"$d/serveur.pid\" 2>/dev/null || true)\"; case \"$pid\" in ''|*[!0-9]*) rm -f \"$d/serveur.pid\"; echo STOP_ALREADY; exit 0;; esac; if ! kill -0 \"$pid\" 2>/dev/null || test \"$(awk '{{print $3}}' \"/proc/$pid/stat\" 2>/dev/null || true)\" = Z || test \"$(readlink -f \"/proc/$pid/cwd\" 2>/dev/null || true)\" != \"$(readlink -f \"$d\")\" || ! tr -d '\\000' < \"/proc/$pid/cmdline\" 2>/dev/null | grep -q 'paper.jar'; then rm -f \"$d/serveur.pid\"; echo STOP_ALREADY; exit 0; fi; kill -TERM \"$pid\"; for i in $(seq 1 60); do if ! kill -0 \"$pid\" 2>/dev/null || test \"$(awk '{{print $3}}' \"/proc/$pid/stat\" 2>/dev/null || true)\" = Z || test \"$(readlink -f \"/proc/$pid/cwd\" 2>/dev/null || true)\" != \"$(readlink -f \"$d\")\"; then rm -f \"$d/serveur.pid\"; echo STOPPED; exit 0; fi; sleep 1; done; echo STOP_TIMEOUT; exit 0"
    );
    let output = run_ssh_timeout(&destination, &command, 75)?;
    match output.lines().find(|line| line.starts_with("STOP_")) {
        Some("STOPPED") => Ok("Serveur arrêté proprement. Le monde a été enregistré.".into()),
        Some("STOP_ALREADY") => Ok("Le serveur est déjà arrêté.".into()),
        Some("STOP_TIMEOUT") => Err("Le serveur ne s’est pas arrêté dans le délai prévu. Il n’a pas été forcé à quitter; consulte le journal avant de poursuivre.".into()),
        _ => Err("Le Pi n’a pas confirmé l’arrêt du serveur.".into()),
    }
}

fn valid_server_folder(value: &str) -> bool {
    Regex::new(r"^Monde-Ensemble-[a-zA-Z0-9-]+/server$")
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
    network_interface: String,
    mac_address: String,
    java_default_busy: bool,
    java_alternate_busy: bool,
    bedrock_default_busy: bool,
    bedrock_alternate_busy: bool,
    rcon_default_busy: bool,
    rcon_alternate_busy: bool,
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
        network_interface: values
            .get("NETWORK_INTERFACE")
            .copied()
            .unwrap_or_default()
            .trim()
            .to_string(),
        mac_address: values
            .get("MAC_ADDRESS")
            .copied()
            .unwrap_or_default()
            .trim()
            .to_string(),
        java_default_busy: required("PORT_25565")?.trim() == "busy",
        java_alternate_busy: required("PORT_25566")?.trim() == "busy",
        bedrock_default_busy: required("PORT_19132")?.trim() == "busy",
        bedrock_alternate_busy: required("PORT_19133")?.trim() == "busy",
        rcon_default_busy: required("PORT_25575")?.trim() == "busy",
        rcon_alternate_busy: required("PORT_25576")?.trim() == "busy",
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

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
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
    let output = ssh_command()
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
        return Err(ssh_failure_message(output.status.code(), &detail));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn ssh_failure_message(exit_code: Option<i32>, detail: &str) -> String {
    if detail.is_empty() {
        return format!(
            "La connexion SSH ou la commande distante a échoué sans fournir de détail (code {}). Le Pi n’a pas confirmé cette étape.",
            exit_code.map_or_else(|| "inconnu".into(), |code| code.to_string())
        );
    }
    format!(
        "Connexion SSH impossible : {}",
        detail.chars().take(6000).collect::<String>()
    )
}

fn build_fresh_server(
    stage: &Path,
    heap_mb: u16,
    remote_linux: bool,
    java_port: u16,
    bedrock_port: u16,
    rcon_port: Option<u16>,
    progress: &impl Fn(&str, u8),
) -> Result<(String, String, String, String, u16, u16), String> {
    let client = client()?;
    progress("Lecture des versions compatibles auprès de GeyserMC…", 3);
    let supported = supported_versions(&client)?;
    let paper = paper_build(&client, &supported.java)?;
    let geyser = bridge_build(&client, "geyser", "spigot")?;
    let floodgate = bridge_build(&client, "floodgate", "spigot")?;
    if geyser.build < MIN_GEYSER_NETHERNET_BUILD {
        return Err(format!(
            "Geyser build {} ne prend pas en charge NetherNet. Aucun serveur n’a été préparé; le catalogue doit publier un build compatible.",
            geyser.build
        ));
    }
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
            // Linux is case-sensitive even when the build host's macOS
            // filesystem is not; keep canonical names in the package.
            name: GEYSER_PLUGIN_JAR.into(),
            version: geyser.version.clone(),
            url: format!(
                "https://download.geysermc.org/v2/projects/geyser/versions/latest/builds/latest/downloads/spigot"
            ),
            checksum: geyser_download.sha256.clone(),
        },
        Artifact {
            name: FLOODGATE_PLUGIN_JAR.into(),
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
    let runtime = runtime_package(java_major, remote_linux, progress, 66)?;
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
        heap_mb,
        remote_linux,
        java_port,
        bedrock_port,
        rcon_port,
        &geyser,
        &floodgate,
        paper.id,
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

/// Paper's published matrix requires Java 25 for Minecraft 26.1 and later in
/// the 26.x release line. Unknown versions are rejected rather than guessed;
/// update this gate only after checking Paper's current requirements.
fn java_major_for(game_version: &str) -> Result<u8, String> {
    let mut parts = game_version.split('.');
    let major = parts.next().and_then(|part| part.parse::<u16>().ok());
    let minor = parts.next().and_then(|part| part.parse::<u16>().ok());
    let has_extra_parts = parts.next().is_some();

    if major == Some(26) && minor.is_some_and(|minor| minor >= 1) && !has_extra_parts {
        Ok(25)
    } else {
        Err(format!(
            "La version Minecraft {game_version} ne correspond à aucune règle Java validée par l’application. Installation bloquée."
        ))
    }
}

fn runtime_package(
    major: u8,
    remote_linux: bool,
    progress: &impl Fn(&str, u8),
    progress_percent: u8,
) -> Result<RuntimePackage, String> {
    let (os, arch) = runtime_target(remote_linux)?;
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jre&os={os}&vendor=eclipse"
    );
    let mut last_error = String::new();
    for attempt in 1..=2 {
        progress(
            &format!("Lecture du catalogue Java… tentative {attempt}/2"),
            progress_percent,
        );
        let runtime_client = Client::builder()
            .user_agent(concat!("MondeEnsemble/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .http1_only()
            .build()
            .map_err(|error| error.to_string())?;
        let assets = runtime_client
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .header(reqwest::header::CONNECTION, "close")
            .send()
            .map_err(network_error)
            .and_then(|response| response.error_for_status().map_err(network_error))
            .and_then(|response| response.bytes().map_err(network_error))
            .and_then(|body| {
                runtime_package_from_json(&body, major)
                    .map_err(|error| format!("Réponse catalogue Java illisible : {error}"))
            });
        match assets {
            Ok(package) => return Ok(package),
            Err(error) => {
                last_error = error;
                if attempt < 2 {
                    std::thread::sleep(Duration::from_secs(attempt as u64));
                }
            }
        }
    }
    progress("Lecture de secours du catalogue Java…", progress_percent);
    match runtime_package_with_curl(&url, major) {
        Ok(package) => Ok(package),
        Err(curl_error) => Err(format!(
            "Impossible de lire le catalogue Java avec les deux méthodes. Lecture HTTP : {last_error}. Lecture de secours : {curl_error}. Vérifie ta connexion puis relance; aucun fichier Java incomplet n’a été installé et le serveur déjà présent reste intact."
        )),
    }
}

fn runtime_package_from_json(body: &[u8], major: u8) -> Result<RuntimePackage, String> {
    let assets: Vec<RuntimeAsset> = serde_json::from_slice(body)
        .map_err(|error| format!("JSON du catalogue invalide : {error}"))?;
    assets
        .into_iter()
        .map(|asset| asset.binary.package)
        .find(|package| valid_sha256(&package.checksum) && package.link.starts_with("https://"))
        .ok_or_else(|| {
            format!("Aucun Java {major} officiel vérifiable ne correspond à cet appareil.")
        })
}

fn runtime_package_with_curl(url: &str, major: u8) -> Result<RuntimePackage, String> {
    let executable = if cfg!(target_os = "windows") {
        "curl.exe"
    } else {
        "curl"
    };
    let output = Command::new(executable)
        .args([
            "--http1.1",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-redirs",
            "5",
            "--max-time",
            "30",
            "--connect-timeout",
            "10",
            "--max-filesize",
            "2097152",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--header",
            "Accept: application/json",
            "--header",
            "Accept-Encoding: identity",
        ])
        .arg("--user-agent")
        .arg(concat!("MondeEnsemble/", env!("CARGO_PKG_VERSION")))
        .arg(url)
        .output()
        .map_err(|error| format!("curl est indisponible : {error}"))?;
    if !output.status.success() {
        let detail: String = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(300)
            .collect();
        return Err(format!("curl n’a pas pu lire le catalogue : {detail}"));
    }
    runtime_package_from_json(&output.stdout, major)
        .map_err(|error| format!("curl a reçu une réponse inutilisable : {error}"))
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
    heap_mb: u16,
    remote_linux: bool,
    java_port: u16,
    bedrock_port: u16,
    rcon_port: Option<u16>,
    geyser: &BridgeBuild,
    floodgate: &BridgeBuild,
    paper_build_id: u64,
) -> Result<(), String> {
    let eula = format!("# EULA acceptée séparément dans l’application Monde Ensemble\neula=true\n");
    fs::write(stage.join("eula.txt"), eula).map_err(io_error)?;
    let (rcon_properties, rcon_profile) = {
        let rcon_port = match rcon_port {
            Some(port) => port,
            None => available_game_port(25575, 25576, false)?,
        };
        let rcon_password = Uuid::new_v4().simple().to_string();
        (
            format!(
                "enable-rcon=true\nrcon.port={rcon_port}\nrcon.password={rcon_password}\nrcon.ip=127.0.0.1\n"
            ),
            serde_json::Map::from_iter([
                ("rcon_port".into(), serde_json::json!(rcon_port)),
                ("rcon_password".into(), serde_json::json!(rcon_password)),
            ]),
        )
    };
    let compact_pi_profile = remote_linux && heap_mb <= COMPACT_PI_HEAP_MB;
    let (max_players, view_distance, simulation_distance) = if compact_pi_profile {
        ("3", "4", "3")
    } else {
        ("20", "8", "6")
    };
    let properties = format!(
        "motd=Monde Ensemble\nserver-port={java_port}\nserver-ip=\nonline-mode=true\nenforce-secure-profile=false\nmax-players={max_players}\nview-distance={view_distance}\nsimulation-distance={simulation_distance}\n{rcon_properties}"
    );
    fs::write(stage.join("server.properties"), properties).map_err(io_error)?;
    set_private_file_permissions(&stage.join("server.properties"))?;

    let geyser_jar = stage.join("plugins").join("Geyser-Spigot.jar");
    let mut jar =
        zip::ZipArchive::new(File::open(geyser_jar).map_err(io_error)?).map_err(|error| {
            format!("Le fichier Geyser téléchargé n’est pas une archive valide : {error}")
        })?;
    let template = geyser_config_from_archive(&mut jar)?;
    let template = managed_geyser_config(&template, bedrock_port);
    fs::create_dir_all(stage.join("plugins/Geyser-Spigot")).map_err(io_error)?;
    fs::write(stage.join("plugins/Geyser-Spigot/config.yml"), template).map_err(io_error)?;
    let geyser_config_sha256 = hash_file(&stage.join("plugins/Geyser-Spigot/config.yml"))?;

    write_launch_files(stage, heap_mb, remote_linux, java_port, bedrock_port)?;
    let mut profile = serde_json::json!({
        "application": "Monde Ensemble",
        "minecraft_java": java_version,
        "minecraft_bedrock": bedrock_versions,
        "paper_build_id": paper_build_id,
        "geyser": geyser.version,
        "geyser_build_id": geyser.build,
        "floodgate": floodgate.version,
        "floodgate_build_id": floodgate.build,
        "geyser_config_sha256": geyser_config_sha256,
        "java_runtime": java_major,
        "java_heap_gb": heap_mb.div_ceil(1024),
        "java_heap_mb": heap_mb,
        "resource_profile": if compact_pi_profile { "compact-pi" } else { "standard" },
        "java_port": java_port,
        "bedrock_port": bedrock_port,
        "bedrock_transport": "nethernet",
        "plugins": ["Geyser-Spigot", "Floodgate-Spigot"],
        "created_at": Utc::now().to_rfc3339(),
    });
    record_managed_artifact_hashes(&mut profile, stage)?;
    if let Some(object) = profile.as_object_mut() {
        object.extend(rcon_profile);
    }
    fs::write(
        stage.join("monde-ensemble-profile.json"),
        serde_json::to_vec_pretty(&profile).map_err(|error| error.to_string())?,
    )
    .map_err(io_error)?;
    set_private_file_permissions(&stage.join("monde-ensemble-profile.json"))?;
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(io_error)
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<(), String> {
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

fn yaml_section_value(source: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indented = line.starts_with([' ', '\t']);
        if !indented {
            in_section = trimmed
                .split_once(':')
                .is_some_and(|(name, _)| name == section);
            continue;
        }
        if in_section {
            let Some((name, value)) = trimmed.split_once(':') else {
                continue;
            };
            if name == key {
                let value = value.split('#').next().unwrap_or(value).trim();
                return Some(value.trim_matches(['\'', '"']).to_string());
            }
        }
    }
    None
}

fn managed_geyser_config(template: &str, bedrock_port: u16) -> String {
    let configured = set_yaml(template, "bedrock", "port", &bedrock_port.to_string());
    let configured = set_yaml(&configured, "bedrock", "transport", "nethernet");
    set_yaml(&configured, "remote", "auth-type", "floodgate")
}

fn geyser_nethernet_ready(server_dir: &Path) -> bool {
    fs::read_to_string(server_dir.join("plugins/Geyser-Spigot/config.yml"))
        .ok()
        .and_then(|config| yaml_section_value(&config, "bedrock", "transport"))
        .is_some_and(|transport| transport.eq_ignore_ascii_case("nethernet"))
}

fn write_launch_files(
    stage: &Path,
    heap_mb: u16,
    remote_linux: bool,
    java_port: u16,
    bedrock_port: u16,
) -> Result<(), String> {
    if cfg!(windows) && !remote_linux {
        let script = format!(
            "@echo off\r\ncd /d \"%~dp0\"\r\nfor /r runtime %%J in (java.exe) do set JAVA_BIN=%%J\r\nif not defined JAVA_BIN echo Java inclus introuvable & exit /b 1\r\n\"%JAVA_BIN%\" {} -Xmx{heap_mb}M -jar paper.jar --nogui\r\npause\r\n",
            initial_heap_argument(heap_mb)
        );
        fs::write(stage.join("demarrer.bat"), script).map_err(io_error)?;
    } else {
        let script = format!(
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nJAVA_BIN=$(find runtime -type f -name java -print -quit)\n[ -n \"$JAVA_BIN\" ] || {{ echo 'Java inclus introuvable.'; exit 1; }}\nexec \"$JAVA_BIN\" {} -Xmx{heap_mb}M -jar paper.jar --nogui\n",
            initial_heap_argument(heap_mb)
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
    ensure_backup_ports_closed(&source)?;
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
    ensure_backup_ports_closed(&source)?;
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

/// A user checkbox cannot prove that a manually launched Minecraft process
/// has stopped. Check the configured game ports immediately before copying.
fn ensure_backup_ports_closed(server_dir: &Path) -> Result<(), String> {
    let profile_path = server_dir.join("monde-ensemble-profile.json");
    let profile = match fs::symlink_metadata(&profile_path) {
        Ok(metadata) if metadata.file_type().is_file() => fs::read(&profile_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("Le profil du serveur est un lien symbolique; sauvegarde bloquée.".into());
        }
        _ => None,
    };
    let profile_port = |key: &str| {
        profile
            .as_ref()?
            .get(key)?
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .filter(|port| *port > 0)
    };
    let properties_path = server_dir.join("server.properties");
    let properties = match fs::symlink_metadata(&properties_path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            fs::read_to_string(&properties_path).map_err(io_error)?
        }
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(
                "Les réglages du serveur sont un lien symbolique; sauvegarde bloquée.".into(),
            );
        }
        _ => String::new(),
    };
    let property_port = |key: &str| {
        properties
            .lines()
            .find_map(|line| line.trim().strip_prefix(&format!("{key}=")))
            .and_then(|value| value.trim().parse::<u16>().ok())
            .filter(|port| *port > 0)
    };
    let java_port = profile_port("java_port")
        .or_else(|| property_port("server-port"))
        .unwrap_or(JAVA_DEFAULT_PORT);
    let bedrock_port = profile_port("bedrock_port");
    let java_busy = !tcp_port_available(java_port);
    let bedrock_busy =
        bedrock_port.is_some_and(|port| !tcp_port_available(port) || !udp_port_available(port));
    if java_busy || bedrock_busy {
        return Err("Un port Minecraft est encore actif. Arrête le serveur puis relance la sauvegarde; aucun fichier n’a été copié.".into());
    }
    Ok(())
}

fn pi_heap_megabytes(memory_gb: f64) -> u16 {
    if memory_gb < COMPACT_PI_MEMORY_GB {
        COMPACT_PI_HEAP_MB
    } else {
        ((memory_gb * 0.4).floor() as u16).clamp(1, 6) * 1024
    }
}

fn initial_heap_argument(heap_mb: u16) -> String {
    if heap_mb <= COMPACT_PI_HEAP_MB {
        "-Xms128M".into()
    } else {
        "-Xms1G".into()
    }
}

fn profile_heap_megabytes(profile: &serde_json::Value) -> Option<u16> {
    profile
        .get("java_heap_mb")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .or_else(|| {
            profile
                .get("java_heap_gb")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .filter(|value| (1..=6).contains(value))
                .map(|value| value * 1024)
        })
}

fn format_heap(heap_mb: u16) -> String {
    if heap_mb % 1024 == 0 {
        format!("{} Go", heap_mb / 1024)
    } else {
        format!("{heap_mb} Mo")
    }
}

pub fn create_pi_backup(
    host: &str,
    username: &str,
    server_folder: &str,
    output_file: &Path,
    progress: impl Fn(&str, u8),
) -> Result<OperationResult, String> {
    if !valid_remote_name(host)
        || !valid_remote_name(username)
        || !valid_server_folder(server_folder)
    {
        return Err("Le serveur Pi enregistré n’est pas reconnu.".into());
    }
    let output_parent = output_file
        .parent()
        .ok_or_else(|| "Choisis l’emplacement de la sauvegarde.".to_string())?;
    if !output_parent.is_dir() {
        return Err("Le dossier choisi pour la sauvegarde est introuvable.".into());
    }
    let manifest_path = PathBuf::from(format!("{}.sha256", output_file.display()));
    if output_file.exists() || manifest_path.exists() {
        return Err("Ce nom de sauvegarde existe déjà. Choisis un autre nom pour conserver les deux copies.".into());
    }
    let destination = format!("{username}@{host}");
    let id = Uuid::new_v4().simple().to_string();
    let remote_archive = format!(".Monde-Ensemble-Sauvegarde-{id}.tar.gz");
    progress(
        "Vérification de l’arrêt du serveur et de l’espace disponible…",
        5,
    );
    let parent_folder = server_folder
        .strip_suffix("/server")
        .ok_or_else(|| "Dossier Pi invalide.".to_string())?;
    let inspect = format!(
        r#"set -eu
d="$HOME/{server_folder}"
test -d "$d" && test ! -L "$d"
test -f "$d/monde-ensemble-profile.json" && test ! -L "$d/monde-ensemble-profile.json"
grep -Fq '"application": "Monde Ensemble"' "$d/monde-ensemble-profile.json"
if find "$d" -type l -print -quit | grep -q .; then echo SYMLINK_FOUND; exit 6; fi
pid="$(cat "$d/serveur.pid" 2>/dev/null || true)"
case "$pid" in ''|*[!0-9]*) pid=;; esac
if test -n "$pid" && kill -0 "$pid" 2>/dev/null && test "$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)" = "$(readlink -f "$d")" && tr -d '\000' < "/proc/$pid/cmdline" 2>/dev/null | grep -q 'paper.jar'; then echo SERVER_RUNNING; exit 7; fi
if ss -H -lntu 2>/dev/null | awk '{{print $5}}' | grep -qE '[:.](25565|25566|19132|19133)$'; then echo SERVER_RUNNING; exit 7; fi
printf 'TOTAL='; du -sb "$d" | awk '{{print $1}}'
printf 'FREE='; df -B1 --output=avail "$HOME" | tail -1 | tr -dc '0-9'
"#
    );
    let inspect = run_ssh(&destination, &inspect).map_err(|error| {
        if error.contains("SERVER_RUNNING") { "Arrête le serveur depuis le tableau de bord avant la sauvegarde. Rien n’a été copié.".to_string() }
        else if error.contains("SYMLINK_FOUND") { "Le dossier Pi contient un lien symbolique. La sauvegarde est bloquée pour éviter de copier des fichiers inattendus.".to_string() }
        else { error }
    })?;
    let values = inspect
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect::<std::collections::HashMap<_, _>>();
    let total = values
        .get("TOTAL")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .ok_or_else(|| "Taille du serveur Pi illisible.".to_string())?;
    let remote_free = values
        .get("FREE")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .ok_or_else(|| "Espace libre du Pi illisible.".to_string())?;
    if remote_free < total.saturating_add(64 * 1024 * 1024) {
        return Err("Le Pi manque d’espace pour préparer une archive complète temporaire; aucun fichier du monde n’a été modifié.".into());
    }
    let local_free = fs2::available_space(output_parent).map_err(io_error)?;
    if local_free < total.saturating_add(64 * 1024 * 1024) {
        return Err(
            "Le disque de sauvegarde manque d’espace pour recevoir le monde complet.".into(),
        );
    }

    progress("Création de l’archive complète sur le Pi…", 20);
    let create = format!(
        r#"set -eu
umask 077
d="$HOME/{server_folder}"
a="$HOME/{remote_archive}"
test ! -e "$a"
pid="$(cat "$d/serveur.pid" 2>/dev/null || true)"
case "$pid" in ''|*[!0-9]*) pid=;; esac
if test -n "$pid" && kill -0 "$pid" 2>/dev/null && test "$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)" = "$(readlink -f "$d")" && tr -d '\000' < "/proc/$pid/cmdline" 2>/dev/null | grep -q 'paper.jar'; then echo SERVER_RUNNING; exit 7; fi
if ss -H -lntu 2>/dev/null | awk '{{print $5}}' | grep -qE '[:.](25565|25566|19132|19133)$'; then echo SERVER_RUNNING; exit 7; fi
tar -czf "$a" -C "$HOME/{parent_folder}" server
tar -tzf "$a" >/dev/null
pid="$(cat "$d/serveur.pid" 2>/dev/null || true)"
case "$pid" in ''|*[!0-9]*) pid=;; esac
if test -n "$pid" && kill -0 "$pid" 2>/dev/null && test "$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)" = "$(readlink -f "$d")" && tr -d '\000' < "/proc/$pid/cmdline" 2>/dev/null | grep -q 'paper.jar'; then echo SERVER_RUNNING; exit 7; fi
if ss -H -lntu 2>/dev/null | awk '{{print $5}}' | grep -qE '[:.](25565|25566|19132|19133)$'; then echo SERVER_RUNNING; exit 7; fi
sha256sum "$a" | awk '{{print $1}}'
"#
    );
    let remote_hash = run_ssh_timeout(&destination, &create, 240)
        .map_err(|error| {
            let cleanup = format!("rm -f -- \"$HOME/{remote_archive}\"");
            let _ = run_ssh(&destination, &cleanup);
            if error.contains("SERVER_RUNNING") {
                "Un port Minecraft est occupé ou le serveur a redémarré. Arrête-le puis relance la sauvegarde; aucun fichier du monde n’a été modifié.".to_string()
            } else {
                format!(
                    "La sauvegarde Pi n’a pas pu être préparée; le monde n’a pas été modifié. {error}"
                )
            }
        })?
        .trim()
        .to_string();
    if remote_hash.len() != 64
        || !remote_hash
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        let _ = run_ssh(
            &destination,
            &format!("rm -f -- \"$HOME/{remote_archive}\""),
        );
        return Err("Le Pi n’a pas fourni une empreinte de sauvegarde valide.".into());
    }
    progress("Transfert chiffré de l’archive vers cet ordinateur…", 55);
    let staging = output_file.with_extension(format!("{id}.preparing.tar.gz"));
    let _cleanup = RemoveFileOnDrop(staging.clone());
    let staging_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .map_err(io_error)?;
    drop(staging_file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o600)).map_err(io_error)?;
    }
    let mut child = scp_command()
        .args([
            "-p",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "PasswordAuthentication=no",
            &format!("{destination}:{remote_archive}"),
            staging.to_string_lossy().as_ref(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            let _ = run_ssh(
                &destination,
                &format!("rm -f -- \"$HOME/{remote_archive}\""),
            );
            "L’outil de transfert sécurisé n’est pas disponible sur cet ordinateur.".to_string()
        })?;
    let deadline = std::time::Instant::now() + Duration::from_secs(1800);
    let transfer_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = run_ssh(
                    &destination,
                    &format!("rm -f -- \"$HOME/{remote_archive}\""),
                );
                return Err(io_error(error));
            }
            Ok(None) if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(250)),
        }
    };
    let cleanup_result = run_ssh(
        &destination,
        &format!("rm -f -- \"$HOME/{remote_archive}\""),
    );
    if transfer_status.is_none() {
        return Err("Le transfert dépasse 30 minutes. Le fichier incomplet est retiré; vérifie le Pi avant de réessayer.".into());
    }
    if !transfer_status.is_some_and(|status| status.success()) {
        return Err("Le transfert sécurisé de la sauvegarde a échoué. Le fichier incomplet local sera retiré.".into());
    }
    if cleanup_result.is_err() {
        return Err(format!(
            "L’archive a été transférée, mais le Pi n’a pas confirmé le retrait de sa copie temporaire ({remote_archive}). Vérifie l’espace du Pi avant de réessayer."
        ));
    }
    progress(
        "Vérification de l’intégrité et publication de la sauvegarde…",
        88,
    );
    let checksum = hash_file(&staging)?;
    if !checksum.eq_ignore_ascii_case(&remote_hash) {
        return Err("L’empreinte de l’archive transférée ne correspond pas à celle du Pi. Sauvegarde rejetée.".into());
    }
    fs::hard_link(&staging, output_file).map_err(|error| {
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
    let mut manifest_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest_path)
        .map_err(|error| {
            let _ = fs::remove_file(output_file);
            format!("Impossible de créer le contrôle d’intégrité : {error}")
        })?;
    manifest_file
        .write_all(manifest.as_bytes())
        .map_err(|error| {
            let _ = fs::remove_file(&manifest_path);
            let _ = fs::remove_file(output_file);
            error.to_string()
        })?;
    progress("Sauvegarde Pi vérifiée et terminée.", 100);
    Ok(OperationResult {
        path: output_file.display().to_string(),
        detail: format!("Sauvegarde complète du Pi · {total} octets avant compression"),
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
    let profile_path = restored_root.join("monde-ensemble-profile.json");
    if profile_path.is_file() {
        let mut profile: serde_json::Value =
            serde_json::from_slice(&fs::read(&profile_path).map_err(io_error)?)
                .map_err(|_| "Le profil contenu dans la sauvegarde est illisible.".to_string())?;
        if profile
            .get("application")
            .and_then(serde_json::Value::as_str)
            == Some("Monde Ensemble")
        {
            progress("Adaptation du runtime Java à cet ordinateur…", 88);
            let java_major = profile.get("java_runtime").and_then(serde_json::Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .or_else(|| profile.get("minecraft_java").and_then(serde_json::Value::as_str).and_then(|version| java_major_for(version).ok()))
                .ok_or_else(|| "La sauvegarde ne précise pas un runtime Java vérifié. Le dossier original reste intact.".to_string())?;
            let heap_mb = profile_heap_megabytes(&profile)
                .filter(|value| (128..=6144).contains(value))
                .unwrap_or(2048);
            let java_port = profile
                .get("java_port")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or(JAVA_DEFAULT_PORT);
            let bedrock_port = profile
                .get("bedrock_port")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or(BEDROCK_DEFAULT_PORT);
            let client = client()?;
            let runtime = runtime_package(java_major, false, &progress, 88)?;
            let runtime_archive = staging.join(&runtime.name);
            download_url_verified(&client, &runtime.link, &runtime.checksum, &runtime_archive)
                .map_err(|error| format!("Téléchargement du Java adapté impossible; aucune restauration n’a été publiée. {error}"))?;
            let portable_runtime = staging.join("runtime-portable");
            unpack_runtime(&runtime_archive, portable_runtime.clone())?;
            fs::remove_file(&runtime_archive).map_err(io_error)?;
            let original_runtime = restored_root.join("runtime");
            if original_runtime.exists() {
                fs::remove_dir_all(&original_runtime).map_err(io_error)?;
            }
            fs::rename(&portable_runtime, &original_runtime).map_err(io_error)?;
            write_launch_files(&restored_root, heap_mb, false, java_port, bedrock_port)?;
            migrate_restored_geyser_config(&restored_root, &mut profile)?;
            set_private_file_permissions(&restored_root.join("server.properties"))?;
            set_private_file_permissions(&profile_path)?;
        }
    }
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

/// Restores a verified archive to a new sibling server on an already configured Pi.
/// The existing installation is never replaced, and the restored server is not started.
pub fn restore_pi_backup(
    host: &str,
    username: &str,
    archive_path: &Path,
    progress: impl Fn(&str, u8),
) -> Result<OperationResult, String> {
    if !valid_remote_name(host) || !valid_remote_name(username) {
        return Err("Nom du Raspberry Pi ou compte invalide.".into());
    }
    let archive_hash_before = hash_file(archive_path)?;
    let manifest_path = PathBuf::from(format!("{}.sha256", archive_path.display()));
    if manifest_path.is_file() {
        let manifest = fs::read_to_string(&manifest_path).map_err(io_error)?;
        let expected = manifest.split_whitespace().next().unwrap_or_default();
        if !valid_sha256(expected) || !expected.eq_ignore_ascii_case(&archive_hash_before) {
            return Err(
                "Le contrôle d’intégrité de la sauvegarde a échoué. Restauration bloquée.".into(),
            );
        }
    }

    progress("Vérification de l’archive et de l’espace sur le Pi…", 3);
    let inspect = run_ssh(&format!("{username}@{host}"), &remote_probe_command())?;
    let facts = parse_remote_facts(&inspect)?;
    if facts.architecture != "aarch64" {
        return Err(format!(
            "{} utilise {}. La restauration du serveur demande un Raspberry Pi OS 64 bits.",
            facts.model, facts.architecture
        ));
    }
    if facts.memory_gb < MINIMUM_PI_MEMORY_GB {
        return Err(format!(
            "{} n’a que {:.1} Go de mémoire détectée. La restauration du serveur demande un Pi 3B+ ou plus récent avec 1 Go de mémoire.",
            facts.model, facts.memory_gb
        ));
    }
    if [
        facts.java_default_busy,
        facts.java_alternate_busy,
        facts.bedrock_default_busy,
        facts.bedrock_alternate_busy,
        facts.rcon_default_busy,
        facts.rcon_alternate_busy,
    ]
    .iter()
    .any(|busy| *busy)
    {
        return Err("Un port Minecraft est déjà utilisé sur le Pi. Arrête le serveur actif avant de restaurer une copie.".into());
    }

    let source = File::open(archive_path)
        .map_err(|error| format!("La sauvegarde choisie est inaccessible : {error}"))?;
    let mut archive = Archive::new(GzDecoder::new(source));
    let mut validated = Vec::new();
    let mut paths = HashSet::new();
    let mut total = 0u64;
    for item in archive
        .entries()
        .map_err(|error| format!("Cette sauvegarde ne peut pas être lue : {error}"))?
    {
        let entry = item.map_err(|error| format!("Archive endommagée : {error}"))?;
        let kind = entry.header().entry_type();
        if kind != EntryType::Regular && kind != EntryType::Directory {
            return Err(
                "La sauvegarde contient un lien ou un fichier spécial; restauration bloquée."
                    .into(),
            );
        }
        let path = entry.path().map_err(io_error)?.into_owned();
        if !safe_archive_path(&path) || !paths.insert(path.clone()) {
            return Err(
                "La sauvegarde contient un chemin dangereux ou en double; restauration bloquée."
                    .into(),
            );
        }
        let size = entry.size();
        total = total.saturating_add(size);
        if total > MAX_RESTORE_BYTES {
            return Err("La sauvegarde dépasse la taille maximale acceptée.".into());
        }
        validated.push((path, kind, size));
    }
    if validated.is_empty()
        || !validated
            .iter()
            .any(|(path, _, _)| path.starts_with("server/"))
    {
        return Err("Cette sauvegarde n’a pas la structure attendue (dossier server).".into());
    }
    let compressed_size = fs::metadata(archive_path).map_err(io_error)?.len();
    let required_space = total
        .saturating_add(compressed_size)
        .saturating_add(512 * 1024 * 1024);
    if facts.free_gb < 2 || required_space > facts.free_gb.saturating_mul(1024 * 1024 * 1024) {
        return Err(format!(
            "Il reste environ {} Go libres sur le Pi, trop peu pour restaurer cette copie en conservant l’installation existante.",
            facts.free_gb
        ));
    }

    let staging = std::env::temp_dir().join(format!(
        "monde-ensemble-pi-restore-{}",
        Uuid::new_v4().simple()
    ));
    fs::create_dir(&staging).map_err(io_error)?;
    let _cleanup = RemoveDirOnDrop(staging.clone());
    let extract_root = staging.join("extrait");
    fs::create_dir(&extract_root).map_err(io_error)?;
    progress("Contrôle et préparation d’une copie propre…", 12);
    extract_validated(archive_path, &extract_root, &validated, &progress)?;
    let restored_root = extract_root.join("server");
    let profile_path = restored_root.join("monde-ensemble-profile.json");
    let profile: serde_json::Value = serde_json::from_slice(&fs::read(&profile_path).map_err(io_error)?)
        .map_err(|_| "Le profil du serveur est absent ou illisible; cette sauvegarde ne peut pas être adaptée au Pi.".to_string())?;
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
        || !restored_root.join("paper.jar").is_file()
        || !restored_root.join("plugins/Geyser-Spigot.jar").is_file()
        || !restored_root.join("plugins/Floodgate-Spigot.jar").is_file()
        || !restored_root.join("server.properties").is_file()
    {
        return Err("Cette sauvegarde ne contient pas un serveur Monde Ensemble complet (Paper, Geyser et Floodgate). Elle reste inchangée.".into());
    }
    let java_major = profile
        .get("java_runtime")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .or_else(|| {
            profile
                .get("minecraft_java")
                .and_then(serde_json::Value::as_str)
                .and_then(|version| java_major_for(version).ok())
        })
        .ok_or_else(|| {
            "La sauvegarde ne précise pas une version Java compatible connue.".to_string()
        })?;
    let java_port = profile
        .get("java_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .filter(|port| *port > 0)
        .unwrap_or(JAVA_DEFAULT_PORT);
    let bedrock_port = profile
        .get("bedrock_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .filter(|port| *port > 0)
        .unwrap_or(BEDROCK_DEFAULT_PORT);
    let heap_limit_mb = pi_heap_megabytes(facts.memory_gb);
    let heap_mb = profile_heap_megabytes(&profile)
        .filter(|heap| (128..=heap_limit_mb).contains(heap))
        .unwrap_or(heap_limit_mb);
    progress(
        "Téléchargement du Java officiel adapté au Raspberry Pi…",
        65,
    );
    let client = client()?;
    let runtime = runtime_package(java_major, true, &progress, 65)?;
    let runtime_archive = staging.join(&runtime.name);
    download_url_verified(&client, &runtime.link, &runtime.checksum, &runtime_archive)?;
    let portable_runtime = staging.join("runtime-pi");
    unpack_runtime(&runtime_archive, portable_runtime.clone())?;
    fs::remove_file(&runtime_archive).map_err(io_error)?;
    let original_runtime = restored_root.join("runtime");
    if original_runtime.exists() {
        fs::remove_dir_all(&original_runtime).map_err(io_error)?;
    }
    fs::rename(&portable_runtime, &original_runtime).map_err(io_error)?;
    let profile_file = fs::read(&profile_path).map_err(io_error)?;
    let mut profile: serde_json::Value = serde_json::from_slice(&profile_file).map_err(io_error)?;
    migrate_restored_geyser_config(&restored_root, &mut profile)?;
    let object = profile
        .as_object_mut()
        .ok_or_else(|| "Le profil du serveur est invalide.".to_string())?;
    object.insert("java_runtime".into(), serde_json::json!(java_major));
    object.insert(
        "java_heap_gb".into(),
        serde_json::json!(heap_mb.div_ceil(1024)),
    );
    object.insert("java_heap_mb".into(), serde_json::json!(heap_mb));
    object.insert(
        "resource_profile".into(),
        serde_json::json!(if heap_mb <= COMPACT_PI_HEAP_MB {
            "compact-pi"
        } else {
            "standard"
        }),
    );
    object.insert("restored_for".into(), serde_json::json!("linux-aarch64"));
    object.insert(
        "restored_at".into(),
        serde_json::json!(Utc::now().to_rfc3339()),
    );
    fs::write(
        &profile_path,
        serde_json::to_vec_pretty(&profile).map_err(|error| error.to_string())?,
    )
    .map_err(io_error)?;
    write_launch_files(&restored_root, heap_mb, true, java_port, bedrock_port)?;
    if heap_mb <= COMPACT_PI_HEAP_MB {
        let properties_path = restored_root.join("server.properties");
        let properties = read_server_properties(&properties_path)?;
        let properties = set_server_property_values(
            &properties,
            &[
                ("max-players", "3"),
                ("view-distance", "4"),
                ("simulation-distance", "3"),
            ],
        );
        fs::write(&properties_path, properties).map_err(io_error)?;
    }
    let _ = fs::remove_file(restored_root.join("demarrer.bat"));
    set_private_file_permissions(&restored_root.join("server.properties"))?;
    set_private_file_permissions(&profile_path)?;

    let upload_archive = staging.join("serveur-pi.tar.gz");
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&upload_archive)
        .map_err(io_error)?;
    let encoder = GzEncoder::new(output, Compression::default());
    let mut builder = Builder::new(encoder);
    builder
        .append_dir_all("server", &restored_root)
        .map_err(io_error)?;
    let encoder = builder.into_inner().map_err(io_error)?;
    encoder.finish().map_err(io_error)?;
    let archive_hash_after = hash_file(archive_path)?;
    if archive_hash_after != archive_hash_before {
        return Err(
            "La sauvegarde source a changé pendant sa vérification; aucune copie n’a été envoyée."
                .into(),
        );
    }
    let upload_hash = hash_file(&upload_archive)?;
    let id = Uuid::new_v4().simple().to_string();
    let remote_archive = format!(".monde-ensemble-restauration-{id}.tar.gz");
    let server_folder = format!("Monde-Ensemble-Restaure-{id}/server");
    let destination = format!("{username}@{host}");
    progress("Envoi chiffré et vérification de la copie sur le Pi…", 84);
    let mut child = scp_command()
        .args([
            "-p",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "PasswordAuthentication=no",
            upload_archive.to_string_lossy().as_ref(),
            &format!("{destination}:{remote_archive}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            "L’outil de transfert sécurisé n’est pas disponible sur cet ordinateur.".to_string()
        })?;
    let deadline = std::time::Instant::now() + Duration::from_secs(1800);
    let transfer = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io_error(error));
            }
            Ok(None) if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(250)),
        }
    };
    if !transfer {
        let _ = run_ssh(
            &destination,
            &format!("rm -f -- \"$HOME/{remote_archive}\""),
        );
        return Err("Le transfert sécurisé a échoué ou a dépassé 30 minutes. Le serveur existant reste intact.".into());
    }
    let remote_script = format!(
        r#"set -eu
umask 077
a="$HOME/{remote_archive}"
test -f "$a" && test ! -L "$a"
test "$(sha256sum "$a" | cut -d' ' -f1)" = "{upload_hash}"
tar -tzf "$a" >/dev/null
final="$HOME/Monde-Ensemble-Restaure-{id}"
stage="$HOME/.Monde-Ensemble-Restaure-{id}.preparing"
test ! -e "$final" && test ! -e "$stage"
mkdir "$stage"
trap 'rm -rf -- "$stage"; rm -f -- "$a"' EXIT
tar -xzf "$a" -C "$stage"
d="$stage/server"
test -d "$d" && test ! -L "$d"
test -s "$d/paper.jar" && test -s "$d/plugins/Geyser-Spigot.jar" && test -s "$d/plugins/Floodgate-Spigot.jar"
find "$d/runtime" -type f -name java -print -quit | grep -q .
test -s "$d/monde-ensemble-profile.json"
grep -Fq '"application": "Monde Ensemble"' "$d/monde-ensemble-profile.json"
if find "$d" -type l -print -quit | grep -q .; then exit 6; fi
chmod 600 "$d/server.properties" "$d/monde-ensemble-profile.json"
mv -- "$stage/server" "$final/server"
rmdir "$stage"
rm -f -- "$a"
trap - EXIT
printf 'RESTORED=%s\n' 'Monde-Ensemble-Restaure-{id}/server'
"#
    );
    let uploaded = match run_ssh_timeout(&destination, &remote_script, 240) {
        Ok(output) => output,
        Err(error) => {
            let cleanup = format!(
                "rm -rf -- \"$HOME/.Monde-Ensemble-Restaure-{id}.preparing\"; rm -f -- \"$HOME/{remote_archive}\""
            );
            let _ = run_ssh(&destination, &cleanup);
            return Err(format!(
                "Le Pi n’a pas confirmé la copie complète; son installation précédente reste intacte. {error}"
            ));
        }
    };
    if !uploaded
        .lines()
        .any(|line| line.trim() == format!("RESTORED={server_folder}"))
    {
        return Err("Le Pi n’a pas confirmé le nom de la copie restaurée. Le serveur existant reste intact.".into());
    }
    progress(
        "Nouvelle copie vérifiée sur le Pi. Elle n’a pas été démarrée.",
        100,
    );
    Ok(OperationResult {
        path: server_folder,
        detail: format!(
            "Copie restaurée sur le Pi · {} · Java {java_major} ARM64 · mémoire Java {} · l’ancien serveur est conservé · cette copie reste arrêtée",
            facts.model,
            format_heap(heap_mb)
        ),
        sha256: Some(archive_hash_before),
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
            // Archives made by tar/append_dir_all commonly include an explicit
            // root `server/` directory. It is validated even though the
            // staging directory already exists and needs no extraction.
            extracted_count += 1;
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

fn safe_runtime_archive_path(path: &Path, archive_root: &mut Option<std::ffi::OsString>) -> bool {
    if path.as_os_str().is_empty() || path.to_string_lossy().contains('\\') {
        return false;
    }
    let components: Vec<_> = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect::<Option<_>>()
        .unwrap_or_default();
    let Some(root) = components.first() else {
        return false;
    };
    if !root.to_string_lossy().starts_with("jdk-") {
        return false;
    }
    match archive_root {
        Some(expected) => expected == root,
        None => {
            *archive_root = Some(root.clone());
            true
        }
    }
}

fn safe_runtime_symlink_target(
    link_path: &Path,
    link_name: &Path,
    archive_root: &std::ffi::OsStr,
) -> Option<PathBuf> {
    if link_name.is_absolute() || link_name.to_string_lossy().contains('\\') {
        return None;
    }
    let mut resolved = Vec::<std::ffi::OsString>::new();
    for component in link_path.parent()?.components() {
        match component {
            Component::Normal(value) => resolved.push(value.to_os_string()),
            _ => return None,
        }
    }
    for component in link_name.components() {
        match component {
            Component::Normal(value) => resolved.push(value.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir if resolved.len() > 1 => {
                resolved.pop();
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if resolved.first().map(std::ffi::OsString::as_os_str) != Some(archive_root) {
        return None;
    }
    let mut target = PathBuf::new();
    target.extend(resolved);
    Some(target)
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
        let mut archive_root = None;
        let mut paths = HashSet::new();
        let mut regular_files = HashSet::new();
        let mut symlinks = Vec::new();
        for item in archive.entries().map_err(io_error)? {
            let mut item = item.map_err(io_error)?;
            let path = item.path().map_err(io_error)?.into_owned();
            if !safe_runtime_archive_path(&path, &mut archive_root) || !paths.insert(path.clone()) {
                return Err("Archive Java contient un chemin ou un lien invalide.".into());
            }
            let entry_type = item.header().entry_type();
            if entry_type.is_hard_link()
                || !(entry_type.is_file() || entry_type.is_dir() || entry_type.is_symlink())
            {
                return Err("Archive Java contient un type d’élément non autorisé.".into());
            }
            if entry_type.is_symlink() {
                let link_name = item
                    .link_name()
                    .map_err(io_error)?
                    .ok_or_else(|| "Lien Java sans cible.".to_string())?
                    .into_owned();
                let root = archive_root
                    .as_deref()
                    .ok_or_else(|| "Dossier racine Java introuvable.".to_string())?;
                let resolved_target = safe_runtime_symlink_target(&path, &link_name, root)
                    .ok_or_else(|| {
                        "Archive Java contient un lien hors de son dossier.".to_string()
                    })?;
                symlinks.push((path, link_name, resolved_target));
                io::copy(&mut item, &mut io::sink()).map_err(io_error)?;
                continue;
            }
            if entry_type.is_file() {
                regular_files.insert(path.clone());
            }
            let destination_path = destination.join(path);
            if !destination_path.starts_with(&destination) {
                return Err("Chemin Java hors du dossier prévu.".into());
            }
            if !item.unpack_in(&destination).map_err(io_error)? {
                return Err("Archive Java contient un chemin hors du dossier prévu.".into());
            }
        }
        for (path, link_name, resolved_target) in symlinks {
            if !regular_files.contains(&resolved_target) {
                return Err("Un lien Java doit viser un fichier régulier de l’archive.".into());
            }
            let destination_path = destination.join(path);
            let target_path = destination.join(resolved_target);
            if !destination_path.starts_with(&destination) || !target_path.starts_with(&destination)
            {
                return Err("Lien Java hors du dossier prévu.".into());
            }
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent).map_err(io_error)?;
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(link_name, destination_path).map_err(io_error)?;
            #[cfg(windows)]
            fs::copy(target_path, destination_path).map_err(io_error)?;
        }
    }
    let _java = WalkDir::new(&destination)
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
        let mut permissions = fs::metadata(_java.path()).map_err(io_error)?.permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        fs::set_permissions(_java.path(), permissions).map_err(io_error)?;
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
        .user_agent(concat!("MondeEnsemble/", env!("CARGO_PKG_VERSION")))
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

fn record_managed_artifact_hashes(
    profile: &mut serde_json::Value,
    server_dir: &Path,
) -> Result<(), String> {
    let hashes = [
        ("paper_sha256", "paper.jar"),
        ("geyser_sha256", "plugins/Geyser-Spigot.jar"),
        ("floodgate_sha256", "plugins/Floodgate-Spigot.jar"),
    ]
    .into_iter()
    .map(|(field, path)| Ok((field, hash_file(&server_dir.join(path))?)))
    .collect::<Result<Vec<_>, String>>()?;
    let object = profile
        .as_object_mut()
        .ok_or_else(|| "Le profil du serveur n’est pas un objet lisible.".to_string())?;
    for (field, checksum) in hashes {
        object.insert(field.into(), serde_json::json!(checksum));
    }
    Ok(())
}

fn validate_pi_server_package(path: &Path) -> Result<(), String> {
    let file = File::open(path).map_err(io_error)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    let required = [
        "server/paper.jar",
        "server/plugins/Geyser-Spigot.jar",
        "server/plugins/Floodgate-Spigot.jar",
    ];
    let mut found = HashSet::new();
    for entry in archive.entries().map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let name = entry
            .path()
            .map_err(io_error)?
            .to_string_lossy()
            .into_owned();
        if required.contains(&name.as_str())
            && entry.header().entry_type().is_file()
            && entry.header().size().map_err(io_error)? > 0
        {
            found.insert(name);
        }
    }
    for name in required {
        if !found.contains(name) {
            return Err(format!(
                "L’archive destinée au Pi ne contient pas le fichier requis exact : {name}. Aucun fichier n’a été envoyé."
            ));
        }
    }
    Ok(())
}

fn hash_regular_file(path: &Path) -> Option<String> {
    fs::symlink_metadata(path)
        .ok()
        .filter(|metadata| metadata.file_type().is_file())
        .and_then(|_| hash_file(path).ok())
}

fn hash_file_sha512(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha512::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

/// Current Geyser plugin JARs can omit `config.yml`: Geyser supplies the rest
/// of its configuration from annotated defaults when it loads this file.
/// Keep only the settings needed for the managed Java/Bedrock bridge here.
const GEYSER_CONFIG_FALLBACK: &str = "bedrock:\n  port: 19132\n  transport: nethernet\nremote:\n  address: auto\n  auth-type: online\n";

fn geyser_config_from_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<String, String> {
    match archive.by_name("config.yml") {
        Ok(mut entry) => {
            let mut template = String::new();
            entry.read_to_string(&mut template).map_err(io_error)?;
            Ok(template)
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(GEYSER_CONFIG_FALLBACK.to_string()),
        Err(error) => Err(format!("Impossible de lire le modèle Geyser : {error}")),
    }
}

fn geyser_config_matches_profile(server_dir: &Path, profile: &serde_json::Value) -> bool {
    let config_path = server_dir.join("plugins/Geyser-Spigot/config.yml");
    let Some(actual_hash) = hash_regular_file(&config_path) else {
        return false;
    };
    if profile
        .get("geyser_config_sha256")
        .and_then(serde_json::Value::as_str)
        .filter(|digest| valid_sha256(digest))
        .is_some_and(|expected| actual_hash != expected)
    {
        return false;
    }

    // Older managed profiles did not record a config digest. Compare their
    // current config with the exact generated defaults from their installed
    // Geyser jar, so an untouched legacy install can still be recognized.
    let Some(bedrock_port) = profile
        .get("bedrock_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
    else {
        return false;
    };
    let jar_path = server_dir.join("plugins/Geyser-Spigot.jar");
    if hash_regular_file(&jar_path).is_none() {
        return false;
    }
    let Ok(file) = File::open(jar_path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let Ok(template) = geyser_config_from_archive(&mut archive) else {
        return false;
    };
    let modern = managed_geyser_config(&template, bedrock_port);
    if format!("{:x}", Sha256::digest(modern.as_bytes())) == actual_hash {
        return true;
    }

    // Older managed profiles used the same official template without an
    // explicit transport. Recognize that exact default so an update can
    // migrate it transactionally instead of making the user edit YAML.
    let legacy = set_yaml(&template, "bedrock", "port", &bedrock_port.to_string());
    let legacy = set_yaml(&legacy, "remote", "auth-type", "floodgate");
    format!("{:x}", Sha256::digest(legacy.as_bytes())) == actual_hash
}

/// Migrates only an exact application-managed Geyser default during restore.
/// Customized or unverifiable configurations are preserved byte-for-byte.
fn migrate_restored_geyser_config(
    server_dir: &Path,
    profile: &mut serde_json::Value,
) -> Result<bool, String> {
    if profile
        .get("application")
        .and_then(serde_json::Value::as_str)
        != Some("Monde Ensemble")
        || geyser_nethernet_ready(server_dir)
    {
        return Ok(false);
    }
    let port = profile
        .get("bedrock_port")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            "Le port Bedrock de la sauvegarde est invalide; sa configuration reste inchangée."
                .to_string()
        })?;
    let config_path = server_dir.join("plugins/Geyser-Spigot/config.yml");
    let Some(actual_hash) = hash_regular_file(&config_path) else {
        return Ok(false);
    };
    if profile
        .get("geyser_config_sha256")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|expected| expected != actual_hash)
    {
        return Ok(false);
    }

    let jar = File::open(server_dir.join("plugins/Geyser-Spigot.jar")).map_err(io_error)?;
    let mut archive = zip::ZipArchive::new(jar)
        .map_err(|error| format!("Le fichier Geyser de la sauvegarde est illisible : {error}"))?;
    let template = geyser_config_from_archive(&mut archive)?;
    // An unchanged hash is not sufficient evidence that the file is the
    // standard template: require the exact prior app-generated configuration
    // before replacing it. Custom or unfamiliar YAML stays untouched.
    let legacy = set_yaml(&template, "bedrock", "port", &port.to_string());
    let legacy = set_yaml(&legacy, "remote", "auth-type", "floodgate");
    if format!("{:x}", Sha256::digest(legacy.as_bytes())) != actual_hash {
        return Ok(false);
    }
    let config = managed_geyser_config(&template, port);
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    fs::write(&config_path, config).map_err(io_error)?;
    let object = profile
        .as_object_mut()
        .ok_or_else(|| "Le profil de la sauvegarde est invalide.".to_string())?;
    object.insert(
        "geyser_config_sha256".into(),
        serde_json::json!(hash_file(&config_path)?),
    );
    object.insert("bedrock_transport".into(), serde_json::json!("nethernet"));
    fs::write(
        server_dir.join("monde-ensemble-profile.json"),
        serde_json::to_vec_pretty(profile).map_err(|error| error.to_string())?,
    )
    .map_err(io_error)?;
    Ok(true)
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

fn valid_plugin_filename(value: &str) -> bool {
    value.ends_with(".jar")
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-+ ".contains(&byte))
}

fn network_error(error: reqwest::Error) -> String {
    format!("Catalogue ou téléchargement indisponible : {error}")
}

fn io_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        GEYSER_CONFIG_FALLBACK, GameSettings, LocalStartResult, PAPER_READY_LOG_PATTERN,
        REMOTE_SERVER_ARTIFACT_CHECK, RaspberryPiOsEntry, RemoveDirOnDrop, add_identity_file,
        apply_game_settings, artifact_hash_matches, copy_verified_pack, geyser_config_from_archive,
        hash_file, inspect_bedrock_resource_pack, java_major_for, latest_supported_pi_image,
        parse_game_settings, parse_supported_versions_page, record_managed_artifact_hashes,
        restore_backup, safe_archive_path, safe_version, shell_single_quote, ssh_failure_message,
        start_updated_local_copy_with_fallback, unpack_runtime, valid_bedrock_pack_filename,
        valid_plugin_filename, valid_server_folder, valid_sha256, validate_game_settings,
        validate_pi_server_package,
    };
    use flate2::{Compression, write::GzEncoder};
    use std::{fs, fs::File, io::Write, path::Path, process::Command};
    use tar::{Builder, EntryType, Header};

    fn test_directory() -> (std::path::PathBuf, RemoveDirOnDrop) {
        let path = std::env::temp_dir().join(format!(
            "monde-ensemble-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&path).unwrap();
        (path.clone(), RemoveDirOnDrop(path))
    }

    #[test]
    fn pi_install_reports_the_exact_server_artifact_missing_after_transfer() {
        assert!(REMOTE_SERVER_ARTIFACT_CHECK.contains("paper.jar"));
        assert!(REMOTE_SERVER_ARTIFACT_CHECK.contains("plugins/Geyser-Spigot.jar"));
        assert!(REMOTE_SERVER_ARTIFACT_CHECK.contains("plugins/Floodgate-Spigot.jar"));
        assert!(REMOTE_SERVER_ARTIFACT_CHECK.contains("Fichier requis absent ou vide"));
    }

    #[test]
    fn pi_whitelist_accepts_managed_server_folder_and_rejects_paths_outside_it() {
        assert!(valid_server_folder(
            "Monde-Ensemble-693799448ac94df8bdffc62de817f362/server"
        ));
        assert!(!valid_server_folder("../server"));
        assert!(!valid_server_folder(
            "Monde-Ensemble-abc/server/../../other"
        ));
        assert!(!valid_server_folder("server"));
    }

    #[test]
    fn pi_package_requires_case_exact_nonempty_floodgate_filename() {
        let (directory, _cleanup) = test_directory();
        let create_archive = |path: &Path, floodgate_name: &str| {
            let file = File::create(path).unwrap();
            let encoder = GzEncoder::new(file, Compression::fast());
            let mut archive = Builder::new(encoder);
            for name in [
                "server/paper.jar",
                "server/plugins/Geyser-Spigot.jar",
                floodgate_name,
            ] {
                let bytes = b"jar";
                let mut header = Header::new_gnu();
                header.set_size(bytes.len() as u64);
                header.set_mode(0o600);
                header.set_cksum();
                archive.append_data(&mut header, name, &bytes[..]).unwrap();
            }
            archive.into_inner().unwrap().finish().unwrap();
        };

        let lowercase = directory.join("lowercase.tar.gz");
        create_archive(&lowercase, "server/plugins/floodgate-spigot.jar");
        let error = validate_pi_server_package(&lowercase).unwrap_err();
        assert!(error.contains("server/plugins/Floodgate-Spigot.jar"));

        let canonical = directory.join("canonical.tar.gz");
        create_archive(&canonical, "server/plugins/Floodgate-Spigot.jar");
        assert!(validate_pi_server_package(&canonical).is_ok());
    }

    #[test]
    fn pi_startup_waits_for_paper_done_not_geyser_done() {
        let ready = regex::Regex::new(PAPER_READY_LOG_PATTERN).unwrap();
        assert!(ready.is_match("[20:10:29 INFO]: Done (431.901s)! For help, type help"));
        assert!(!ready.is_match(
            "[20:10:20 INFO]: [Geyser-Spigot] Done (134.21s)! Run /geyser help for help"
        ));
    }

    #[test]
    fn silent_remote_command_failure_is_not_misreported_as_ssh_refusal() {
        let message = ssh_failure_message(Some(42), "");
        assert!(message.contains("commande distante"));
        assert!(message.contains("code 42"));
        assert!(!message.contains("refusée"));
        assert!(
            ssh_failure_message(Some(255), "ssh: Connection refused")
                .contains("Connection refused")
        );
    }

    #[test]
    fn geyser_config_uses_managed_defaults_when_jar_has_no_template() {
        let (directory, _cleanup) = test_directory();
        let jar_path = directory.join("geyser.jar");
        zip::ZipWriter::new(File::create(&jar_path).unwrap())
            .finish()
            .unwrap();

        let file = File::open(jar_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let template = geyser_config_from_archive(&mut archive).unwrap();
        let configured = super::set_yaml(&template, "bedrock", "port", "19133");
        let configured = super::set_yaml(&configured, "remote", "auth-type", "floodgate");

        assert_eq!(template, GEYSER_CONFIG_FALLBACK);
        assert!(configured.contains("  port: 19133"));
        assert!(configured.contains("  address: auto"));
        assert!(configured.contains("  auth-type: floodgate"));
    }

    #[test]
    fn geyser_config_keeps_template_when_jar_contains_one() {
        let (directory, _cleanup) = test_directory();
        let jar_path = directory.join("geyser.jar");
        let mut writer = zip::ZipWriter::new(File::create(&jar_path).unwrap());
        writer
            .start_file("config.yml", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bedrock:\n  port: 19132\n").unwrap();
        writer.finish().unwrap();

        let file = File::open(jar_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        assert_eq!(
            geyser_config_from_archive(&mut archive).unwrap(),
            "bedrock:\n  port: 19132\n"
        );
    }

    fn write_gzip_tar_entry(path: &Path, entry_path: &str, contents: &[u8]) {
        let file = File::create(path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut archive = Builder::new(encoder);
        let mut header = Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o600);
        let entry_path = entry_path.as_bytes();
        assert!(entry_path.len() <= 100);
        header.as_mut_bytes()[..entry_path.len()].copy_from_slice(entry_path);
        header.set_cksum();
        // The normal builder refuses traversal paths. Use a raw header here
        // to model an archive crafted outside this application.
        archive.append(&header, contents).unwrap();
        archive.finish().unwrap();
        archive.into_inner().unwrap().finish().unwrap();
    }

    fn write_runtime_tar(path: &Path, symlink_target: &str) {
        let file = File::create(path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut archive = Builder::new(encoder);
        let mut root_header = Header::new_gnu();
        root_header.set_entry_type(EntryType::Directory);
        root_header.set_size(0);
        root_header.set_mode(0o755);
        root_header.set_cksum();
        archive
            .append_data(&mut root_header, "jdk-25.0.5+8-jre", &b""[..])
            .unwrap();
        archive
            .append_data(
                &mut runtime_file_header(b"java runtime"),
                "jdk-25.0.5+8-jre/bin/java",
                &b"java runtime"[..],
            )
            .unwrap();
        archive
            .append_data(
                &mut runtime_file_header(b"license text"),
                "jdk-25.0.5+8-jre/legal/java.base/LICENSE",
                &b"license text"[..],
            )
            .unwrap();
        let mut link_header = Header::new_gnu();
        link_header.set_entry_type(EntryType::Symlink);
        link_header.set_size(0);
        link_header.set_mode(0o777);
        link_header.set_cksum();
        archive
            .append_link(
                &mut link_header,
                "jdk-25.0.5+8-jre/legal/jdk.unsupported/LICENSE",
                symlink_target,
            )
            .unwrap();
        archive.finish().unwrap();
        archive.into_inner().unwrap().finish().unwrap();
    }

    fn runtime_file_header(contents: &[u8]) -> Header {
        let mut header = Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        header
    }

    fn write_pack_zip(path: &Path, manifest: &str) {
        let file = File::create(path).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file("manifest.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(manifest.as_bytes()).unwrap();
        archive.finish().unwrap();
    }

    fn pi_os_entry(release_date: &str) -> RaspberryPiOsEntry {
        RaspberryPiOsEntry {
            name: "Raspberry Pi OS Lite (64-bit)".into(),
            release_date: Some(release_date.into()),
            url: Some("https://downloads.raspberrypi.com/raspios_lite_arm64/test.img.xz".into()),
            extract_sha256: Some("a".repeat(64)),
            image_download_sha256: Some("b".repeat(64)),
            image_download_size: Some(1_000_000_000),
            extract_size: Some(2_000_000_000),
            init_format: Some("cloudinit-rpi".into()),
            devices: Some(vec!["pi4-64bit".into(), "pi5-64bit".into()]),
            subitems: None,
        }
    }

    #[test]
    fn java_catalog_reads_current_adoptium_binary_shape_and_requires_sha256() {
        let body = format!(
            r#"[{{"binary":{{"package":{{"checksum":"{}","link":"https://example.com/java.tar.gz","name":"java.tar.gz"}}}}}}]"#,
            "a".repeat(64)
        );
        let package = super::runtime_package_from_json(body.as_bytes(), 25).unwrap();
        assert_eq!(package.name, "java.tar.gz");
        assert_eq!(package.checksum, "a".repeat(64));

        let missing_checksum = br#"[{"binary":{"package":{"checksum":"","link":"https://example.com/java.tar.gz","name":"java.tar.gz"}}}]"#;
        assert!(super::runtime_package_from_json(missing_checksum, 25).is_err());
        assert!(super::runtime_package_from_json(b"not json", 25).is_err());
    }

    #[test]
    fn pi_os_selection_accepts_release_named_entries_and_requires_image_verification() {
        let older_verified = pi_os_entry("2026-09-01");
        let newer_release_without_optional_download_hash = RaspberryPiOsEntry {
            name: "Raspberry Pi OS Lite (64-bit), 2026-10-01 release (Debian 13 Trixie)".into(),
            release_date: Some("2026-10-01".into()),
            image_download_sha256: None,
            ..pi_os_entry("2026-10-01")
        };

        let selected = latest_supported_pi_image(vec![
            older_verified,
            newer_release_without_optional_download_hash,
        ])
        .unwrap();
        assert_eq!(selected.release_date.as_deref(), Some("2026-10-01"));
        assert!(selected.extract_sha256.as_deref().is_some_and(valid_sha256));

        let only_unverified = RaspberryPiOsEntry {
            extract_sha256: None,
            ..pi_os_entry("2026-10-01")
        };
        assert!(latest_supported_pi_image(vec![only_unverified]).is_none());
    }

    #[test]
    fn expert_pi_os_options_require_supported_64_bit_hardware_and_first_boot_setup() {
        let mut supported = pi_os_entry("2026-10-01");
        supported.name = "Raspberry Pi OS (64-bit) with desktop".into();
        supported.devices = Some(vec!["pi4-64bit".into()]);
        let option = super::pi_os_entry_to_option(supported).unwrap();
        assert_eq!(option.minimum_server_memory_gb, 4.0);
        assert_eq!(option.devices, vec!["pi4-64bit"]);

        assert_eq!(super::pi_os_device_generation("pi3-64bit"), Some(3));
        assert_eq!(super::pi_os_device_generation("pi5-64bit"), Some(5));
        assert_eq!(super::pi_os_device_generation("pi3-32bit"), None);

        let no_first_boot = RaspberryPiOsEntry {
            init_format: None,
            ..pi_os_entry("2026-10-01")
        };
        assert!(super::pi_os_entry_to_option(no_first_boot).is_none());

        let untrusted_image_host = RaspberryPiOsEntry {
            url: Some("https://example.com/os.img.xz".into()),
            ..pi_os_entry("2026-10-01")
        };
        assert!(super::pi_os_entry_to_option(untrusted_image_host).is_none());
    }

    #[test]
    fn bedrock_pack_import_requires_a_valid_resource_manifest_and_declares_minimum() {
        let (directory, _cleanup) = test_directory();
        let pack_path = directory.join("Shared textures.mcpack");
        write_pack_zip(
            &pack_path,
            r#"{
                "format_version": 2,
                "header": {
                    "name": "Shared textures",
                    "uuid": "f0a1b2c3-d4e5-4678-9abc-def012345678",
                    "version": [1, 0, 0],
                    "min_engine_version": [1, 21, 0]
                },
                "modules": [{
                    "type": "resources",
                    "uuid": "a0a1b2c3-d4e5-4678-9abc-def012345678",
                    "version": [1, 0, 0]
                }]
            }"#,
        );
        let pack = inspect_bedrock_resource_pack(&pack_path).unwrap();
        assert_eq!(pack.name, "Shared textures");
        assert_eq!(pack.filename, "Shared textures.mcpack");
        assert_eq!(pack.minimum_engine_version, "1.21.0");
        assert_eq!(pack.sha256.len(), 64);
    }

    #[test]
    fn bedrock_pack_import_rejects_java_pack_or_unsafe_filename() {
        let (directory, _cleanup) = test_directory();
        let java_path = directory.join("Java pack.zip");
        write_pack_zip(
            &java_path,
            r#"{"pack":{"pack_format":1,"description":"Java"}}"#,
        );
        assert!(inspect_bedrock_resource_pack(&java_path).is_err());
        assert!(valid_bedrock_pack_filename("pack.mcpack"));
        assert!(valid_bedrock_pack_filename("pack.zip"));
        assert!(!valid_bedrock_pack_filename("../pack.mcpack"));
        assert!(!valid_bedrock_pack_filename("pack.mcpack.sh"));
    }

    #[test]
    fn bedrock_pack_copy_checks_hash_and_never_replaces_an_existing_file() {
        let (directory, _cleanup) = test_directory();
        let source = directory.join("Pack.mcpack");
        write_pack_zip(
            &source,
            r#"{"format_version":2,"header":{"name":"Pack","uuid":"f0a1b2c3-d4e5-4678-9abc-def012345678","version":[1,0,0],"min_engine_version":[1,21,0]},"modules":[{"type":"resources","uuid":"a0a1b2c3-d4e5-4678-9abc-def012345678","version":[1,0,0]}]}"#,
        );
        let pack = inspect_bedrock_resource_pack(&source).unwrap();
        let destination = directory.join("staged.mcpack");
        copy_verified_pack(&pack, &destination).unwrap();
        assert_eq!(hash_file(&destination).unwrap(), pack.sha256);

        assert!(copy_verified_pack(&pack, &destination).is_err());
        assert_eq!(hash_file(&destination).unwrap(), pack.sha256);
    }

    #[test]
    fn ssh_identity_is_used_only_when_the_app_owned_key_exists() {
        let (directory, _cleanup) = test_directory();
        let identity = directory.join("monde-ensemble-pi");
        let mut command = Command::new("ssh");
        add_identity_file(&mut command, &identity);
        assert!(command.get_args().next().is_none());

        fs::write(&identity, "private key placeholder").unwrap();
        add_identity_file(&mut command, &identity);
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy())
            .collect();
        assert_eq!(args[0], "-i");
        assert_eq!(args[1], identity.to_string_lossy());
    }

    #[test]
    fn expert_game_settings_preserve_managed_and_unknown_properties() {
        let original = "# keep this comment\nmotd=Old\nrcon.password=keep-this-secret\nserver-port=25566\ncustom-option=yes\nview-distance=8\nview-distance=10\n";
        let parsed = parse_game_settings(original).unwrap();
        assert_eq!(parsed.motd, "Old");
        assert_eq!(parsed.view_distance, 10);
        let settings = GameSettings {
            motd: "Our shared world".into(),
            gamemode: "survival".into(),
            difficulty: "normal".into(),
            max_players: 24,
            view_distance: 8,
            simulation_distance: 6,
            pvp: false,
            allow_nether: true,
            java_resource_pack_url: "https://packs.example.net/shared.zip".into(),
            java_resource_pack_sha1: "a".repeat(40),
            java_resource_pack_required: true,
        };
        let changed = apply_game_settings(original, &settings).unwrap();
        assert!(changed.contains("# keep this comment\n"));
        assert!(changed.contains("rcon.password=keep-this-secret\n"));
        assert!(changed.contains("server-port=25566\n"));
        assert!(changed.contains("custom-option=yes\n"));
        assert_eq!(changed.matches("view-distance=").count(), 1);
        assert!(changed.contains("view-distance=8\n"));
        assert!(changed.contains("simulation-distance=6\n"));
        assert!(changed.contains("pvp=false\n"));
        assert!(changed.contains("resource-pack=https://packs.example.net/shared.zip\n"));
        assert!(changed.contains(&format!("resource-pack-sha1={}\n", "a".repeat(40))));
        assert!(changed.contains("require-resource-pack=true\n"));
    }

    #[test]
    fn expert_game_settings_reject_invalid_values_and_quote_remote_json() {
        let mut settings = GameSettings {
            motd: "Good world".into(),
            gamemode: "survival".into(),
            difficulty: "normal".into(),
            max_players: 20,
            view_distance: 8,
            simulation_distance: 6,
            pvp: true,
            allow_nether: true,
            java_resource_pack_url: String::new(),
            java_resource_pack_sha1: String::new(),
            java_resource_pack_required: false,
        };
        assert!(validate_game_settings(&settings).is_ok());
        settings.java_resource_pack_url = "http://packs.example.net/shared.zip".into();
        assert!(validate_game_settings(&settings).is_err());
        settings.java_resource_pack_url = "https://packs.example.net/shared.zip".into();
        settings.java_resource_pack_sha1 = "not-a-sha1".into();
        assert!(validate_game_settings(&settings).is_err());
        settings.java_resource_pack_sha1 = "".into();
        settings.java_resource_pack_required = true;
        assert!(validate_game_settings(&settings).is_ok());
        settings.java_resource_pack_url.clear();
        assert!(validate_game_settings(&settings).is_err());
        settings.java_resource_pack_url = "https://packs.example.net/shared.zip".into();
        settings.java_resource_pack_required = false;
        settings.motd = "invalid\nserver-port=1".into();
        assert!(validate_game_settings(&settings).is_err());
        settings.motd = "Good world".into();
        settings.gamemode = "../survival".into();
        assert!(validate_game_settings(&settings).is_err());
        assert_eq!(shell_single_quote("name's world"), "'name'\"'\"'s world'");
    }

    #[test]
    fn remote_facts_keep_the_default_route_address_and_hardware_identity() {
        let facts = super::parse_remote_facts(
            "MODEL=Raspberry Pi 5 Model B Rev 1.0\nMEMORY_GB=7.7\nFREE_GB=24\nARCH=aarch64\nIP=192.168.1.201\nNETWORK_INTERFACE=eth0\nMAC_ADDRESS=d8:3a:dd:12:34:56\nPORT_25565=free\nPORT_25566=free\nPORT_19132=free\nPORT_19133=free\nPORT_25575=free\nPORT_25576=free\n",
        )
        .unwrap();

        assert_eq!(facts.ip, "192.168.1.201");
        assert_eq!(facts.network_interface, "eth0");
        assert_eq!(facts.mac_address, "d8:3a:dd:12:34:56");
    }

    #[test]
    fn restore_archive_paths_must_stay_inside_server_root() {
        for safe in [
            "server",
            "server/world/region/r.0.0.mca",
            "server/server.properties",
        ] {
            assert!(
                safe_archive_path(Path::new(safe)),
                "expected safe path: {safe}"
            );
        }

        for unsafe_path in [
            "",
            "world/level.dat",
            "../outside.txt",
            "server/../../outside.txt",
            "server/world/../../../outside.txt",
            "/server/world/level.dat",
        ] {
            assert!(
                !safe_archive_path(Path::new(unsafe_path)),
                "expected rejected path: {unsafe_path}"
            );
        }
    }

    #[test]
    fn version_and_plugin_names_reject_path_and_command_syntax() {
        assert!(safe_version("26.3"));
        assert!(safe_version("1.21.11-1.21.12"));
        assert!(!safe_version("../latest"));
        assert!(!safe_version("26.3;rm"));

        assert!(valid_plugin_filename("Geyser-Spigot.jar"));
        assert!(!valid_plugin_filename("../Geyser.jar"));
        assert!(!valid_plugin_filename("Geyser.jar.sh"));
    }

    #[test]
    fn java_runtime_gate_matches_papers_published_26_1_requirement() {
        for version in ["26.1", "26.3", "26.10"] {
            assert_eq!(java_major_for(version).unwrap(), 25, "{version}");
        }

        for unknown in ["26.0", "26.", "26.3-preview", "26.3.1", "27.0", "1.21.11"] {
            assert!(java_major_for(unknown).is_err(), "{unknown}");
        }
    }

    #[test]
    fn compatibility_gate_reads_bedrock_range_and_java_version_from_geyser_page() {
        let page = "<h1>Currently supported versions</h1><p>Geyser currently supports Minecraft Bedrock 26.40-26.63 and Minecraft Java 26.3.</p>";
        let versions = parse_supported_versions_page(page).unwrap();

        assert_eq!(versions.bedrock, "26.40-26.63");
        assert_eq!(versions.java, "26.3");
    }

    #[test]
    fn compatibility_gate_blocks_unrecognized_geyser_page_format() {
        for page in [
            "<h1>Supported versions</h1><p>Minecraft Java 26.3 and Bedrock 26.40.</p>",
            "<h1>Currently supported versions</h1><p>Minecraft versions are being updated.</p>",
        ] {
            assert!(parse_supported_versions_page(page).is_err());
        }
    }

    #[test]
    fn update_gate_requires_a_matching_install_hash_or_verified_legacy_build() {
        let recorded = "a".repeat(64);
        let official = "b".repeat(64);

        assert!(artifact_hash_matches("Paper", &recorded, Some(&recorded), None).is_ok());
        assert!(artifact_hash_matches("Paper", &recorded, None, Some(&recorded)).is_ok());
        assert!(artifact_hash_matches("Paper", &recorded, Some(&official), None).is_err());
        assert!(artifact_hash_matches("Paper", &recorded, None, None).is_err());
        assert!(
            artifact_hash_matches("Paper", &recorded, Some("invalid"), Some(&recorded)).is_err()
        );
    }

    #[test]
    fn failed_local_update_starts_and_reports_the_verified_original_copy() {
        let (root, _cleanup) = test_directory();
        let original = root.join("Monde-Ensemble-original");
        let candidate = root.join("Monde-Ensemble-candidate");
        let backup = root.join("sauvegardes/avant-mise-a-jour.tar.gz");
        let mut candidate_stopped = false;
        let mut original_started = false;

        let result = start_updated_local_copy_with_fallback(
            &original,
            &candidate,
            &backup,
            Err("le signal serveur prêt est absent".into()),
            |path| {
                assert_eq!(path, candidate);
                candidate_stopped = true;
                Ok("copie candidate arrêtée proprement".into())
            },
            |path| {
                assert_eq!(path, original);
                original_started = true;
                Ok(LocalStartResult {
                    server_path: path.display().to_string(),
                    message: "Java et Bedrock vérifiés".into(),
                })
            },
            &|_, _| {},
        )
        .unwrap();

        assert!(candidate_stopped);
        assert!(original_started);
        assert_eq!(result.server_path, original.display().to_string());
        assert!(
            result
                .message
                .contains("ancienne version a été relancée et vérifiée")
        );
        assert!(result.message.contains(&backup.display().to_string()));
        assert!(result.message.contains(&candidate.display().to_string()));
        assert!(result.message.contains("Java et Bedrock vérifiés"));
    }

    #[test]
    fn failed_local_update_preserves_diagnostics_when_original_cannot_start() {
        let (root, _cleanup) = test_directory();
        let original = root.join("Monde-Ensemble-original");
        let candidate = root.join("Monde-Ensemble-candidate");
        let backup = root.join("sauvegardes/avant-mise-a-jour.tar.gz");

        let error = match start_updated_local_copy_with_fallback(
            &original,
            &candidate,
            &backup,
            Err("mise à jour candidate impossible".into()),
            |_| Err("serveur candidat toujours actif".into()),
            |_| Err("port Java déjà utilisé".into()),
            &|_, _| {},
        ) {
            Ok(_) => panic!("le fallback devait signaler l’échec du serveur d’origine"),
            Err(error) => error,
        };

        assert!(error.contains(&backup.display().to_string()));
        assert!(error.contains(&candidate.display().to_string()));
        assert!(error.contains("mise à jour candidate impossible"));
        assert!(error.contains("serveur candidat toujours actif"));
        assert!(error.contains("port Java déjà utilisé"));
    }

    #[test]
    fn profile_records_hashes_for_paper_geyser_and_floodgate() {
        let (root, _cleanup) = test_directory();
        fs::create_dir_all(root.join("plugins")).unwrap();
        fs::write(root.join("paper.jar"), b"paper").unwrap();
        fs::write(root.join("plugins/Geyser-Spigot.jar"), b"geyser").unwrap();
        fs::write(root.join("plugins/Floodgate-Spigot.jar"), b"floodgate").unwrap();
        let mut profile = serde_json::json!({});

        record_managed_artifact_hashes(&mut profile, &root).unwrap();

        for (field, path) in [
            ("paper_sha256", "paper.jar"),
            ("geyser_sha256", "plugins/Geyser-Spigot.jar"),
            ("floodgate_sha256", "plugins/Floodgate-Spigot.jar"),
        ] {
            let expected = super::hash_file(&root.join(path)).unwrap();
            assert_eq!(
                profile.get(field).and_then(serde_json::Value::as_str),
                Some(expected.as_str())
            );
        }
    }

    #[test]
    fn restore_rejects_traversal_before_creating_any_world_copy() {
        let (root, _cleanup) = test_directory();
        let archive_path = root.join("unsafe.tar.gz");
        let destination = root.join("restored");
        let outside = root.join("outside.txt");
        write_gzip_tar_entry(
            &archive_path,
            "server/../../outside.txt",
            b"must not escape",
        );

        let result = restore_backup(&archive_path, &destination, |_, _| {});

        assert!(result.is_err());
        assert!(!destination.exists());
        assert!(!outside.exists());
    }

    #[test]
    fn runtime_extractor_accepts_jdk_root_and_safe_relative_symlinks() {
        let (root, _cleanup) = test_directory();
        let archive_path = root.join("runtime.tar.gz");
        let destination = root.join("runtime");
        write_runtime_tar(&archive_path, "../java.base/LICENSE");

        unpack_runtime(&archive_path, destination.clone()).unwrap();

        assert_eq!(
            fs::read(destination.join("jdk-25.0.5+8-jre/bin/java")).unwrap(),
            b"java runtime"
        );
        assert_eq!(
            fs::read(destination.join("jdk-25.0.5+8-jre/legal/jdk.unsupported/LICENSE")).unwrap(),
            b"license text"
        );
    }

    #[test]
    fn runtime_extractor_rejects_symlink_escape() {
        let (root, _cleanup) = test_directory();
        let archive_path = root.join("runtime-escape.tar.gz");
        let destination = root.join("runtime");
        let outside = root.join("outside");
        write_runtime_tar(&archive_path, "../../../../outside");

        assert!(unpack_runtime(&archive_path, destination).is_err());
        assert!(!outside.exists());
    }

    #[test]
    fn restore_copies_a_valid_world_without_changing_its_source_archive() {
        let (root, _cleanup) = test_directory();
        let source = root.join("source");
        fs::create_dir_all(source.join("world")).unwrap();
        fs::write(source.join("paper.jar"), b"server jar placeholder").unwrap();
        fs::write(source.join("world/level.dat"), b"world data").unwrap();

        let archive_path = root.join("world.tar.gz");
        let encoder = GzEncoder::new(File::create(&archive_path).unwrap(), Compression::default());
        let mut archive = Builder::new(encoder);
        archive.append_dir_all("server", &source).unwrap();
        archive.finish().unwrap();
        archive.into_inner().unwrap().finish().unwrap();
        let archive_hash = super::hash_file(&archive_path).unwrap();

        let destination = root.join("restored");
        let result = restore_backup(&archive_path, &destination, |_, _| {}).unwrap();
        let restored_world = Path::new(&result.path).join("world/level.dat");

        assert_eq!(fs::read(restored_world).unwrap(), b"world data");
        assert_eq!(
            fs::read(source.join("world/level.dat")).unwrap(),
            b"world data"
        );
        assert_eq!(super::hash_file(&archive_path).unwrap(), archive_hash);
    }
}
