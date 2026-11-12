//! Verified Raspberry Pi Imager runtime and first-boot provisioning.
//!
//! The pinned official utility is downloaded on demand and checked against the
//! SHA-256 published by Raspberry Pi. Its own removable-drive guard and image
//! verification remain enabled for every write.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const IMAGER_VERSION: &str = "2.0.12";
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

#[derive(Serialize)]
pub struct PiFlashResult {
    pub detail: String,
    pub log_path: String,
    pub imager_version: String,
    pub hostname: String,
}

struct MountedImager {
    executable: PathBuf,
    mountpoint: Option<PathBuf>,
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
pub fn flash_pi_os(
    app_data: &Path,
    target_device: &str,
    image_url: &str,
    image_sha256: &str,
    wifi_ssid: &str,
    wifi_password: &str,
    wifi_country: &str,
    progress: impl Fn(&str, u8),
) -> Result<PiFlashResult, String> {
    if !image_url.starts_with("https://downloads.raspberrypi.com/") || !valid_sha256(image_sha256) {
        return Err("Le catalogue Raspberry Pi n’a pas fourni une image vérifiable. Aucun support n’a été modifié.".into());
    }
    validate_wifi(wifi_ssid, wifi_password, wifi_country)?;
    let ssh_key = ensure_ssh_public_key()?;

    let run_id = uuid::Uuid::new_v4().simple().to_string();
    let hostname = format!("monde-ensemble-{run_id}");
    let config_dir = app_data.join("pi-first-boot").join(&run_id);
    fs::create_dir_all(&config_dir).map_err(io_error)?;
    let user_data_path = config_dir.join("user-data");
    let network_config_path = config_dir.join("network-config");
    let _sensitive_files =
        SensitiveConfigFiles(user_data_path.clone(), network_config_path.clone());
    let log_path = config_dir.join("ecriture.log");
    fs::write(&user_data_path, user_data(&ssh_key, &hostname)).map_err(io_error)?;
    fs::write(
        &network_config_path,
        network_config(wifi_ssid, wifi_password, wifi_country),
    )
    .map_err(io_error)?;
    restrict_file_permissions(&user_data_path)?;
    restrict_file_permissions(&network_config_path)?;
    let _ = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(io_error)?;

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
        return Err("La version de l’outil Raspberry Pi Imager n’est pas celle qui a été vérifiée. Aucun support n’a été modifié.".into());
    }

    let args = [
        "--cli".to_string(),
        "--debug".to_string(),
        "--sha256".to_string(),
        image_sha256.to_string(),
        "--cloudinit-userdata".to_string(),
        user_data_path.display().to_string(),
        "--cloudinit-networkconfig".to_string(),
        network_config_path.display().to_string(),
        image_url.to_string(),
        target_device.to_string(),
    ];
    progress(
        "Téléchargement, écriture puis vérification de la carte…",
        18,
    );
    run_imager(&tool, &args, &log_path, &progress)?;

    // Sensitive cloud-init files are removed by the guard on success or error.
    progress("Le système Raspberry Pi est écrit et vérifié.", 100);
    Ok(PiFlashResult {
        detail: format!(
            "Raspberry Pi OS Lite 64 bits écrit et vérifié sur {target_device}. Insère maintenant le support dans le Pi, relie-le au même routeur par câble réseau et allume-le. L’application pourra alors terminer la configuration du serveur."
        ),
        log_path: log_path.display().to_string(),
        imager_version: IMAGER_VERSION.into(),
        hostname,
    })
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
        .status()
        .map_err(|_| "macOS n’a pas pu ouvrir l’outil officiel Raspberry Pi Imager.".to_string())?;
    if !attached.success() {
        let _ = fs::remove_dir(&mountpoint);
        return Err("macOS n’a pas pu ouvrir l’outil officiel Raspberry Pi Imager. Aucun support n’a été modifié.".into());
    }
    let executable = mountpoint.join("Raspberry Pi Imager.app/Contents/MacOS/rpi-imager");
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
    progress: &impl Fn(&str, u8),
) -> Result<(), String> {
    let mut child = match std::env::consts::OS {
        "macos" => spawn_macos_elevated(tool, args)?,
        "windows" => spawn_windows_elevated(tool, args)?,
        "linux" => spawn_linux_elevated(tool, args)?,
        _ => return Err("L’écriture sécurisée n’est pas disponible sur ce système.".into()),
    };
    // The elevated process only writes to its inherited pipes. The ordinary
    // app process owns the log file and copies those bytes, so elevation never
    // follows a user-controlled log path.
    let log = Arc::new(Mutex::new(
        OpenOptions::new()
            .append(true)
            .open(log_path)
            .map_err(io_error)?,
    ));
    let stdout_thread = child
        .stdout
        .take()
        .map(|pipe| copy_pipe_to_log(pipe, Arc::clone(&log)));
    let stderr_thread = child
        .stderr
        .take()
        .map(|pipe| copy_pipe_to_log(pipe, Arc::clone(&log)));
    let mut last_percent = 18u8;
    loop {
        update_write_progress(log_path, &mut last_percent, progress);
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

fn copy_pipe_to_log(
    mut pipe: impl Read + Send + 'static,
    log: Arc<Mutex<File>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            let Ok(count) = pipe.read(&mut buffer) else {
                break;
            };
            if count == 0 {
                break;
            }
            if let Ok(mut file) = log.lock() {
                use std::io::Write;
                if file.write_all(&buffer[..count]).is_err() {
                    break;
                }
            } else {
                break;
            }
        }
    })
}

fn spawn_macos_elevated(tool: &MountedImager, args: &[String]) -> Result<Child, String> {
    let command = format!(
        "{} {}",
        shell_quote(&tool.executable.display().to_string()),
        args.iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let script = format!(
        "do shell script {} with administrator privileges",
        applescript_quote(&command)
    );
    Command::new("osascript")
        .args(["-e", &script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "macOS n’a pas pu demander l’autorisation nécessaire à l’écriture.".into())
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
    if let Some(captures) = pattern.captures_iter(&log).last() {
        let phase = captures.get(1).map(|m| m.as_str()).unwrap_or_default();
        let value = captures
            .get(2)
            .and_then(|m| m.as_str().parse::<u8>().ok())
            .unwrap_or_default()
            .min(100);
        let mapped = if phase.eq_ignore_ascii_case("verifying") {
            78 + value.saturating_mul(20) / 100
        } else {
            18 + value.saturating_mul(58) / 100
        };
        if mapped > *last_percent {
            *last_percent = mapped;
            progress(
                if phase.eq_ignore_ascii_case("verifying") {
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
    let mut config = String::from("version: 2\nethernets:\n  eth0:\n    dhcp4: true\n");
    if !ssid.is_empty() {
        let ssid = serde_json::to_string(ssid).unwrap_or_else(|_| "\"\"".into());
        let password = serde_json::to_string(password).unwrap_or_else(|_| "\"\"".into());
        config.push_str(&format!(
            "wifis:\n  wlan0:\n    dhcp4: true\n    regulatory-domain: {}\n    access-points:\n      {}:\n        password: {}\n",
            country.to_ascii_uppercase(), ssid, password
        ));
    }
    config
}

fn user_data(public_key: &str, hostname: &str) -> String {
    let key = serde_json::to_string(public_key.trim()).unwrap_or_else(|_| "\"\"".into());
    let cleanup = serde_json::to_string(
        r#"for d in /boot/firmware /boot /var/lib/cloud/instance /var/lib/cloud/instances; do find "$d" -maxdepth 2 -type f \( -name user-data -o -name network-config -o -name user-data.txt -o -name network-config.txt \) -delete 2>/dev/null || true; done"#,
    )
    .unwrap_or_else(|_| "\"true\"".into());
    format!(
        // Cloud-init parses this as YAML; these must be real line breaks, not
        // the two-character sequence `\\n` (which makes the whole file invalid).
        "#cloud-config\nhostname: {hostname}\nmanage_etc_hosts: true\nusers:\n  - name: minecraft\n    gecos: Monde Ensemble\n    groups: [users]\n    shell: /bin/bash\n    lock_passwd: true\n    ssh_authorized_keys:\n      - {key}\nssh_pwauth: false\nenable_ssh: true\nruncmd:\n  - [systemctl, enable, ssh]\n  - [systemctl, start, ssh]\n  - [bash, -lc, {cleanup}]\n"
    )
}

fn ensure_ssh_public_key() -> Result<String, String> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| {
            "Le dossier personnel de cet ordinateur n’a pas pu être détecté.".to_string()
        })?;
    let ssh_dir = home.join(".ssh");
    fs::create_dir_all(&ssh_dir).map_err(io_error)?;
    for name in ["id_ed25519", "id_ecdsa", "id_rsa"] {
        let private_key = ssh_dir.join(name);
        let public_key = ssh_dir.join(format!("{name}.pub"));
        if public_key.is_file() && private_key.is_file() {
            let content = fs::read_to_string(&public_key).map_err(io_error)?;
            if content.lines().any(valid_ssh_public_key) {
                return Ok(content
                    .lines()
                    .find(|line| valid_ssh_public_key(line))
                    .unwrap_or_default()
                    .to_string());
            }
        }
    }
    let private_key = ssh_dir.join("id_ed25519");
    let public_key = ssh_dir.join("id_ed25519.pub");
    if private_key.exists() {
        return Err("Une clé SSH existe déjà mais son fichier public n’est pas lisible. L’application n’écrasera jamais cette clé. Consulte le mode expert pour préparer l’accès SSH.".into());
    }
    let output = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "monde-ensemble", "-f"])
        .arg(&private_key)
        .output()
        .map_err(|_| "ssh-keygen n’est pas disponible. L’application ne peut pas préparer l’accès sécurisé au Pi.".to_string())?;
    if !output.status.success() {
        return Err("La clé d’accès SSH au Raspberry Pi n’a pas pu être créée. Aucune clé existante n’a été modifiée.".into());
    }
    let content = fs::read_to_string(&public_key).map_err(io_error)?;
    content
        .lines()
        .find(|line| valid_ssh_public_key(line))
        .map(str::to_string)
        .ok_or_else(|| "La clé SSH créée est illisible. Aucun support n’a été modifié.".into())
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

fn applescript_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
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
        network_config, powershell_quote, restrict_file_permissions, user_data, windows_quote,
    };
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn cloud_init_user_data_is_multiline_and_enables_ssh_at_root() {
        let config = user_data(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExampleKey monde-ensemble",
            "monde-ensemble-test",
        );

        assert!(config.starts_with("#cloud-config\nhostname: monde-ensemble-test\n"));
        assert!(config.contains("\nenable_ssh: true\nruncmd:\n"));
        assert!(!config.contains("\\n"));
        assert!(config.ends_with('\n'));
    }

    #[test]
    fn network_config_preserves_wifi_values_as_yaml_strings() {
        let config = network_config("Maison: 2.4G", "mot de passe \"secret\"", "fr");

        assert!(config.starts_with("version: 2\nethernets:\n"));
        assert!(config.contains("regulatory-domain: FR\n"));
        assert!(config.contains("\"Maison: 2.4G\":\n"));
        assert!(config.contains("password: \"mot de passe \\\"secret\\\"\"\n"));
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
