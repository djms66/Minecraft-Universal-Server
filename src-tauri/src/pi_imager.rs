//! Verified Raspberry Pi Imager runtime and first-boot provisioning.
//!
//! The pinned official utility is downloaded on demand and checked against the
//! SHA-256 published by Raspberry Pi. Its own removable-drive guard and image
//! verification remain enabled for every write.
//! L’outil officiel épinglé est téléchargé à la demande et comparé à l’empreinte
//! SHA-256 publiée par Raspberry Pi. Sa protection des supports amovibles et la
//! vérification de l’image restent actives pour chaque écriture.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const IMAGER_VERSION: &str = "2.0.12";
const MAX_IMAGER_WRITE_TIME: Duration = Duration::from_secs(45 * 60);
const IMAGER_MAC_URL: &str =
    "https://github.com/raspberrypi/rpi-imager/releases/download/v2.0.12/rpi-imager-v2.0.12.dmg";
const IMAGER_MAC_SHA256: &str = "15b1a94c712c0032750855653c2bf129dceb401329fbce0a83d7cf05dc58a421";
const IMAGER_WINDOWS_URL: &str =
    "https://github.com/raspberrypi/rpi-imager/releases/download/v2.0.12/imager-v2.0.12.exe";
const IMAGER_WINDOWS_SHA256: &str =
    "98bff90afe23339d0768915f6f68d35ac254740ec1eac2082ebeaa9ba41bc8e3";
const IMAGER_LINUX_X64_URL: &str = "https://github.com/raspberrypi/rpi-imager/releases/download/v2.0.12/Raspberry_Pi_Imager-v2.0.12-cli-x86_64.AppImage";
const IMAGER_LINUX_X64_SHA256: &str =
    "8a2fa9c0fe6c61f57d5826a7419131b18f33226c681c740ebc30a713928d05e3";
const IMAGER_LINUX_ARM64_URL: &str = "https://github.com/raspberrypi/rpi-imager/releases/download/v2.0.12/Raspberry_Pi_Imager-v2.0.12-cli-aarch64.AppImage";
const IMAGER_LINUX_ARM64_SHA256: &str =
    "b87c82db68fbe8f4fd7c266af8a5d7b2aad3d0ddc2e4243ebd6ea77bcb65573c";
const IMAGER_LINUX_ARMHF_URL: &str = "https://github.com/raspberrypi/rpi-imager/releases/download/v2.0.12/Raspberry_Pi_Imager-v2.0.12-cli-armhf.AppImage";
const IMAGER_LINUX_ARMHF_SHA256: &str =
    "dcf25bcbd83256e2b568d2fde8f994629937ebe254cfbf8ec7ad718491cff3bc";

#[derive(Clone, Deserialize, Serialize)]
pub struct PiFlashResult {
    pub detail: String,
    pub log_path: String,
    pub imager_version: String,
    pub hostname: String,
    #[serde(default)]
    pub target_id: String,
    #[serde(default)]
    pub target_name: String,
    #[serde(default)]
    pub target_size_bytes: u64,
    #[serde(default)]
    pub target_model: String,
    #[serde(default)]
    pub expected_devices: Vec<String>,
}

/// Restores the latest completed first-boot card session after an application
/// restart. Only the newest log is considered, so an older prepared card can
/// never silently replace a newer failed or incomplete attempt.
/// Restaure la session de préparation de carte la plus récente après le redémarrage
/// de l’application. Seul le journal récent est pris en compte afin qu’une ancienne
/// carte ne remplace pas silencieusement une tentative plus récente échouée.
pub fn recover_latest_prepared_pi(app_data: &Path) -> Result<Option<PiFlashResult>, String> {
    let root = app_data.join("pi-first-boot");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(error)),
    };

    let cleared_at = fs::metadata(root.join("prepared-session-cleared"))
        .and_then(|metadata| metadata.modified())
        .ok();
    let mut latest: Option<(std::time::SystemTime, PathBuf, String)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(run_id) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if run_id.len() != 32 || !run_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let log_path = path.join("ecriture.log");
        let Ok(metadata) = fs::metadata(&log_path) else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if cleared_at.is_some_and(|cleared| modified <= cleared) {
            continue;
        }
        if latest
            .as_ref()
            .is_some_and(|(seen, _, _)| modified <= *seen)
        {
            continue;
        }
        let Ok(log) = fs::read_to_string(&log_path) else {
            continue;
        };
        let hostname = format!("monde-ensemble-{run_id}");
        let lowered = log.to_ascii_lowercase();
        let completed = lowered.contains("write complete")
            && lowered.contains("verification complete")
            && log.contains(&hostname);
        latest = Some((
            modified,
            log_path,
            if completed { hostname } else { String::new() },
        ));
    }

    let Some((_, log_path, hostname)) = latest else {
        return Ok(None);
    };
    if hostname.is_empty() {
        return Ok(None);
    }

    Ok(Some(PiFlashResult {
        detail:
            "Carte Raspberry Pi écrite et vérifiée. Reprends l’installation sans réécrire la carte."
                .into(),
        log_path: log_path.display().to_string(),
        imager_version: IMAGER_VERSION.into(),
        hostname,
        target_id: String::new(),
        target_name: String::new(),
        target_size_bytes: 0,
        target_model: String::new(),
        // The prepared image is app-owned and was validated against the
        // official Pi OS catalogue before writing. Hardware is checked again
        // against the Pi's reported model before any server files are copied.
        // L’image préparée a été validée dans le catalogue officiel avant écriture.
        // Vérifier le modèle réel du Pi avant d’y transférer les fichiers serveur.
        expected_devices: Vec::new(),
    }))
}

/// Records that the user deliberately left a restored card-preparation flow.
/// Keep the diagnostic logs; this marker only prevents an old successful log
/// from trapping the next launch in the obsolete "card ready" screen.
/// Enregistre que l’utilisateur a volontairement quitté la préparation restaurée.
/// Conserver les journaux ; ce marqueur évite qu’un ancien succès bloque le prochain
/// lancement sur l’écran obsolète de carte prête.
pub fn clear_prepared_pi_recovery(app_data: &Path) -> Result<(), String> {
    let root = app_data.join("pi-first-boot");
    fs::create_dir_all(&root).map_err(io_error)?;
    fs::write(
        root.join("prepared-session-cleared"),
        chrono::Utc::now().to_rfc3339(),
    )
    .map_err(io_error)
}

struct MountedImager {
    executable: PathBuf,
    mountpoint: Option<PathBuf>,
}

/// Confirms the pinned macOS Imager can launch before the user is asked to
/// approve erasing a removable drive. Dropping the handle also releases any
/// temporary mount created for this check.
/// Vérifie que la version épinglée de Raspberry Pi Imager démarre avant de demander
/// l’effacement d’un support amovible. La fermeture du handle libère tout montage temporaire.
pub fn check_macos_writer(app_data: &Path) -> Result<String, String> {
    if std::env::consts::OS != "macos" {
        return Err("La vérification préalable est disponible sur macOS uniquement.".into());
    }
    let tool = ensure_macos_imager(app_data, &|_, _| {})?;
    let output = Command::new(&tool.executable)
        .args(["--cli", "--version"])
        .output()
        .map_err(|_| "Raspberry Pi Imager est introuvable ou ne peut pas démarrer. Aucun support n’a été modifié.".to_string())?;
    let version = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() || !version.contains(IMAGER_VERSION) {
        return Err("La version vérifiée de Raspberry Pi Imager n’a pas démarré correctement. Aucun support n’a été modifié.".into());
    }
    Ok(format!("Raspberry Pi Imager {IMAGER_VERSION} est prêt."))
}

struct SensitiveConfigFiles(PathBuf, PathBuf);

impl Drop for SensitiveConfigFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        let _ = fs::remove_file(&self.1);
    }
}

impl Drop for MountedImager {
    fn drop(&mut self) {
        if let Some(mountpoint) = &self.mountpoint {
            let _ = Command::new("hdiutil")
                .args(["detach", "-quiet"])
                .arg(mountpoint)
                .status();
            let _ = fs::remove_dir(mountpoint);
        }
    }
}

/// Writes the current official 64-bit Lite image and provisions SSH plus
/// DHCP networking. The caller must revalidate the selected removable disk
/// and obtain explicit erase confirmation immediately before calling here.
/// Écrit l’image Lite officielle 64 bits et configure SSH ainsi que le réseau DHCP.
/// L’appelant revérifie le disque amovible et demande la confirmation d’effacement
/// immédiatement avant cet appel.
pub fn flash_pi_os(
    app_data: &Path,
    target_device: &str,
    image_name: &str,
    image_url: &str,
    image_sha256: &str,
    download_sha256: Option<&str>,
    wifi_ssid: &str,
    wifi_password: &str,
    wifi_country: &str,
    progress: impl Fn(&str, u8),
    verify_target: impl Fn() -> Result<(), String>,
    cancelled: Arc<AtomicBool>,
) -> Result<PiFlashResult, String> {
    // The caller first validates this exact URL against the official Imager
    // catalog; keep HTTPS and SHA-256 checks here as defense in depth.
    // L’appelant valide cette URL exacte dans le catalogue officiel ; garder aussi
    // les contrôles HTTPS et SHA-256 ici comme protection supplémentaire.
    if !image_url.starts_with("https://")
        || !valid_sha256(image_sha256)
        || download_sha256.is_some_and(|hash| !valid_sha256(hash))
    {
        return Err("Le catalogue officiel n’a pas fourni une image vérifiable. Aucun support n’a été modifié.".into());
    }
    validate_wifi(wifi_ssid, wifi_password, wifi_country)?;

    progress("Préparation de l’outil officiel d’écriture…", 8);
    let tool = ensure_imager(app_data, &progress)?;
    let version_check = Command::new(&tool.executable)
        .args(["--cli", "--version"])
        .output()
        .map_err(|_| {
            "L’outil officiel Raspberry Pi Imager ne peut pas démarrer sur cet appareil."
                .to_string()
        })?;
    if !version_check.status.success()
        || !String::from_utf8_lossy(&version_check.stdout).contains(IMAGER_VERSION)
            && !String::from_utf8_lossy(&version_check.stderr).contains(IMAGER_VERSION)
    {
        return Err("La version de l’outil officiel Raspberry Pi Imager n’est pas celle qui a été vérifiée. Aucun support n’a été modifié.".into());
    }

    let image_path = download_pi_image(
        app_data,
        image_url,
        download_sha256,
        image_sha256,
        &progress,
    )?;
    // Downloading can take several minutes. Recheck the exact removable disk
    // after it finishes and immediately before the official writer opens it.
    // Le téléchargement peut prendre plusieurs minutes. Revérifier le disque choisi
    // après le téléchargement et juste avant que l’outil officiel l’ouvre.
    verify_target()?;

    let ssh_key = ensure_ssh_public_key(app_data)?;

    let run_id = uuid::Uuid::new_v4().simple().to_string();
    let hostname = format!("monde-ensemble-{run_id}");
    let config_dir = app_data.join("pi-first-boot").join(&run_id);
    fs::create_dir_all(&config_dir).map_err(io_error)?;
    let user_data_path = config_dir.join("user-data");
    let network_config_path = config_dir.join("network-config");
    let _sensitive_files =
        SensitiveConfigFiles(user_data_path.clone(), network_config_path.clone());
    let log_path = config_dir.join("ecriture.log");
    let user_data_contents = user_data(&ssh_key, &hostname);
    let network_config_contents = network_config(wifi_ssid, wifi_password, wifi_country);
    fs::write(&user_data_path, &user_data_contents).map_err(io_error)?;
    fs::write(&network_config_path, &network_config_contents).map_err(io_error)?;
    restrict_file_permissions(&user_data_path)?;
    restrict_file_permissions(&network_config_path)?;
    let _ = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(io_error)?;

    let args = imager_cli_args(
        image_sha256,
        &user_data_path,
        &network_config_path,
        &image_path,
        target_device,
    );
    let redactions = sensitive_log_values(
        wifi_ssid,
        wifi_password,
        &user_data_contents,
        &network_config_contents,
    );
    progress("Écriture et vérification de la carte…", 38);
    run_imager(&tool, &args, &log_path, &redactions, &progress, &cancelled)?;

    // Sensitive cloud-init files are removed by the guard on success or error.
    // Les fichiers cloud-init sensibles sont supprimés par la garde en cas de succès
    // comme d’erreur.
    progress("Le système Raspberry Pi est écrit et vérifié.", 100);
    Ok(PiFlashResult {
        detail: format!("{image_name} écrit et vérifié sur {target_device}."),
        log_path: log_path.display().to_string(),
        imager_version: IMAGER_VERSION.into(),
        hostname,
        target_id: String::new(),
        target_name: String::new(),
        target_size_bytes: 0,
        target_model: String::new(),
        expected_devices: Vec::new(),
    })
}

fn imager_cli_args(
    image_sha256: &str,
    user_data_path: &Path,
    network_config_path: &Path,
    image_path: &Path,
    target_device: &str,
) -> Vec<String> {
    [
        "--cli".to_string(),
        "--debug".to_string(),
        "--sha256".to_string(),
        image_sha256.to_string(),
        "--cloudinit-userdata".to_string(),
        user_data_path.display().to_string(),
        "--cloudinit-networkconfig".to_string(),
        network_config_path.display().to_string(),
        image_path.display().to_string(),
        target_device.to_string(),
    ]
    .into()
}

/// Downloads Raspberry Pi OS before unmounting the card. When the manifest
/// includes a compressed-file hash, it is checked here. Otherwise Imager
/// verifies the extracted image against `image_sha256` while writing it.
/// Télécharge Raspberry Pi OS avant de démonter la carte. Une empreinte du fichier
/// compressé est vérifiée si le manifeste en fournit une ; sinon Imager compare
/// l’image extraite à image_sha256 pendant l’écriture.
pub fn download_pi_image(
    app_data: &Path,
    image_url: &str,
    download_sha256: Option<&str>,
    image_sha256: &str,
    progress: &impl Fn(&str, u8),
) -> Result<PathBuf, String> {
    if !image_url.starts_with("https://downloads.raspberrypi.com/")
        || !valid_sha256(image_sha256)
        || download_sha256.is_some_and(|hash| !valid_sha256(hash))
    {
        return Err("Le catalogue officiel n’a pas fourni une image vérifiable. Aucun support n’a été modifié.".into());
    }

    let directory = app_data.join("pi-images");
    fs::create_dir_all(&directory).map_err(io_error)?;
    let cache_key = download_sha256.unwrap_or(image_sha256);
    let image_path = directory.join(format!("{cache_key}.img.xz"));
    if let Some(expected_sha256) = download_sha256 {
        if image_path.is_file() && sha256_file(&image_path).as_deref() == Some(expected_sha256) {
            progress("Image du système déjà téléchargée et vérifiée.", 38);
            return Ok(image_path);
        }
    }
    if image_path.exists() {
        fs::remove_file(&image_path).map_err(io_error)?;
    }

    progress("Téléchargement de l’image Raspberry Pi OS…", 18);
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("MondeEnsemble/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(900))
        .build()
        .map_err(|error| format!("Connexion au téléchargement officiel impossible : {error}"))?;
    let mut response = client
        .get(image_url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| {
            format!("Téléchargement de Raspberry Pi OS impossible : {error}. La carte n’a pas été modifiée.")
        })?;
    let total_bytes = response.content_length().ok_or_else(|| {
        "Le serveur Raspberry Pi n’a pas indiqué la taille de l’image. La carte n’a pas été modifiée.".to_string()
    })?;
    if total_bytes == 0 {
        return Err("Le téléchargement officiel est vide. La carte n’a pas été modifiée.".into());
    }
    let available_bytes = fs2::available_space(&directory).map_err(io_error)?;
    let required_bytes = total_bytes.saturating_add(64 * 1024 * 1024);
    if available_bytes < required_bytes {
        return Err(format!(
            "Il n’y a pas assez d’espace libre sur cet ordinateur pour télécharger l’image ({} Mo requis). Libère de l’espace puis réessaie. La carte n’a pas été modifiée.",
            required_bytes.div_ceil(1_000_000)
        ));
    }

    let temporary = directory.join(format!("{}.download", uuid::Uuid::new_v4().simple()));
    let _temporary_file = RemoveFileOnDrop(temporary.clone());
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 256 * 1024];
    let mut downloaded_bytes = 0u64;
    let mut last_percent = 18u8;
    loop {
        let count = response.read(&mut buffer).map_err(|error| {
            format!("Téléchargement interrompu : {error}. La carte n’a pas été modifiée.")
        })?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count]).map_err(io_error)?;
        hasher.update(&buffer[..count]);
        downloaded_bytes = downloaded_bytes.saturating_add(count as u64);
        if downloaded_bytes > total_bytes {
            return Err("Le serveur a transmis une image plus grande que prévu. La carte n’a pas été modifiée.".into());
        }
        let percent = 18 + (downloaded_bytes.saturating_mul(20) / total_bytes) as u8;
        if percent > last_percent {
            last_percent = percent.min(38);
            progress("Téléchargement de l’image Raspberry Pi OS…", last_percent);
        }
    }
    if downloaded_bytes != total_bytes {
        return Err(format!(
            "Le téléchargement s’est terminé trop tôt ({} sur {} Mo). La carte n’a pas été modifiée.",
            downloaded_bytes / 1_000_000,
            total_bytes / 1_000_000
        ));
    }
    output.sync_all().map_err(io_error)?;
    drop(output);
    if let Some(expected_sha256) = download_sha256 {
        let actual_sha256 = format!("{:x}", hasher.finalize());
        if actual_sha256 != expected_sha256 {
            return Err("L’empreinte du téléchargement ne correspond pas au catalogue officiel. La carte n’a pas été modifiée.".into());
        }
    }
    fs::rename(&temporary, &image_path).map_err(io_error)?;
    progress(
        if download_sha256.is_some() {
            "Image Raspberry Pi OS téléchargée et vérifiée."
        } else {
            "Téléchargement terminé. L’image sera vérifiée par Raspberry Pi Imager."
        },
        38,
    );
    Ok(image_path)
}

struct RemoveFileOnDrop(PathBuf);

impl Drop for RemoveFileOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn sensitive_log_values(
    wifi_ssid: &str,
    wifi_password: &str,
    user_data: &str,
    network_config: &str,
) -> Vec<Vec<u8>> {
    let mut values = [wifi_password, user_data, network_config]
        .into_iter()
        .filter(|value| !value.is_empty())
        .map(|value| value.as_bytes().to_vec())
        .collect::<Vec<_>>();

    // Debug logs may print these cloud-init values with JSON escaping. Keep
    // both the entered text and escaped forms out of the persistent log.
    // Les journaux de débogage peuvent afficher ces valeurs cloud-init en JSON.
    // Exclure du journal persistant le texte saisi et ses formes échappées.
    for value in [wifi_ssid, wifi_password] {
        if !value.is_empty() {
            if let Ok(encoded) = serde_json::to_string(value) {
                values.push(encoded.as_bytes().to_vec());
                if let Ok(double_encoded) = serde_json::to_string(&encoded) {
                    values.push(double_encoded.as_bytes().to_vec());
                }
            }
        }
    }
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    values
}

fn ensure_imager(app_data: &Path, progress: &impl Fn(&str, u8)) -> Result<MountedImager, String> {
    match std::env::consts::OS {
        "macos" => ensure_macos_imager(app_data, progress),
        "windows" => ensure_windows_imager(app_data, progress),
        "linux" => ensure_linux_imager(app_data, progress),
        _ => Err(
            "La préparation automatique du Raspberry Pi n’est pas disponible sur ce système."
                .into(),
        ),
    }
}

fn ensure_macos_imager(
    app_data: &Path,
    progress: &impl Fn(&str, u8),
) -> Result<MountedImager, String> {
    let directory = app_data.join("tools");
    fs::create_dir_all(&directory).map_err(io_error)?;
    if let Some(executable) = find_existing_macos_imager(app_data) {
        progress("Raspberry Pi Imager est déjà prêt.", 8);
        return Ok(MountedImager {
            executable,
            mountpoint: None,
        });
    }
    let dmg = verified_asset(
        &directory.join("rpi-imager-v2.0.12.dmg"),
        IMAGER_MAC_URL,
        IMAGER_MAC_SHA256,
        progress,
    )?;
    let mountpoint = directory.join(format!("imager-mount-{}", uuid::Uuid::new_v4().simple()));
    fs::create_dir(&mountpoint).map_err(io_error)?;
    let attached = Command::new("hdiutil")
        .args(["attach", "-quiet", "-readonly", "-nobrowse", "-mountpoint"])
        .arg(&mountpoint)
        .arg(&dmg)
        .output()
        .map_err(|_| "macOS n’a pas pu ouvrir l’outil officiel Raspberry Pi Imager.".to_string())?;
    let executable = mountpoint.join("Raspberry Pi Imager.app/Contents/MacOS/rpi-imager");
    if !attached.status.success() {
        if executable.is_file() && is_expected_imager(&executable) {
            return Ok(MountedImager {
                executable,
                mountpoint: Some(mountpoint),
            });
        }
        let _ = fs::remove_dir(&mountpoint);
        if is_resource_busy(&attached.stderr) {
            if let Some(executable) = find_existing_macos_imager(app_data) {
                return Ok(MountedImager {
                    executable,
                    mountpoint: None,
                });
            }
            return Err("Raspberry Pi Imager est déjà ouvert depuis une image disque, mais son emplacement n’a pas pu être retrouvé. Ferme cette image puis réessaie. Aucun support n’a été modifié.".into());
        }
        let details = String::from_utf8_lossy(&attached.stderr).trim().to_string();
        return Err(if details.is_empty() {
            "macOS n’a pas pu ouvrir l’outil officiel Raspberry Pi Imager. Aucun support n’a été modifié.".into()
        } else {
            format!(
                "macOS n’a pas pu ouvrir Raspberry Pi Imager : {details}. Aucun support n’a été modifié."
            )
        });
    }
    if !executable.is_file() {
        let _ = Command::new("hdiutil")
            .args(["detach", "-quiet"])
            .arg(&mountpoint)
            .status();
        let _ = fs::remove_dir(&mountpoint);
        return Err(
            "Le paquet officiel Raspberry Pi Imager ne contient pas le programme attendu.".into(),
        );
    }
    Ok(MountedImager {
        executable,
        mountpoint: Some(mountpoint),
    })
}

fn find_existing_macos_imager(app_data: &Path) -> Option<PathBuf> {
    const APP_BUNDLE: &str = "Raspberry Pi Imager.app";

    let mut search_roots = vec![PathBuf::from("/Applications"), PathBuf::from("/Volumes")];
    if let Some(home) = std::env::var_os("HOME") {
        search_roots.push(PathBuf::from(home).join("Applications"));
    }
    search_roots.push(app_data.join("tools"));

    for root in search_roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let app = if path.file_name().is_some_and(|name| name == APP_BUNDLE) {
                path
            } else {
                path.join(APP_BUNDLE)
            };
            let executable = app.join("Contents/MacOS/rpi-imager");
            if executable.is_file() && is_expected_imager(&executable) {
                return Some(executable);
            }
        }
    }
    None
}

fn is_expected_imager(executable: &Path) -> bool {
    let Some(app_bundle) = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return false;
    };
    let signature_valid = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(app_bundle)
        .status()
        .is_ok_and(|status| status.success());
    if !signature_valid {
        return false;
    }
    let signer = Command::new("codesign")
        .args(["-dv", "--verbose=4"])
        .arg(app_bundle)
        .output();
    let signer_matches = signer.is_ok_and(|output| {
        let details = String::from_utf8_lossy(&output.stderr);
        details.contains("Identifier=com.raspberrypi.rpi-imager")
            && details.contains("TeamIdentifier=8RDZTRXE62")
    });
    if !signer_matches {
        return false;
    }
    Command::new(executable)
        .args(["--cli", "--version"])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && (String::from_utf8_lossy(&output.stdout).contains(IMAGER_VERSION)
                    || String::from_utf8_lossy(&output.stderr).contains(IMAGER_VERSION))
        })
}

fn is_resource_busy(stderr: &[u8]) -> bool {
    let message = String::from_utf8_lossy(stderr).to_lowercase();
    message.contains("resource busy") || message.contains("ressource occupée")
}

fn ensure_windows_imager(
    app_data: &Path,
    progress: &impl Fn(&str, u8),
) -> Result<MountedImager, String> {
    let program_files = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    let install_dir = program_files.join("Raspberry Pi Ltd/Imager");
    let executable = install_dir.join("rpi-imager.exe");
    if executable.is_file() {
        return Ok(MountedImager {
            executable,
            mountpoint: None,
        });
    }

    let directory = app_data.join("tools");
    fs::create_dir_all(&directory).map_err(io_error)?;
    let installer = verified_asset(
        &directory.join("imager-v2.0.12.exe"),
        IMAGER_WINDOWS_URL,
        IMAGER_WINDOWS_SHA256,
        progress,
    )?;
    progress(
        "Installation silencieuse du composant officiel Raspberry Pi…",
        14,
    );
    let script = format!(
        "$ErrorActionPreference='Stop'; $p=Start-Process -FilePath {} -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /DIR=\"{}\"' -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
        powershell_quote(&installer.display().to_string()),
        install_dir.display().to_string().replace('\'', "''")
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|_| {
            "Windows n’a pas pu lancer l’installation sécurisée de l’outil Raspberry Pi."
                .to_string()
        })?;
    if !output.status.success() || !executable.is_file() {
        return Err("L’installation de l’outil officiel Raspberry Pi n’a pas abouti. Aucun support n’a été modifié.".into());
    }
    Ok(MountedImager {
        executable,
        mountpoint: None,
    })
}

fn ensure_linux_imager(
    app_data: &Path,
    progress: &impl Fn(&str, u8),
) -> Result<MountedImager, String> {
    let (url, digest, filename) = match std::env::consts::ARCH {
        "x86_64" => (IMAGER_LINUX_X64_URL, IMAGER_LINUX_X64_SHA256, "imager-cli-x86_64.AppImage"),
        "aarch64" => (IMAGER_LINUX_ARM64_URL, IMAGER_LINUX_ARM64_SHA256, "imager-cli-aarch64.AppImage"),
        "arm" | "armv7" => (IMAGER_LINUX_ARMHF_URL, IMAGER_LINUX_ARMHF_SHA256, "imager-cli-armhf.AppImage"),
        _ => return Err("L’outil Raspberry Pi officiel ne publie pas de version pour l’architecture de cet ordinateur.".into()),
    };
    let directory = app_data.join("tools");
    fs::create_dir_all(&directory).map_err(io_error)?;
    let executable = verified_asset(&directory.join(filename), url, digest, progress)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
    }
    Ok(MountedImager {
        executable,
        mountpoint: None,
    })
}

fn verified_asset(
    path: &Path,
    url: &str,
    expected_sha256: &str,
    progress: &impl Fn(&str, u8),
) -> Result<PathBuf, String> {
    if path.is_file() && sha256_file(path).as_deref() == Some(expected_sha256) {
        return Ok(path.to_path_buf());
    }
    if path.exists() {
        fs::remove_file(path).map_err(io_error)?;
    }
    progress("Téléchargement de l’outil officiel Raspberry Pi…", 10);
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("MondeEnsemble/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| format!("Connexion impossible au téléchargement officiel : {error}"))?;
    let mut response = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("Téléchargement de l’outil officiel impossible : {error}"))?;
    let temporary = path.with_extension(format!("{}.download", uuid::Uuid::new_v4().simple()));
    let mut output = File::create(&temporary).map_err(io_error)?;
    io::copy(&mut response, &mut output).map_err(io_error)?;
    output.sync_all().map_err(io_error)?;
    drop(output);
    if sha256_file(&temporary).as_deref() != Some(expected_sha256) {
        let _ = fs::remove_file(&temporary);
        return Err("L’empreinte du téléchargement Raspberry Pi ne correspond pas à la publication officielle. Aucun support n’a été modifié.".into());
    }
    fs::rename(&temporary, path).map_err(io_error)?;
    Ok(path.to_path_buf())
}

fn run_imager(
    tool: &MountedImager,
    args: &[String],
    log_path: &Path,
    redactions: &[Vec<u8>],
    progress: &impl Fn(&str, u8),
    cancelled: &AtomicBool,
) -> Result<(), String> {
    if std::env::consts::OS == "macos" {
        let target_device = args.last().ok_or_else(|| {
            "Le support cible n’est pas indiqué. Aucun support n’a été modifié.".to_string()
        })?;
        if macos_imager_targets_device(target_device)? {
            return Err("Une autre écriture Raspberry Pi Imager utilise déjà cette carte. Attends qu’elle se termine avant de réessayer. Aucun nouveau processus d’écriture n’a été lancé.".into());
        }
    }
    let mut child = match std::env::consts::OS {
        "macos" => spawn_macos_imager(tool, args)?,
        "windows" => spawn_windows_elevated(tool, args)?,
        "linux" => spawn_linux_elevated(tool, args)?,
        _ => return Err("L’écriture sécurisée n’est pas disponible sur ce système.".into()),
    };
    // The elevated process only writes to its inherited pipes. The ordinary
    // app process owns the log file and copies those bytes, so elevation never
    // follows a user-controlled log path.
    // Le processus élevé n’écrit que dans ses canaux hérités. Le processus normal
    // possède le journal et copie ces octets ; l’élévation ne suit donc aucun chemin
    // de journal contrôlé par l’utilisateur.
    let log = Arc::new(Mutex::new(
        OpenOptions::new()
            .append(true)
            .open(log_path)
            .map_err(io_error)?,
    ));
    let stdout_thread = child
        .stdout
        .take()
        .map(|pipe| copy_pipe_to_log(pipe, Arc::clone(&log), redactions.to_vec()));
    let stderr_thread = child
        .stderr
        .take()
        .map(|pipe| copy_pipe_to_log(pipe, Arc::clone(&log), redactions.to_vec()));
    let mut last_percent = 18u8;
    let started_at = Instant::now();
    loop {
        update_write_progress(log_path, &mut last_percent, progress);
        // The macOS Imager process is a direct child of the app. On Windows,
        // the elevated writer is spawned behind Start-Process and cannot be
        // safely stopped by killing its PowerShell parent.
        // Sur macOS, Imager est un enfant direct de l’application. Sous Windows, le
        // programme élevé est lancé par Start-Process et ne peut pas être arrêté en
        // tuant de façon sûre son processus PowerShell parent.
        if std::env::consts::OS == "macos" && cancelled.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(thread) = stdout_thread {
                let _ = thread.join();
            }
            if let Some(thread) = stderr_thread {
                let _ = thread.join();
            }
            return Err("Écriture interrompue. La carte peut être incomplète : laisse-la connectée, puis utilise la récupération de carte avant de recommencer.".into());
        }
        if std::env::consts::OS == "macos" && started_at.elapsed() >= MAX_IMAGER_WRITE_TIME {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(thread) = stdout_thread {
                let _ = thread.join();
            }
            if let Some(thread) = stderr_thread {
                let _ = thread.join();
            }
            return Err("L’écriture a dépassé 45 minutes et a été arrêtée pour rendre la main. La carte peut être incomplète : laisse-la connectée, puis utilise la récupération de carte avant de recommencer.".into());
        }
        match child.try_wait().map_err(io_error)? {
            Some(status) => {
                if let Some(thread) = stdout_thread {
                    let _ = thread.join();
                }
                if let Some(thread) = stderr_thread {
                    let _ = thread.join();
                }
                if status.success() {
                    return Ok(());
                }
                let detail = fs::read_to_string(log_path).unwrap_or_default();
                let excerpt = detail
                    .lines()
                    .rev()
                    .take(8)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(if excerpt.trim().is_empty() {
                    format!(
                        "Raspberry Pi Imager n’a pas pu écrire le support. Consulte le journal : {}",
                        log_path.display()
                    )
                } else {
                    format!(
                        "Raspberry Pi Imager n’a pas pu écrire le support : {excerpt}\nJournal complet : {}",
                        log_path.display()
                    )
                });
            }
            None => thread::sleep(Duration::from_millis(500)),
        }
    }
}

fn macos_imager_targets_device(target_device: &str) -> Result<bool, String> {
    let output = Command::new("ps")
        .args(["-axo", "command="])
        .output()
        .map_err(|_| "macOS n’a pas pu vérifier si une écriture utilise déjà cette carte. Aucun support n’a été modifié.".to_string())?;
    if !output.status.success() {
        return Err("macOS n’a pas pu vérifier si une écriture utilise déjà cette carte. Aucun support n’a été modifié.".into());
    }
    let processes = String::from_utf8_lossy(&output.stdout);
    Ok(processes
        .lines()
        .any(|line| process_targets_imager_device(line, target_device)))
}

fn process_targets_imager_device(command_line: &str, target_device: &str) -> bool {
    let command_line = command_line.trim();
    let is_imager = command_line.contains("/rpi-imager ") || command_line.ends_with("/rpi-imager");
    is_imager && command_line.split_whitespace().last() == Some(target_device)
}

fn copy_pipe_to_log(
    mut pipe: impl Read + Send + 'static,
    log: Arc<Mutex<File>>,
    redactions: Vec<Vec<u8>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        let mut pending = Vec::new();
        let max_secret_len = redactions.iter().map(Vec::len).max().unwrap_or(1);
        loop {
            let Ok(count) = pipe.read(&mut buffer) else {
                break;
            };
            if count == 0 {
                break;
            }
            pending.extend_from_slice(&buffer[..count]);
            let safe_len = redacted_prefix_len(&pending, max_secret_len, &redactions);
            if safe_len > 0 {
                let safe = pending.drain(..safe_len).collect::<Vec<_>>();
                if write_redacted(&safe, &redactions, &log).is_err() {
                    return;
                }
            }
        }
        if !pending.is_empty() {
            if write_redacted(&pending, &redactions, &log).is_err() {
                return;
            }
        }
    })
}

fn redacted_prefix_len(input: &[u8], max_secret_len: usize, redactions: &[Vec<u8>]) -> usize {
    let mut safe_len = input.len().saturating_sub(max_secret_len.saturating_sub(1));
    for secret in redactions.iter().filter(|secret| !secret.is_empty()) {
        let mut offset = 0;
        while let Some(found) = find_bytes(&input[offset..], secret) {
            let start = offset + found;
            let end = start + secret.len();
            if start < safe_len && end > safe_len {
                safe_len = start;
            }
            offset = start + 1;
            if offset >= input.len() {
                break;
            }
        }
    }
    safe_len
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn write_redacted(input: &[u8], redactions: &[Vec<u8>], log: &Arc<Mutex<File>>) -> io::Result<()> {
    let mut output = input.to_vec();
    for secret in redactions.iter().filter(|secret| !secret.is_empty()) {
        let mut redacted = Vec::with_capacity(output.len());
        let mut remaining = output.as_slice();
        while let Some(found) = find_bytes(remaining, secret) {
            redacted.extend_from_slice(&remaining[..found]);
            redacted.extend_from_slice(b"[confidential redacted]");
            remaining = &remaining[found + secret.len()..];
        }
        redacted.extend_from_slice(remaining);
        output = redacted;
    }
    let mut file = log
        .lock()
        .map_err(|_| io::Error::other("log lock poisoned"))?;
    use std::io::Write;
    file.write_all(&output)
}

/// Runs Imager as the signed-in user on macOS. Imager opens removable media
/// through macOS authorization itself; wrapping the whole process in
/// AppleScript elevation can stall device setup and buffers output until exit.
/// Lance Imager avec le compte connecté sur macOS. L’outil ouvre les supports avec
/// l’autorisation macOS ; l’élévation AppleScript peut bloquer la détection et retenir
/// la sortie jusqu’à la fin du processus.
fn spawn_macos_imager(tool: &MountedImager, args: &[String]) -> Result<Child, String> {
    Command::new(&tool.executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| {
            "macOS n’a pas pu démarrer Raspberry Pi Imager. Aucun support n’a été modifié.".into()
        })
}

fn spawn_windows_elevated(tool: &MountedImager, args: &[String]) -> Result<Child, String> {
    let arguments = args
        .iter()
        .map(|arg| windows_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        "$ErrorActionPreference='Stop'; $p=Start-Process -FilePath {} -ArgumentList {} -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
        powershell_quote(&tool.executable.display().to_string()),
        powershell_quote(&arguments)
    );
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Windows n’a pas pu demander l’autorisation nécessaire à l’écriture.".into())
}

fn spawn_linux_elevated(tool: &MountedImager, args: &[String]) -> Result<Child, String> {
    let command = format!(
        "{} --appimage-extract-and-run {}",
        shell_quote(&tool.executable.display().to_string()),
        args.iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );
    Command::new("pkexec")
        .args(["/bin/sh", "-c", &command])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Linux n’a pas trouvé la fenêtre d’autorisation nécessaire à l’écriture (PolicyKit). Aucun support n’a été modifié.".into())
}

fn update_write_progress(log_path: &Path, last_percent: &mut u8, progress: &impl Fn(&str, u8)) {
    let Ok(log) = fs::read_to_string(log_path) else {
        return;
    };
    let Ok(pattern) = regex::Regex::new(r"(?i)(writing|verifying)[^\r\n]{0,120}?(\d{1,3})\s*%")
    else {
        return;
    };
    let mut target_write_complete = false;
    let mut latest_target_progress = None;
    for captures in pattern.captures_iter(&log) {
        let phase = captures.get(1).map(|m| m.as_str()).unwrap_or_default();
        let value = captures
            .get(2)
            .and_then(|m| m.as_str().parse::<u8>().ok())
            .unwrap_or_default()
            .min(100);
        if phase.eq_ignore_ascii_case("writing") {
            target_write_complete = value == 100;
            latest_target_progress = Some((false, value));
        } else if target_write_complete {
            // Raspberry Pi Imager also logs a "Verifying" phase while it
            // checks the downloaded image cache. Do not count that as SD
            // verification; only accept this phase after the card write hits
            // 100 percent.
            // Imager journalise également « Verifying » pendant le contrôle de son
            // cache. Ne pas confondre cela avec la vérification de la carte ; accepter
            // cette phase uniquement après 100 % de l’écriture.
            latest_target_progress = Some((true, value));
        }
    }
    if let Some((is_verifying_card, value)) = latest_target_progress {
        let mapped = if is_verifying_card {
            78 + value.saturating_mul(20) / 100
        } else {
            18 + value.saturating_mul(58) / 100
        };
        if mapped > *last_percent {
            *last_percent = mapped;
            progress(
                if is_verifying_card {
                    "Vérification de la carte écrite…"
                } else {
                    "Écriture de Raspberry Pi OS sur la carte…"
                },
                mapped,
            );
        }
    }
}

fn validate_wifi(ssid: &str, password: &str, country: &str) -> Result<(), String> {
    if country.len() != 2 || !country.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err("Choisis un code pays Wi-Fi de deux lettres, par exemple FR.".into());
    }
    if ssid.is_empty() {
        if !password.is_empty() {
            return Err(
                "Le mot de passe Wi-Fi doit être vide si aucun nom de réseau n’est saisi.".into(),
            );
        }
        return Ok(());
    }
    if ssid.as_bytes().len() > 32 || ssid.contains(['\n', '\r', '\0']) {
        return Err("Le nom du réseau Wi-Fi doit contenir au plus 32 caractères.".into());
    }
    let raw_key = password.len() == 64 && password.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !raw_key && !(8..=63).contains(&password.len())
        || password
            .chars()
            .any(|character| !character.is_ascii_graphic() && character != ' ')
    {
        return Err("Le mot de passe Wi-Fi doit contenir 8 à 63 caractères imprimables.".into());
    }
    Ok(())
}

fn network_config(ssid: &str, password: &str, country: &str) -> String {
    // Raspberry Pi Imager's cloud-init network-config follows the official
    // Netplan v2 shape. Keep it aligned with Imager's own generator instead
    // of forcing a renderer that Raspberry Pi OS selects for itself.
    // La configuration réseau cloud-init suit le format Netplan v2 officiel.
    // Laissons Raspberry Pi OS choisir son moteur réseau, comme le fait Imager.
    let mut config = String::from(
        "network:\n  version: 2\n  ethernets:\n    eth0:\n      dhcp4: true\n      dhcp6: true\n      optional: true\n",
    );
    if !ssid.is_empty() {
        let ssid = serde_json::to_string(ssid).unwrap_or_else(|_| "\"\"".into());
        let password = serde_json::to_string(password).unwrap_or_else(|_| "\"\"".into());
        config.push_str(&format!(
            "  wifis:\n    wlan0:\n      dhcp4: true\n      regulatory-domain: \"{}\"\n      access-points:\n        {}:\n          password: {}\n      optional: true\n",
            country.to_ascii_uppercase(), ssid, password,
        ));
    }
    config
}

fn user_data(public_key: &str, hostname: &str) -> String {
    let key = serde_json::to_string(public_key.trim()).unwrap_or_else(|_| "\"\"".into());
    // Raspberry Pi OS cloud-init merges `user:` over the distro's default
    // account. Keep its standard memberships while explicitly suppressing the
    // image's passwordless sudo policy; this account has no password and only
    // needs SSH access to run the server as an unprivileged user.
    // Raspberry Pi OS fusionne user: avec le compte par défaut. Garder ses groupes
    // habituels tout en désactivant explicitement sudo sans mot de passe ; ce compte
    // sans mot de passe sert au SSH et au lancement du serveur sans privilèges.
    let cleanup = serde_json::to_string(
        r#"for d in /boot/firmware /boot /var/lib/cloud/instance /var/lib/cloud/instances; do find "$d" -maxdepth 2 -type f \( -name user-data -o -name network-config -o -name user-data.txt -o -name network-config.txt \) -delete 2>/dev/null || true; done"#,
    )
    .unwrap_or_else(|_| "\"true\"".into());
    format!(
        // Cloud-init parses this as YAML; these must be real line breaks, not
        // the two-character sequence `\\n` (which makes the whole file invalid).
        // Cloud-init lit ce contenu comme YAML : utiliser de vrais retours à la ligne,
        // et non les deux caractères barre oblique inverse et n, qui invalideraient
        // le fichier entier.
        "#cloud-config\nhostname: \"{hostname}\"\nmanage_etc_hosts: true\nuser:\n  name: \"minecraft\"\n  gecos: \"Monde Ensemble\"\n  shell: /bin/bash\n  lock_passwd: true\n  ssh_authorized_keys:\n    - {key}\n  sudo: null\nssh_pwauth: false\nruncmd:\n  - [systemctl, enable, --now, ssh]\n  - [bash, -lc, {cleanup}]\n"
    )
}

fn ensure_ssh_public_key(app_data: &Path) -> Result<String, String> {
    let ssh_dir = app_data.join("ssh");
    fs::create_dir_all(&ssh_dir).map_err(io_error)?;
    restrict_directory_permissions(&ssh_dir)?;
    let private_key = ssh_dir.join("monde-ensemble-pi");
    let public_key = ssh_dir.join("monde-ensemble-pi.pub");
    match (private_key.exists(), public_key.exists()) {
        (true, true) => {
            let content = fs::read_to_string(&public_key).map_err(io_error)?;
            let key = content
                .lines()
                .find(|line| valid_ssh_public_key(line))
                .ok_or_else(|| "La clé SSH privée de l’application existe, mais sa clé publique est illisible. Aucun fichier existant n’a été remplacé.".to_string())?;
            restrict_file_permissions(&private_key)?;
            return Ok(key.to_string());
        }
        (true, false) | (false, true) => {
            return Err("Le dossier SSH privé de l’application contient une clé incomplète. Aucun fichier existant n’a été remplacé.".into());
        }
        (false, false) => {}
    }
    let output = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "monde-ensemble", "-f"])
        .arg(&private_key)
        .output()
        .map_err(|_| "ssh-keygen n’est pas disponible. L’application ne peut pas préparer l’accès sécurisé au Pi.".to_string())?;
    if !output.status.success() {
        return Err("La clé d’accès SSH au Raspberry Pi n’a pas pu être créée. Aucune clé existante n’a été modifiée.".into());
    }
    restrict_file_permissions(&private_key)?;
    let content = fs::read_to_string(&public_key).map_err(io_error)?;
    content
        .lines()
        .find(|line| valid_ssh_public_key(line))
        .map(str::to_string)
        .ok_or_else(|| "La clé SSH créée est illisible. Aucun support n’a été modifié.".into())
}

fn restrict_directory_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn valid_ssh_public_key(line: &str) -> bool {
    let mut fields = line.split_whitespace();
    fields.next() == Some("ssh-ed25519") && fields.next().is_some_and(|value| !value.is_empty())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256_file(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn windows_quote(value: &str) -> String {
    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for character in value.chars() {
        if character == '\\' {
            backslashes += 1;
        } else if character == '"' {
            quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
            quoted.push('"');
            backslashes = 0;
        } else {
            quoted.push_str(&"\\".repeat(backslashes));
            quoted.push(character);
            backslashes = 0;
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

fn io_error(error: io::Error) -> String {
    format!("Opération impossible : {error}")
}

#[cfg(unix)]
fn restrict_file_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(io_error)
}

#[cfg(not(unix))]
fn restrict_file_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        copy_pipe_to_log, imager_cli_args, network_config, powershell_quote,
        process_targets_imager_device, redacted_prefix_len, restrict_file_permissions,
        sensitive_log_values, user_data, windows_quote,
    };
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::{
        fs,
        io::Cursor,
        sync::{Arc, Mutex},
    };

    #[test]
    fn cloud_init_user_data_preserves_default_user_safely_and_enables_ssh() {
        let config = user_data(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExampleKey monde-ensemble",
            "monde-ensemble-test",
        );

        assert!(config.starts_with("#cloud-config\nhostname: \"monde-ensemble-test\"\n"));
        assert!(config.contains("\nuser:\n  name: \"minecraft\"\n"));
        assert!(config.contains("\n  lock_passwd: true\n"));
        assert!(config.contains("\n  sudo: null\n"));
        assert!(config.contains("\nruncmd:\n  - [systemctl, enable, --now, ssh]\n"));
        assert!(!config.contains("enable_ssh:"));
        assert!(!config.contains("users:"));
        assert!(!config.contains("\\n"));
        assert!(config.ends_with('\n'));
    }

    #[test]
    fn network_config_preserves_wifi_values_as_yaml_strings() {
        let config = network_config("Maison: 2.4G", "mot de passe \"secret\"", "fr");

        assert!(config.starts_with("network:\n  version: 2\n  ethernets:\n"));
        assert!(config.contains("  wifis:\n    wlan0:\n"));
        assert!(config.contains("      regulatory-domain: \"FR\"\n"));
        assert!(config.contains("        \"Maison: 2.4G\":\n"));
        assert!(config.contains("          password: \"mot de passe \\\"secret\\\"\"\n"));
        assert!(!config.contains("renderer:"));
        assert!(config.ends_with("      optional: true\n"));
    }

    #[test]
    fn imager_cli_uses_supported_options_and_keeps_image_verification() {
        let args = imager_cli_args(
            &"a".repeat(64),
            Path::new("/private/user-data"),
            Path::new("/private/network-config"),
            Path::new("/private/pi-images/verified.img.xz"),
            "/dev/disk4",
        );

        assert!(!args.contains(&"--disable-telemetry".to_string()));
        assert!(args.contains(&"--sha256".to_string()));
        assert!(args.contains(&"--cloudinit-userdata".to_string()));
        assert!(args.contains(&"--cloudinit-networkconfig".to_string()));
        assert!(args.contains(&"--debug".to_string()));
        assert!(!args.contains(&"--disable-verify".to_string()));
        assert!(args.contains(&"/private/pi-images/verified.img.xz".to_string()));
    }

    #[test]
    fn existing_imager_is_matched_only_to_the_same_device() {
        assert!(process_targets_imager_device(
            "/Volumes/Raspberry Pi Imager.app/Contents/MacOS/rpi-imager --cli /dev/disk16",
            "/dev/disk16"
        ));
        assert!(!process_targets_imager_device(
            "/Volumes/Raspberry Pi Imager.app/Contents/MacOS/rpi-imager --cli /dev/disk15",
            "/dev/disk16"
        ));
        assert!(!process_targets_imager_device(
            "/usr/bin/other-tool /dev/disk16",
            "/dev/disk16"
        ));
    }

    #[test]
    fn persistent_imager_log_redacts_secrets_even_across_read_boundaries() {
        let secret = b"wifi-secret".to_vec();
        let first_chunk = b"start wifi-se";
        let second_chunk = b"cret done";
        let mut combined = first_chunk.to_vec();
        combined.extend_from_slice(second_chunk);

        let safe_len = redacted_prefix_len(&combined, secret.len(), &[secret.clone()]);

        assert!(safe_len <= b"start ".len());
        assert!(safe_len < combined.len());
    }

    #[test]
    fn imager_pipe_never_persists_a_wifi_password_split_across_reads() {
        let path = std::env::temp_dir().join(format!(
            "monde-ensemble-redacted-log-{}.log",
            uuid::Uuid::new_v4().simple()
        ));
        let file = fs::File::create(&path).unwrap();
        let log = Arc::new(Mutex::new(file));
        let secret = b"wifi-secret".to_vec();
        let mut output = vec![b'x'; 8_189];
        output.extend_from_slice(&secret);
        output.extend_from_slice(b" done\n");

        copy_pipe_to_log(Cursor::new(output), log, vec![secret])
            .join()
            .unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains("wifi-secret"));
        assert!(contents.contains("[confidential redacted]"));
        assert!(contents.ends_with(" done\n"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sensitive_log_values_keep_short_wifi_names_from_over_redacting_progress() {
        let values = sensitive_log_values("a", "password-123", "userdata", "network-config");

        assert!(!values.iter().any(|value| value == b"a"));
        assert!(values.iter().any(|value| value == b"password-123"));
        assert!(values.iter().any(|value| value == b"\"a\""));
    }

    #[test]
    fn windows_elevation_arguments_preserve_paths_quotes_and_trailing_slashes() {
        assert_eq!(
            windows_quote(r#"C:\Users\Daniel Streng\Imager\rpi-imager.exe"#),
            r#""C:\Users\Daniel Streng\Imager\rpi-imager.exe""#
        );
        assert_eq!(windows_quote("a\"b"), r#""a\"b""#);
        assert_eq!(
            windows_quote("C:\\path with space\\"),
            "\"C:\\path with space\\\\\""
        );
        assert_eq!(
            powershell_quote(r"C:\Users\Daniel O'Neil"),
            r"'C:\Users\Daniel O''Neil'"
        );
    }

    #[cfg(unix)]
    #[test]
    fn temporary_wifi_configuration_is_readable_only_by_its_owner() {
        let path = std::env::temp_dir().join(format!(
            "monde-ensemble-wifi-{}.conf",
            uuid::Uuid::new_v4().simple()
        ));
        fs::write(&path, "wifi-password").unwrap();

        restrict_file_permissions(&path).unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_file(path).unwrap();
    }
}
