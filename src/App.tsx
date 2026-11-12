import { useEffect, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import "./App.css";

type SystemInfo = {
  os: string;
  version: string;
  architecture: string;
  memory_gb: number | null;
  free_disk_gb: number | null;
  disk_gb: number | null;
  board: string | null;
  desktop: boolean;
};
type Page = "home" | "install" | "compat" | "backup" | "advanced";
type CompatibilityReport = {
  paper_version: string;
  paper_build_id: number;
  geyser_version: string;
  geyser_build_id: number;
  floodgate_version: string;
  floodgate_build_id: number;
  bedrock_versions: string;
  java_version: string;
  checked_at: string;
  source: string;
};
type PiUpdateCheck = {
  ready: boolean;
  update_available: boolean;
  current_java: string;
  current_bedrock: string;
  current_geyser: string;
  current_floodgate: string;
  latest_java: string;
  latest_bedrock: string;
  latest_geyser: string;
  latest_floodgate: string;
  reasons: string[];
  checked_at: string;
};
type ProgressMessage = { step: string; percent: number };
type OperationResult = { path: string; detail: string; sha256: string | null };
type DiscoveredPi = { host: string; address: string; model: string };
type PiBootTarget = {
  id: string;
  name: string;
  size: string;
  size_bytes: number;
  connection: string;
};
type PiOsRecommendation = {
  name: string;
  release_date: string;
  image_url: string;
  image_sha256: string;
  download_sha256: string;
  download_size_gb: number;
  image_size_gb: number;
  devices: string[];
  init_format: string;
  minimum_server_memory_gb: number;
  initial_server_storage_gb: number;
  checked_at: string;
};
type PiFlashResult = {
  detail: string;
  log_path: string;
  imager_version: string;
  hostname: string;
};
type PiInstallation = {
  host: string;
  username: string;
  serverFolder: string;
  address: string;
};
type LocalInstallation = { serverPath: string };
type LocalServerStatus = {
  running: boolean;
  ready: boolean;
  minecraft_java: string;
  minecraft_bedrock: string;
  java_port: number;
  bedrock_port: number;
  address: string;
  players: string;
  log: string;
};
type PiServerStatus = {
  running: boolean;
  ready: boolean;
  java_port_open: boolean;
  bedrock_port_open: boolean;
  autostart_configured: boolean;
  minecraft_java: string;
  minecraft_bedrock: string;
  paper_build_id: number;
  geyser_version: string;
  geyser_build_id: number;
  floodgate_version: string;
  floodgate_build_id: number;
  java_port: number;
  bedrock_port: number;
  address: string;
  log: string;
};

const nav: { id: Page; label: string; icon: string }[] = [
  { id: "home", label: "Mon monde", icon: "▦" },
  { id: "install", label: "Préparer le serveur", icon: "＋" },
  { id: "compat", label: "Compatibilité", icon: "◈" },
  { id: "backup", label: "Sauvegardes", icon: "▤" },
];

function App() {
  const [page, setPage] = useState<Page>("home");
  const [advanced, setAdvanced] = useState(false);
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [systemError, setSystemError] = useState("");
  const [target, setTarget] = useState<"this" | "pi">("this");
  const [piHost, setPiHost] = useState("");
  const [piUser, setPiUser] = useState("");
  const [piVerified, setPiVerified] = useState(false);
  const [piDiscovering, setPiDiscovering] = useState(false);
  const [piCandidates, setPiCandidates] = useState<DiscoveredPi[]>([]);
  const [piBootTargets, setPiBootTargets] = useState<PiBootTarget[]>([]);
  const [piBootScanState, setPiBootScanState] = useState<
    "idle" | "scanning" | "done"
  >("idle");
  const [piBootScanError, setPiBootScanError] = useState("");
  const [piWifiSsid, setPiWifiSsid] = useState("");
  const [piWifiPassword, setPiWifiPassword] = useState("");
  const [piWifiCountry, setPiWifiCountry] = useState("FR");
  const [piFlashResult, setPiFlashResult] = useState<PiFlashResult | null>(
    null,
  );
  const [piOsRecommendation, setPiOsRecommendation] =
    useState<PiOsRecommendation | null>(null);
  const [piOsLoading, setPiOsLoading] = useState(false);
  const [piOsError, setPiOsError] = useState("");
  const [piInstallation, setPiInstallation] = useState<PiInstallation | null>(
    () => {
      try {
        const stored = localStorage.getItem("monde-ensemble.pi-installation");
        return stored ? (JSON.parse(stored) as PiInstallation) : null;
      } catch {
        return null;
      }
    },
  );
  const [piServerStatus, setPiServerStatus] = useState<PiServerStatus | null>(
    null,
  );
  const [localInstallation, setLocalInstallation] =
    useState<LocalInstallation | null>(() => {
      try {
        const stored = localStorage.getItem(
          "monde-ensemble.local-installation",
        );
        return stored ? (JSON.parse(stored) as LocalInstallation) : null;
      } catch {
        return null;
      }
    });
  const [localServerStatus, setLocalServerStatus] =
    useState<LocalServerStatus | null>(null);
  const [localActionMessage, setLocalActionMessage] = useState("");
  const [localStatusError, setLocalStatusError] = useState("");
  const [piStatusError, setPiStatusError] = useState("");
  const [piActionMessage, setPiActionMessage] = useState("");
  const [piUpdateCheck, setPiUpdateCheck] = useState<PiUpdateCheck | null>(
    null,
  );
  const [piUpdateError, setPiUpdateError] = useState("");
  const [checkingPiUpdate, setCheckingPiUpdate] = useState(false);
  const [connection, setConnection] = useState("");
  const [busy, setBusy] = useState(false);
  const [compatibility, setCompatibility] =
    useState<CompatibilityReport | null>(null);
  const [compatibilityError, setCompatibilityError] = useState("");
  const [eulaAccepted, setEulaAccepted] = useState(false);
  const [piEulaAccepted, setPiEulaAccepted] = useState(false);
  const [destination, setDestination] = useState("");
  const [backupSource, setBackupSource] = useState("");
  const [backupOutput, setBackupOutput] = useState("");
  const [backupStopped, setBackupStopped] = useState(false);
  const [restoreArchive, setRestoreArchive] = useState("");
  const [restoreDestination, setRestoreDestination] = useState("");
  const [operationProgress, setOperationProgress] =
    useState<ProgressMessage | null>(null);
  const [operationResult, setOperationResult] =
    useState<OperationResult | null>(null);
  const [operationError, setOperationError] = useState("");

  useEffect(() => {
    invoke<SystemInfo>("inspect_system")
      .then(setSystem)
      .catch((e) => setSystemError(String(e)));
  }, []);

  useEffect(() => {
    if (piInstallation) {
      localStorage.setItem(
        "monde-ensemble.pi-installation",
        JSON.stringify(piInstallation),
      );
    }
  }, [piInstallation]);

  useEffect(() => {
    if (localInstallation) {
      localStorage.setItem(
        "monde-ensemble.local-installation",
        JSON.stringify(localInstallation),
      );
    }
  }, [localInstallation]);

  useEffect(() => {
    if (page !== "compat" && page !== "install") return;
    setCompatibilityError("");
    invoke<CompatibilityReport>("get_compatibility")
      .then(setCompatibility)
      .catch((e) => setCompatibilityError(String(e)));
  }, [page]);

  useEffect(() => {
    if (page !== "install") return;
    let active = true;
    setPiOsLoading(true);
    setPiOsError("");
    invoke<PiOsRecommendation>("get_pi_os_recommendation")
      .then((recommendation) => {
        if (active) setPiOsRecommendation(recommendation);
      })
      .catch((error) => {
        if (active) setPiOsError(String(error));
      })
      .finally(() => {
        if (active) setPiOsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [page]);

  const progressChannel = () => {
    const channel = new Channel<ProgressMessage>();
    channel.onmessage = setOperationProgress;
    return channel;
  };

  const selectFolder = async (setPath: (path: string) => void) => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") setPath(selected);
  };

  const selectArchive = async () => {
    const selected = await open({
      directory: false,
      multiple: false,
      filters: [{ name: "Sauvegarde Monde Ensemble", extensions: ["gz"] }],
    });
    if (typeof selected === "string") setRestoreArchive(selected);
  };

  const resetOperation = () => {
    setOperationError("");
    setOperationResult(null);
    setOperationProgress(null);
  };

  const createServer = async () => {
    resetOperation();
    setBusy(true);
    try {
      const result = await invoke<OperationResult>("create_server", {
        destinationRoot: destination,
        acceptEula: eulaAccepted,
        memoryGb: system?.memory_gb ?? null,
        progress: progressChannel(),
      });
      setOperationResult(result);
      setLocalInstallation({ serverPath: result.path });
      try {
        setLocalServerStatus(
          await invoke<LocalServerStatus>("inspect_local_server", {
            serverPath: result.path,
          }),
        );
        setLocalStatusError("");
      } catch (error) {
        setLocalStatusError(String(error));
      }
    } catch (error) {
      setOperationError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const createBackup = async () => {
    resetOperation();
    setBusy(true);
    try {
      const output =
        backupOutput ||
        (await save({
          defaultPath: "Monde-Ensemble-Sauvegarde.tar.gz",
          filters: [{ name: "Archive compressée", extensions: ["gz"] }],
        }));
      if (!output) return;
      setBackupOutput(output);
      const result = await invoke<OperationResult>("backup_server", {
        source: backupSource,
        outputFile: output,
        serverStopped: backupStopped,
        progress: progressChannel(),
      });
      setOperationResult(result);
    } catch (error) {
      setOperationError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const restoreWorld = async () => {
    resetOperation();
    setBusy(true);
    try {
      const result = await invoke<OperationResult>("restore_server", {
        archive: restoreArchive,
        destinationRoot: restoreDestination,
        progress: progressChannel(),
      });
      setOperationResult(result);
    } catch (error) {
      setOperationError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const testPi = async () => {
    setBusy(true);
    setPiVerified(false);
    setConnection("Vérification de l’accès…");
    try {
      setConnection(
        await invoke<string>("probe_ssh", { host: piHost, username: piUser }),
      );
      setPiVerified(true);
    } catch (e) {
      setConnection(String(e));
    } finally {
      setBusy(false);
    }
  };

  const findPi = async () => {
    if (!piUser) {
      setConnection("Saisis d’abord le nom du compte utilisé sur le Pi.");
      return;
    }
    setPiDiscovering(true);
    setPiVerified(false);
    setConnection("Recherche du Raspberry Pi sur le réseau local…");
    try {
      const found = await invoke<DiscoveredPi[]>("discover_pi", {
        username: piUser,
      });
      setPiCandidates(found);
      if (found.length === 1) {
        const pi = found[0];
        setPiHost(pi.host);
        setPiVerified(true);
        setConnection(`${pi.model} · trouvé à ${pi.address} · accès vérifié`);
      } else {
        setPiVerified(false);
        setConnection(
          `${found.length} Raspberry Pi trouvés. Choisis celui qui hébergera le serveur.`,
        );
      }
    } catch (error) {
      setConnection(String(error));
    } finally {
      setPiDiscovering(false);
    }
  };

  const findPiBootTargets = async () => {
    setPiBootScanState("scanning");
    setPiBootScanError("");
    setPiBootTargets([]);
    try {
      setPiBootTargets(
        await invoke<PiBootTarget[]>("discover_pi_boot_targets"),
      );
      setPiBootScanState("done");
    } catch (error) {
      setPiBootScanError(String(error));
      setPiBootScanState("idle");
    }
  };

  const flashPiOs = async (device: PiBootTarget) => {
    if (!piOsRecommendation) return;
    const minimumGb = Math.ceil(
      piOsRecommendation.image_size_gb +
        piOsRecommendation.initial_server_storage_gb,
    );
    if (device.size_bytes < minimumGb * 1_000_000_000) {
      setOperationError(
        `Ce support est trop petit. Prévois au moins ${minimumGb} Go.`,
      );
      return;
    }
    const confirmed = window.confirm(
      `Le support « ${device.name} » (${device.size}) va être entièrement effacé pour préparer le Raspberry Pi. Les autres disques ne seront pas ciblés. Continuer ?`,
    );
    if (!confirmed) return;
    resetOperation();
    setPiFlashResult(null);
    setBusy(true);
    try {
      const result = await invoke<PiFlashResult>("prepare_pi_sd", {
        targetId: device.id,
        targetName: device.name,
        targetSizeBytes: device.size_bytes,
        imageUrl: piOsRecommendation.image_url,
        imageSha256: piOsRecommendation.download_sha256,
        wifiSsid: piWifiSsid,
        wifiPassword: piWifiPassword,
        wifiCountry: piWifiCountry,
        confirmErase: true,
        progress: progressChannel(),
      });
      setPiFlashResult(result);
      setPiWifiPassword("");
    } catch (error) {
      setOperationError(String(error));
      setPiWifiPassword("");
    } finally {
      setBusy(false);
    }
  };

  const installPiAt = async (host: string, username: string) => {
    resetOperation();
    setBusy(true);
    try {
      const result = await invoke<OperationResult>("install_pi", {
        host,
        username,
        acceptEula: piEulaAccepted,
        progress: progressChannel(),
      });
      setOperationResult(result);
      const serverFolder = result.path.match(
        /Monde-Ensemble-[a-f0-9]{32}\/server/,
      )?.[0];
      const address = result.detail.match(/(?:\d{1,3}\.){3}\d{1,3}/)?.[0];
      if (serverFolder && address) {
        const installation = {
          host,
          username,
          serverFolder,
          address,
        };
        setPiInstallation(installation);
        try {
          const status = await invoke<PiServerStatus>("inspect_pi_server", {
            host: installation.host,
            username: installation.username,
            serverFolder: installation.serverFolder,
          });
          setPiServerStatus(status);
          setPiStatusError("");
        } catch (error) {
          setPiStatusError(
            `Le serveur est installé, mais son tableau de bord n’a pas pu être actualisé : ${String(error)}`,
          );
        }
      }
    } catch (error) {
      setOperationError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const installPi = async () => installPiAt(piHost, piUser);

  const finishNewPi = async () => {
    if (!piFlashResult) return;
    if (!piEulaAccepted) {
      setOperationError(
        "Accepte d’abord le contrat de licence Minecraft pour installer le serveur.",
      );
      return;
    }
    resetOperation();
    setBusy(true);
    setOperationProgress({
      step: "Recherche du Pi sur le réseau local…",
      percent: 8,
    });
    try {
      let device: DiscoveredPi | undefined;
      let lastError = "";
      for (let attempt = 0; attempt < 12; attempt += 1) {
        setOperationProgress({
          step:
            attempt === 0
              ? "Recherche du Pi préparé sur le réseau local…"
              : `Premier démarrage du Pi… recherche ${attempt + 1}/12`,
          percent: 8,
        });
        try {
          const found = await invoke<DiscoveredPi[]>("discover_new_pi", {
            expectedHostname: piFlashResult.hostname,
          });
          if (found.length > 1) {
            throw new Error(
              "Plusieurs Raspberry Pi préparés répondent. Éteins les autres Pi puis réessaie.",
            );
          }
          if (found.length === 1) {
            device = found[0];
            break;
          }
        } catch (error) {
          lastError = String(error);
          if (lastError.includes("Plusieurs Raspberry Pi préparés"))
            throw error;
        }
        if (attempt < 11) {
          await new Promise((resolve) => window.setTimeout(resolve, 5000));
        }
      }
      if (!device) {
        throw new Error(
          `Le Pi n’a pas encore rejoint le réseau. Vérifie qu’il est allumé et relié au même routeur, puis réessaie. ${lastError}`,
        );
      }
      setPiHost(device.host);
      setPiUser("minecraft");
      setPiVerified(true);
      setConnection(
        `${device.model} · trouvé sur le réseau · accès sécurisé confirmé`,
      );
      setBusy(false);
      await installPiAt(device.host, "minecraft");
    } catch (error) {
      setOperationError(String(error));
      setBusy(false);
    }
  };

  const refreshPiStatus = async (includeLogs = false) => {
    if (!piInstallation) return;
    setBusy(true);
    setPiStatusError("");
    try {
      const status = await invoke<PiServerStatus>("inspect_pi_server", {
        host: piInstallation.host,
        username: piInstallation.username,
        serverFolder: piInstallation.serverFolder,
      });
      setPiServerStatus(status);
      if (includeLogs && !status.log) {
        setPiStatusError("Le serveur n’a pas encore écrit de journal.");
      }
    } catch (error) {
      setPiStatusError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const checkPiUpdate = async () => {
    if (!piInstallation) return;
    setCheckingPiUpdate(true);
    setPiUpdateError("");
    setPiUpdateCheck(null);
    try {
      const report = await invoke<PiUpdateCheck>("check_pi_update", {
        host: piInstallation.host,
        username: piInstallation.username,
        serverFolder: piInstallation.serverFolder,
      });
      setPiUpdateCheck(report);
    } catch (error) {
      setPiUpdateError(String(error));
    } finally {
      setCheckingPiUpdate(false);
    }
  };

  useEffect(() => {
    if (page === "home" && piInstallation) void refreshPiStatus(false);
    // A status check runs when the saved server dashboard is opened.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page, piInstallation?.serverFolder]);

  const refreshLocalStatus = async () => {
    if (!localInstallation) return;
    setLocalStatusError("");
    try {
      setLocalServerStatus(
        await invoke<LocalServerStatus>("inspect_local_server", {
          serverPath: localInstallation.serverPath,
        }),
      );
    } catch (error) {
      setLocalStatusError(String(error));
    }
  };

  useEffect(() => {
    if (page === "home" && localInstallation) void refreshLocalStatus();
    // The saved local path is the only input to this read-only status refresh.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page, localInstallation?.serverPath]);

  const startLocalServer = async () => {
    if (!localInstallation) return;
    setBusy(true);
    setLocalStatusError("");
    setLocalActionMessage("");
    setOperationProgress({
      step: "Démarrage et vérification du serveur…",
      percent: 5,
    });
    try {
      setLocalActionMessage(
        await invoke<string>("start_local_server", {
          serverPath: localInstallation.serverPath,
          progress: progressChannel(),
        }),
      );
      await refreshLocalStatus();
    } catch (error) {
      setLocalStatusError(String(error));
    } finally {
      setOperationProgress(null);
      setBusy(false);
    }
  };

  const stopLocalServer = async () => {
    if (!localInstallation) return;
    if (
      !window.confirm(
        "Arrêter le serveur déconnectera les joueurs présents. Le monde sera enregistré avant l’arrêt. Continuer ?",
      )
    )
      return;
    setBusy(true);
    setLocalStatusError("");
    setLocalActionMessage("");
    try {
      setLocalActionMessage(
        await invoke<string>("stop_local_server", {
          serverPath: localInstallation.serverPath,
        }),
      );
      await refreshLocalStatus();
    } catch (error) {
      setLocalStatusError(String(error));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (piServerStatus?.address && piInstallation) {
      setPiInstallation((current) =>
        current && current.address !== piServerStatus.address
          ? { ...current, address: piServerStatus.address }
          : current,
      );
    }
  }, [piServerStatus?.address]);

  const startPiServer = async () => {
    if (!piInstallation) return;
    setBusy(true);
    setPiStatusError("");
    setPiActionMessage("");
    setOperationProgress({
      step: "Vérification des mises à jour et sauvegarde de sécurité…",
      percent: 2,
    });
    try {
      const detail = await invoke<string>("start_pi_server", {
        host: piInstallation.host,
        username: piInstallation.username,
        serverFolder: piInstallation.serverFolder,
        progress: progressChannel(),
      });
      setPiActionMessage(detail);
      await new Promise((resolve) => window.setTimeout(resolve, 2500));
      await refreshPiStatus(true);
    } catch (error) {
      setPiStatusError(String(error));
    } finally {
      setOperationProgress(null);
      setBusy(false);
    }
  };

  const stopPiServer = async () => {
    if (!piInstallation) return;
    if (
      !window.confirm(
        "Arrêter le serveur déconnectera les joueurs présents. Le monde sera enregistré avant l’arrêt. Continuer ?",
      )
    )
      return;
    setBusy(true);
    setPiStatusError("");
    setPiActionMessage("");
    try {
      const detail = await invoke<string>("stop_pi_server", {
        host: piInstallation.host,
        username: piInstallation.username,
        serverFolder: piInstallation.serverFolder,
      });
      setPiActionMessage(detail);
      await refreshPiStatus(true);
    } catch (error) {
      setPiStatusError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const pageTitle: Record<Page, string> = {
    home: "Ton monde, en sécurité",
    install: "Préparer le serveur",
    compat: "Ce qui peut jouer ensemble",
    backup: "Sauvegarder et déplacer",
    advanced: "Réglages avancés",
  };
  const pageIntro: Record<Page, string> = {
    home: "Prépare un serveur neuf, protège ton monde dans une archive et restaure-le sans écraser l’original.",
    install:
      "On vérifie d’abord la machine, puis on prépare une installation adaptée.",
    compat:
      "Les règles qui rendront les mises à jour sûres, expliquées sans jargon.",
    backup:
      "Tes constructions et réglages doivent pouvoir te suivre sur une autre machine.",
    advanced: "Les réglages avancés restent séparés de l’installation guidée.",
  };

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">
            <i />
            <i />
            <i />
            <i />
          </div>
          <div>
            <strong>MONDE</strong>
            <span>ENSEMBLE</span>
          </div>
        </div>
        <div className="side-label">TON SERVEUR</div>
        <nav>
          {nav.map((item) => (
            <button
              key={item.id}
              onClick={() => setPage(item.id)}
              className={page === item.id ? "nav-item active" : "nav-item"}
            >
              <b>{item.icon}</b>
              {item.label}
            </button>
          ))}
        </nav>
        <div className="side-spacer" />
        <button
          className={advanced ? "nav-item active" : "nav-item"}
          onClick={() => {
            setAdvanced(!advanced);
            setPage(!advanced ? "advanced" : "home");
          }}
        >
          <b>⚙</b>
          {advanced ? "Mode débutant" : "Mode avancé"}
        </button>
        <div className="server-mini">
          <span className="status-dot" />
          <div>
            <strong>Installations protégées</strong>
            <small>Le serveur actuel n’est jamais remplacé</small>
          </div>
        </div>
        <div className="side-version">
          MONDE ENSEMBLE <span>·</span> 0.8.0
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div className="crumb">
            MONDE ENSEMBLE <span>/</span> {pageTitle[page].toUpperCase()}
          </div>
          <div className="top-status">
            <span>AUCUN ÉCRASEMENT · COPIE NEUVE</span>
            <div className="beginner-pill">
              <span>✦</span> MODE {advanced ? "AVANCÉ" : "DÉBUTANT"}
            </div>
          </div>
        </header>
        <section className="page-wrap">
          <div className="page-heading">
            <div>
              <div className="eyebrow">TON SERVEUR MINECRAFT</div>
              <h1>{pageTitle[page]}</h1>
              <p>{pageIntro[page]}</p>
            </div>
            <div className="world-stamp">
              <div className="grass" />
              <div className="dirt" />
              <div className="stamp-text">
                JAVA
                <br />
                <b>＋</b>
                <br />
                BEDROCK
              </div>
            </div>
          </div>
          {page === "home" && (
            <Home
              system={system}
              systemError={systemError}
              localInstallation={localInstallation}
              localServerStatus={localServerStatus}
              localStatusError={localStatusError}
              localActionMessage={localActionMessage}
              onRefreshLocal={refreshLocalStatus}
              onStartLocal={startLocalServer}
              onStopLocal={stopLocalServer}
              piInstallation={piInstallation}
              piServerStatus={piServerStatus}
              piStatusError={piStatusError}
              piActionMessage={piActionMessage}
              actionProgress={busy ? (operationProgress?.step ?? "") : ""}
              busy={busy}
              onRefreshPi={() => refreshPiStatus(true)}
              onStartPi={startPiServer}
              onStopPi={stopPiServer}
              onCheckPiUpdate={checkPiUpdate}
              piUpdateCheck={piUpdateCheck}
              piUpdateError={piUpdateError}
              checkingPiUpdate={checkingPiUpdate}
              onInstall={() => setPage("install")}
              onCompat={() => setPage("compat")}
              onBackup={() => setPage("backup")}
            />
          )}
          {page === "install" && (
            <section className="install-layout">
              <div className="wizard-main">
                <div className="step-ribbon">
                  <span className="step-num">1</span>
                  <div>
                    <strong>Où veux-tu mettre ton serveur ?</strong>
                    <small>
                      Tu pourras le déplacer plus tard avec une sauvegarde.
                    </small>
                  </div>
                </div>
                <div className="target-choice">
                  <button
                    className={
                      target === "this" ? "target-card selected" : "target-card"
                    }
                    onClick={() => setTarget("this")}
                  >
                    <span className="target-icon">▣</span>
                    <strong>Sur cet ordinateur</strong>
                    <small>Mac, PC ou Raspberry Pi avec écran.</small>
                    <em>Le plus simple pour commencer</em>
                  </button>
                  <button
                    className={
                      target === "pi" ? "target-card selected" : "target-card"
                    }
                    onClick={() => setTarget("pi")}
                  >
                    <span className="target-icon">▤</span>
                    <strong>Sur un Raspberry Pi</strong>
                    <small>Le serveur peut tourner sans écran.</small>
                    <em>Besoin d’un accès configuré une fois</em>
                  </button>
                </div>
                {target === "this" ? (
                  <>
                    <SystemPanel system={system} error={systemError} />
                    <LocalInstallPanel
                      compatibility={compatibility}
                      compatibilityError={compatibilityError}
                      destination={destination}
                      eulaAccepted={eulaAccepted}
                      setEulaAccepted={setEulaAccepted}
                      busy={busy}
                      progress={operationProgress}
                      result={operationResult}
                      error={operationError}
                      onChooseFolder={() => selectFolder(setDestination)}
                      onInstall={createServer}
                    />
                  </>
                ) : (
                  <div className="pi-panel">
                    <div className="eyebrow">INSTALLATION GUIDÉE</div>
                    <h3>Installer réellement le serveur sur le Pi</h3>
                    <p>
                      L’application va trouver le Pi sur le réseau, vérifier son
                      modèle, sa mémoire et l’espace disponible, puis installer
                      et démarrer le serveur. Elle crée un nouveau dossier sans
                      supprimer les fichiers déjà présents.
                    </p>
                    <p className="pi-setup-help">
                      Pour éviter de te faire saisir une adresse IP, commence
                      par « Trouver mon Pi ». Il faut seulement connaître le nom
                      du compte créé sur le Pi. Cette version exige que son
                      accès sécurisé soit déjà activé; elle ne te demandera ni
                      ne conservera de mot de passe.
                    </p>
                    <div className="form-row">
                      <label>
                        Compte Raspberry Pi
                        <input
                          value={piUser}
                          onChange={(e) => {
                            setPiUser(e.target.value);
                            setPiVerified(false);
                          }}
                          placeholder="ex. daniel"
                          autoComplete="off"
                        />
                        <small>
                          Le compte choisi lors de la première configuration du
                          Raspberry Pi.
                        </small>
                      </label>
                      <label>
                        Adresse (si détection impossible)
                        <input
                          value={piHost}
                          onChange={(e) => {
                            setPiHost(e.target.value);
                            setPiVerified(false);
                          }}
                          placeholder="Détection automatique"
                          autoComplete="off"
                        />
                      </label>
                    </div>
                    {piCandidates.length > 1 && (
                      <label className="pi-candidate-choice">
                        Raspberry Pi trouvé
                        <select
                          value={piHost}
                          onChange={(event) => {
                            const selected = piCandidates.find(
                              (pi) => pi.host === event.target.value,
                            );
                            setPiHost(selected?.host ?? "");
                            setPiVerified(Boolean(selected));
                            setConnection(
                              selected
                                ? `${selected.model} · ${selected.address} · accès vérifié`
                                : "",
                            );
                          }}
                        >
                          <option value="">Choisir le bon appareil…</option>
                          {piCandidates.map((pi) => (
                            <option
                              key={`${pi.host}-${pi.address}`}
                              value={pi.host}
                            >
                              {pi.model} · {pi.address}
                            </option>
                          ))}
                        </select>
                      </label>
                    )}
                    <div className="pi-actions">
                      <button
                        className="button secondary"
                        onClick={findPi}
                        disabled={busy || piDiscovering || !piUser}
                      >
                        {piDiscovering ? "Recherche…" : "Trouver mon Pi"}
                      </button>
                      <button
                        className="button secondary"
                        onClick={testPi}
                        disabled={busy || !piHost || !piUser}
                      >
                        Vérifier cette adresse
                      </button>
                    </div>
                    {connection && (
                      <div className="connection-result">{connection}</div>
                    )}
                    <label className="eula-choice">
                      <input
                        type="checkbox"
                        checked={piEulaAccepted}
                        onChange={(event) =>
                          setPiEulaAccepted(event.target.checked)
                        }
                      />
                      <span>
                        J’accepte le contrat de licence Minecraft pour ce
                        serveur.
                      </span>
                    </label>
                    <button
                      className="button primary action-button"
                      onClick={installPi}
                      disabled={busy || !piVerified || !piEulaAccepted}
                    >
                      {busy
                        ? "Installation du serveur sur le Pi…"
                        : "Installer et démarrer le serveur"}
                    </button>
                    {(operationProgress ||
                      operationResult ||
                      operationError) && (
                      <OperationStatus
                        progress={operationProgress}
                        result={operationResult}
                        error={operationError}
                      />
                    )}
                    <div className="pi-os-recommendation" aria-live="polite">
                      <div className="eyebrow">
                        SYSTÈME CHOISI AUTOMATIQUEMENT
                      </div>
                      {piOsLoading && (
                        <p>Vérification du catalogue officiel Raspberry Pi…</p>
                      )}
                      {piOsError && (
                        <p className="pi-error">
                          Impossible de confirmer le système recommandé :{" "}
                          {piOsError}
                        </p>
                      )}
                      {piOsRecommendation && (
                        <>
                          <strong>{piOsRecommendation.name}</strong>
                          <span>
                            Publication du{" "}
                            {new Date(
                              `${piOsRecommendation.release_date}T12:00:00`,
                            ).toLocaleDateString("fr-FR")}{" "}
                            · téléchargement{" "}
                            {piOsRecommendation.download_size_gb} Go · écrit{" "}
                            {piOsRecommendation.image_size_gb} Go · 64 bits
                          </span>
                          <span>
                            Modèles compatibles :{" "}
                            {describePiModels(piOsRecommendation.devices)}.
                          </span>
                          <small>
                            Les Pi 3 ne conviennent pas à ce serveur. Pour
                            l’héberger, choisis un Pi 4 ou 5 avec au moins{" "}
                            {piOsRecommendation.minimum_server_memory_gb} Go de
                            mémoire. L’application vérifiera le modèle et la
                            mémoire avant l’installation.
                          </small>
                        </>
                      )}
                    </div>
                    <section
                      className="pi-first-boot"
                      aria-labelledby="pi-fresh-title"
                    >
                      <div className="eyebrow">PRÉPARATION D’UN PI NEUF</div>
                      <h4 id="pi-fresh-title">
                        Préparer le Raspberry Pi depuis cette application
                      </h4>
                      <p>
                        Branche sa carte SD ou son SSD à cet ordinateur, puis
                        lance la détection. L’application choisira le système
                        compatible et vérifiera le support avant de l’effacer.
                      </p>
                      <button
                        className="button secondary"
                        onClick={findPiBootTargets}
                        disabled={piBootScanState === "scanning"}
                      >
                        {piBootScanState === "scanning"
                          ? "Recherche des supports…"
                          : "Détecter la carte SD ou le SSD"}
                      </button>
                      {piBootScanError && (
                        <p className="pi-error">{piBootScanError}</p>
                      )}
                      <div className="pi-wireless-fields">
                        <div>
                          <label htmlFor="pi-wifi-ssid">
                            Réseau Wi-Fi (facultatif)
                          </label>
                          <input
                            id="pi-wifi-ssid"
                            value={piWifiSsid}
                            onChange={(event) =>
                              setPiWifiSsid(event.target.value)
                            }
                            autoComplete="off"
                            placeholder="Vide si le Pi sera relié par câble"
                            disabled={busy || Boolean(piFlashResult)}
                          />
                        </div>
                        {piWifiSsid && (
                          <>
                            <small className="pi-wifi-note">
                              Le mot de passe sert à connecter le Pi au premier
                              démarrage ; il est effacé du support après cette
                              configuration. Pour éviter de l’écrire sur la
                              carte, utilise plutôt un câble réseau.
                            </small>
                            <div>
                              <label htmlFor="pi-wifi-password">
                                Mot de passe Wi-Fi
                              </label>
                              <input
                                id="pi-wifi-password"
                                type="password"
                                value={piWifiPassword}
                                onChange={(event) =>
                                  setPiWifiPassword(event.target.value)
                                }
                                autoComplete="new-password"
                                disabled={busy || Boolean(piFlashResult)}
                              />
                            </div>
                            <div>
                              <label htmlFor="pi-wifi-country">
                                Pays du réseau Wi-Fi
                              </label>
                              <select
                                id="pi-wifi-country"
                                value={piWifiCountry}
                                onChange={(event) =>
                                  setPiWifiCountry(event.target.value)
                                }
                                disabled={busy || Boolean(piFlashResult)}
                              >
                                <option value="FR">France</option>
                                <option value="BE">Belgique</option>
                                <option value="CH">Suisse</option>
                                <option value="CA">Canada</option>
                                <option value="US">États-Unis</option>
                                <option value="GB">Royaume-Uni</option>
                                <option value="DE">Allemagne</option>
                                <option value="ES">Espagne</option>
                                <option value="IT">Italie</option>
                                <option value="NL">Pays-Bas</option>
                              </select>
                            </div>
                          </>
                        )}
                        {!piWifiSsid && (
                          <small>
                            Sans Wi-Fi ici, branche le Pi au routeur avec un
                            câble réseau lors du premier démarrage.
                          </small>
                        )}
                      </div>
                      {piBootScanState === "done" &&
                        piBootTargets.length === 0 && (
                          <p className="pi-help">
                            Aucun support externe compatible n’a été détecté.
                            Vérifie son branchement puis relance la recherche.
                          </p>
                        )}
                      {piBootTargets.length > 0 && (
                        <ul className="pi-storage-list">
                          {piBootTargets.map((device) => (
                            <li key={device.id}>
                              <div>
                                <strong>{device.name}</strong>
                                <span>
                                  {device.size} · {device.connection}
                                </span>
                              </div>
                              {piOsRecommendation && device.size_bytes > 0 && (
                                <em
                                  className={
                                    device.size_bytes >=
                                    Math.ceil(
                                      piOsRecommendation.image_size_gb +
                                        piOsRecommendation.initial_server_storage_gb,
                                    ) *
                                      1_000_000_000
                                      ? "storage-ready"
                                      : "storage-small"
                                  }
                                >
                                  {device.size_bytes >=
                                  Math.ceil(
                                    piOsRecommendation.image_size_gb +
                                      piOsRecommendation.initial_server_storage_gb,
                                  ) *
                                    1_000_000_000
                                    ? "Assez de place pour l’OS et le serveur"
                                    : `Trop petit · prévois au moins ${Math.ceil(piOsRecommendation.image_size_gb + piOsRecommendation.initial_server_storage_gb)} Go`}
                                </em>
                              )}
                              {piOsRecommendation &&
                                device.size_bytes >=
                                  Math.ceil(
                                    piOsRecommendation.image_size_gb +
                                      piOsRecommendation.initial_server_storage_gb,
                                  ) *
                                    1_000_000_000 && (
                                  <button
                                    className="button secondary pi-flash-button"
                                    onClick={() => flashPiOs(device)}
                                    disabled={
                                      busy ||
                                      Boolean(piFlashResult) ||
                                      !piOsRecommendation.download_sha256
                                    }
                                  >
                                    {busy
                                      ? "Préparation en cours…"
                                      : "Effacer et préparer ce support"}
                                  </button>
                                )}
                            </li>
                          ))}
                        </ul>
                      )}
                      {piOsRecommendation && (
                        <p className="pi-storage-guidance">
                          Il faut environ{" "}
                          {Math.ceil(
                            piOsRecommendation.image_size_gb +
                              piOsRecommendation.initial_server_storage_gb,
                          )}{" "}
                          Go pour le système et l’espace initial du serveur. Une
                          carte microSD de 16 Go suffit au profil initial; 32 Go
                          laisse plus de place au monde. Un SSD USB dépend du
                          modèle du Pi.
                        </p>
                      )}
                      {(operationProgress || operationError) && (
                        <OperationStatus
                          progress={operationProgress}
                          result={null}
                          error={operationError}
                        />
                      )}
                      {piFlashResult && (
                        <div className="pi-flash-success" aria-live="polite">
                          <strong>Carte prête · vérification terminée</strong>
                          <p>{piFlashResult.detail}</p>
                          <p>
                            Le premier démarrage peut prendre quelques minutes.
                            Laisse le Pi branché au même routeur et allumé ;
                            l’application le recherchera automatiquement avant
                            d’installer le serveur.
                          </p>
                          <button
                            className="button primary"
                            onClick={finishNewPi}
                            disabled={busy || !piEulaAccepted}
                          >
                            {busy
                              ? "Recherche automatique du Pi…"
                              : "Le Pi est allumé · terminer l’installation"}
                          </button>
                          <small>
                            Outil officiel Raspberry Pi Imager{" "}
                            {piFlashResult.imager_version} · journal :{" "}
                            {piFlashResult.log_path}
                          </small>
                        </div>
                      )}
                    </section>
                  </div>
                )}
                <div className="notice safe">
                  <span>✓</span>
                  <p>
                    <strong>Le serveur actuel reste intact.</strong>
                    <br />
                    Une installation neuve est créée dans un dossier distinct;
                    la restauration ne remplace pas l’original.
                  </p>
                </div>
              </div>
              <aside className="wizard-aside">
                <div className="aside-cap">COMMENT ÇA VA SE PASSER</div>
                <ol>
                  <li className="current">
                    <i>1</i>
                    <span>
                      <b>Vérifier la machine</b>
                      <small>Modèle, mémoire et disque</small>
                    </span>
                  </li>
                  <li>
                    <i>2</i>
                    <span>
                      <b>Choisir le profil compatible</b>
                      <small>Java, Bedrock et extensions</small>
                    </span>
                  </li>
                  <li>
                    <i>3</i>
                    <span>
                      <b>Préparer le serveur</b>
                      <small>Une copie sûre avant l’activation</small>
                    </span>
                  </li>
                </ol>
                <div className="aside-bottom">
                  Tu peux arrêter à chaque étape.
                  <br />
                  Ton monde existant reste intact.
                </div>
              </aside>
            </section>
          )}
          {page === "compat" && (
            <Compatibility report={compatibility} error={compatibilityError} />
          )}
          {page === "backup" && (
            <Backups
              source={backupSource}
              output={backupOutput}
              stopped={backupStopped}
              setStopped={setBackupStopped}
              archive={restoreArchive}
              destination={restoreDestination}
              busy={busy}
              progress={operationProgress}
              result={operationResult}
              error={operationError}
              onChooseSource={() => selectFolder(setBackupSource)}
              onChooseArchive={selectArchive}
              onChooseDestination={() => selectFolder(setRestoreDestination)}
              onBackup={createBackup}
              onRestore={restoreWorld}
            />
          )}
          {page === "advanced" && (
            <Advanced
              system={system}
              localInstallation={localInstallation}
              localServerStatus={localServerStatus}
              piInstallation={piInstallation}
              piServerStatus={piServerStatus}
              piUpdateCheck={piUpdateCheck}
              piUpdateError={piUpdateError}
              busy={busy || checkingPiUpdate}
              onRefresh={() => refreshPiStatus(true)}
              onCheckUpdates={checkPiUpdate}
            />
          )}
        </section>
        <footer className="footer">
          <span>Conçu pour garder ton monde intact.</span>
          <span>Cible : macOS · Windows · Raspberry Pi OS ARM64</span>
        </footer>
      </main>
    </div>
  );
}

function describePiModels(devices: string[]) {
  const generations = Array.from(
    new Set(
      devices
        .map((device) => device.match(/^pi(\d+)-64bit$/)?.[1])
        .filter((generation): generation is string => Boolean(generation)),
    ),
  ).sort();
  if (generations.length === 0) return "à vérifier dans le catalogue";
  const models = generations.map((generation) => `Pi ${generation}`);
  if (models.length === 1) return `${models[0]} (64 bits)`;
  return `${models.slice(0, -1).join(", ")} et ${models[models.length - 1]} (64 bits)`;
}

function SystemPanel({
  system,
  error,
}: {
  system: SystemInfo | null;
  error: string;
}) {
  return (
    <div className="machine-panel">
      <div className="panel-head">
        <div>
          <div className="eyebrow">ÉTAPE 1 · VÉRIFICATION</div>
          <h3>Ta machine</h3>
        </div>
        <span className={system ? "detect-tag" : "detect-tag loading"}>
          {system ? "DÉTECTÉE" : "ANALYSE…"}
        </span>
      </div>
      {error && <p className="error-text">Lecture impossible : {error}</p>}
      {system ? (
        <>
          <div className="machine-name">
            <span>▣</span>
            <div>
              <strong>{system.board || system.os}</strong>
              <small>
                {system.os} {system.version} · {system.architecture}
              </small>
            </div>
          </div>
          <div className="spec-grid">
            <Spec
              label="MÉMOIRE"
              value={
                system.memory_gb ? `${system.memory_gb} Go` : "Non détectée"
              }
              note="Mémoire vive"
            />
            <Spec
              label="DISQUE"
              value={system.disk_gb ? `${system.disk_gb} Go` : "Non détecté"}
              note="Taille totale"
            />
            <Spec
              label="ESPACE LIBRE"
              value={
                system.free_disk_gb
                  ? `${system.free_disk_gb} Go`
                  : "Non détecté"
              }
              note="Sur le disque système"
            />
          </div>
          {system.board && (
            <p className="pi-tip">
              La capacité du stockage externe sera vérifiée aussi avant la
              préparation du serveur.
            </p>
          )}
        </>
      ) : (
        <div className="skeleton">Lecture des caractéristiques…</div>
      )}
    </div>
  );
}

function LocalInstallPanel({
  compatibility,
  compatibilityError,
  destination,
  eulaAccepted,
  setEulaAccepted,
  busy,
  progress,
  result,
  error,
  onChooseFolder,
  onInstall,
}: {
  compatibility: CompatibilityReport | null;
  compatibilityError: string;
  destination: string;
  eulaAccepted: boolean;
  setEulaAccepted: (accepted: boolean) => void;
  busy: boolean;
  progress: ProgressMessage | null;
  result: OperationResult | null;
  error: string;
  onChooseFolder: () => void;
  onInstall: () => void;
}) {
  return (
    <div className="action-panel">
      <div className="panel-head">
        <div>
          <div className="eyebrow">ÉTAPE 2 · NOUVELLE INSTALLATION</div>
          <h3>Un serveur propre</h3>
        </div>
        <span className="detect-tag">PAPER + GEYSER</span>
      </div>
      <p className="action-description">
        Prépare un nouveau serveur Paper avec Geyser et Floodgate, à côté des
        dossiers existants. Cette étape crée un monde vide; pour garder des
        constructions, restaure d’abord une sauvegarde dans l’onglet «
        Sauvegardes ».
      </p>
      <div className="release-grid">
        <ReleaseItem
          label="MINECRAFT JAVA"
          value={compatibility?.java_version ?? "Vérification…"}
        />
        <ReleaseItem
          label="MINECRAFT BEDROCK"
          value={compatibility?.bedrock_versions ?? "Vérification…"}
        />
        <ReleaseItem
          label="PAPER"
          value={compatibility?.paper_version ?? "Vérification…"}
        />
        <ReleaseItem
          label="GEYSER / FLOODGATE"
          value={
            compatibility
              ? `${compatibility.geyser_version} / ${compatibility.floodgate_version}`
              : "Vérification…"
          }
        />
      </div>
      {compatibilityError && (
        <div className="inline-error">
          Les catalogues officiels sont injoignables. L’installation est bloquée
          : aucune version récente ne sera choisie à l’aveugle.
        </div>
      )}
      <div className="folder-choice">
        <div>
          <strong>Où créer le nouveau serveur ?</strong>
          <small>{destination || "Choisis un dossier de destination"}</small>
        </div>
        <button className="button secondary" onClick={onChooseFolder}>
          Choisir un dossier
        </button>
      </div>
      <label className="eula-choice">
        <input
          type="checkbox"
          checked={eulaAccepted}
          onChange={(event) => setEulaAccepted(event.target.checked)}
        />
        <span>
          J’accepte le contrat de licence Minecraft pour ce nouveau serveur.
        </span>
      </label>
      <button
        className="button primary action-button"
        onClick={onInstall}
        disabled={busy || !destination || !eulaAccepted || !compatibility}
      >
        {busy
          ? "Installation et démarrage en cours…"
          : "Créer et démarrer le serveur"}
      </button>
      {progress && (
        <div className="operation-progress">
          <div className="progress-head">
            <span>{progress.step}</span>
            <b>{progress.percent}%</b>
          </div>
          <div className="progress-track">
            <i style={{ width: `${progress.percent}%` }} />
          </div>
        </div>
      )}
      {error && <div className="inline-error">{error}</div>}
      {result && (
        <div className="operation-success">
          <strong>Serveur préparé · {result.detail}</strong>
          <small>{result.path}</small>
          <span>
            Le serveur a confirmé son démarrage. Toute installation déjà
            présente est restée intacte.
          </span>
        </div>
      )}
    </div>
  );
}

function ReleaseItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="release-item">
      <small>{label}</small>
      <strong>{value}</strong>
    </div>
  );
}

function Spec({
  label,
  value,
  note,
}: {
  label: string;
  value: string;
  note: string;
}) {
  return (
    <div className="spec">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{note}</small>
    </div>
  );
}

function Home({
  system,
  systemError,
  localInstallation,
  localServerStatus,
  localStatusError,
  localActionMessage,
  onRefreshLocal,
  onStartLocal,
  onStopLocal,
  piInstallation,
  piServerStatus,
  piStatusError,
  piActionMessage,
  actionProgress,
  busy,
  onRefreshPi,
  onStartPi,
  onStopPi,
  onCheckPiUpdate,
  piUpdateCheck,
  piUpdateError,
  checkingPiUpdate,
  onInstall,
  onCompat,
  onBackup,
}: {
  system: SystemInfo | null;
  systemError: string;
  localInstallation: LocalInstallation | null;
  localServerStatus: LocalServerStatus | null;
  localStatusError: string;
  localActionMessage: string;
  onRefreshLocal: () => void;
  onStartLocal: () => void;
  onStopLocal: () => void;
  piInstallation: PiInstallation | null;
  piServerStatus: PiServerStatus | null;
  piStatusError: string;
  piActionMessage: string;
  actionProgress: string;
  busy: boolean;
  onRefreshPi: () => void;
  onStartPi: () => void;
  onStopPi: () => void;
  onCheckPiUpdate: () => void;
  piUpdateCheck: PiUpdateCheck | null;
  piUpdateError: string;
  checkingPiUpdate: boolean;
  onInstall: () => void;
  onCompat: () => void;
  onBackup: () => void;
}) {
  return (
    <>
      {localInstallation && (
        <section
          className="pi-dashboard"
          aria-labelledby="local-dashboard-title"
        >
          <div className="pi-dashboard-head">
            <div>
              <div className="eyebrow">SERVEUR LOCAL ENREGISTRÉ</div>
              <h2 id="local-dashboard-title">
                {localStatusError
                  ? "Vérification nécessaire"
                  : !localServerStatus
                    ? "Vérification du serveur…"
                    : localServerStatus.ready
                      ? "Ton monde est prêt"
                      : localServerStatus.running
                        ? "Le serveur démarre"
                        : "Serveur arrêté"}
              </h2>
              <p>
                {localServerStatus?.address || "Adresse à vérifier"} · dossier{" "}
                {localInstallation.serverPath}
              </p>
            </div>
            <div
              className={
                localServerStatus?.ready ? "pi-state ready" : "pi-state"
              }
            >
              <span />
              {localServerStatus?.ready
                ? "PRÊT"
                : localServerStatus?.running
                  ? "DÉMARRAGE"
                  : "HORS LIGNE"}
            </div>
          </div>
          <div className="pi-connect-grid">
            <div>
              <small>ORDINATEUR · JAVA</small>
              <strong>
                {localServerStatus?.address || "—"}:
                {localServerStatus?.java_port || "—"}
              </strong>
              <span>Minecraft → Multijoueur → Ajouter un serveur</span>
            </div>
            <div>
              <small>TÉLÉPHONE · TABLETTE · BEDROCK</small>
              <strong>{localServerStatus?.address || "—"}</strong>
              <span>
                Port : {localServerStatus?.bedrock_port || "—"} · Ajouter un
                serveur
              </span>
            </div>
          </div>
          <div className="pi-version-line">
            <strong>Version exacte installée</strong>
            <span>
              Java {localServerStatus?.minecraft_java || "à vérifier"} · Bedrock
              compatible {localServerStatus?.minecraft_bedrock || "à vérifier"}
            </span>
            {localServerStatus?.running && (
              <span>
                Joueurs · {localServerStatus.players || "état non communiqué"}
              </span>
            )}
          </div>
          <div className="pi-actions">
            {localServerStatus?.running ? (
              <button
                className="button secondary"
                onClick={onStopLocal}
                disabled={busy || Boolean(localStatusError)}
              >
                Arrêter proprement
              </button>
            ) : (
              <button
                className="button primary"
                onClick={onStartLocal}
                disabled={
                  busy || !localServerStatus || Boolean(localStatusError)
                }
              >
                {busy ? "Démarrage…" : "Démarrer le serveur"}
              </button>
            )}
            <button
              className="text-button"
              onClick={onRefreshLocal}
              disabled={busy}
            >
              Actualiser l’état et le journal
            </button>
          </div>
          {localActionMessage && (
            <p className="pi-action-message" aria-live="polite">
              {localActionMessage}
            </p>
          )}
          {localStatusError && (
            <p className="pi-error" role="alert">
              {localStatusError}
            </p>
          )}
          {localServerStatus?.log && (
            <details className="pi-log">
              <summary>Journal récent du serveur</summary>
              <pre>{localServerStatus.log}</pre>
            </details>
          )}
        </section>
      )}
      {piInstallation && (
        <section className="pi-dashboard" aria-labelledby="pi-dashboard-title">
          <div className="pi-dashboard-head">
            <div>
              <div className="eyebrow">SERVEUR RASPBERRY PI ENREGISTRÉ</div>
              <h2 id="pi-dashboard-title">
                {!piServerStatus && !piStatusError
                  ? "Vérification du Pi…"
                  : piServerStatus?.ready
                    ? "Ton monde est prêt"
                    : piServerStatus?.running
                      ? "Le serveur démarre"
                      : "État du serveur"}
              </h2>
              <p>
                {piServerStatus?.address || piInstallation.address} · dossier
                dédié conservé sur le Pi
              </p>
            </div>
            <div
              className={piServerStatus?.ready ? "pi-state ready" : "pi-state"}
            >
              <span />
              {!piServerStatus && !piStatusError
                ? "VÉRIFICATION"
                : piServerStatus?.ready
                  ? "PRÊT"
                  : piServerStatus?.running
                    ? "DÉMARRAGE"
                    : "HORS LIGNE"}
            </div>
          </div>
          <div className="pi-connect-grid">
            <div>
              <small>ORDINATEUR · JAVA</small>
              <strong>
                {piServerStatus?.address || piInstallation.address}:
                {piServerStatus?.java_port || 25565}
              </strong>
              <span>Dans Minecraft : Multijoueur → Ajouter un serveur</span>
            </div>
            <div>
              <small>TÉLÉPHONE · TABLETTE · BEDROCK</small>
              <strong>
                {piServerStatus?.address || piInstallation.address}
              </strong>
              <span>
                Port : {piServerStatus?.bedrock_port || 19132} · Ajouter un
                serveur
              </span>
            </div>
          </div>
          <div className="pi-version-line">
            <strong>Version exacte installée</strong>
            <span>
              Java {piServerStatus?.minecraft_java || "à vérifier"}
              {piServerStatus?.minecraft_bedrock
                ? ` · Bedrock ${piServerStatus.minecraft_bedrock}`
                : ""}
            </span>
            <span>
              Paper{" "}
              {piServerStatus?.paper_build_id
                ? `build ${piServerStatus.paper_build_id}`
                : "à vérifier"}
              {piServerStatus?.geyser_version
                ? ` · Geyser ${piServerStatus.geyser_version} build ${piServerStatus.geyser_build_id}`
                : ""}
              {piServerStatus?.floodgate_version
                ? ` · Floodgate ${piServerStatus.floodgate_version} build ${piServerStatus.floodgate_build_id}`
                : ""}
            </span>
          </div>
          <div className="pi-checks">
            <span className={piServerStatus?.java_port_open ? "ok" : ""}>
              Java{" "}
              {piServerStatus?.java_port_open
                ? "· disponible"
                : "· non vérifié"}
            </span>
            <span className={piServerStatus?.bedrock_port_open ? "ok" : ""}>
              Bedrock{" "}
              {piServerStatus?.bedrock_port_open
                ? "· disponible"
                : "· non vérifié"}
            </span>
            <span className={piServerStatus?.autostart_configured ? "ok" : ""}>
              Après redémarrage{" "}
              {piServerStatus?.autostart_configured
                ? "· automatique"
                : "· non configuré"}
            </span>
            <div className="pi-actions">
              {!piServerStatus?.running && (
                <button
                  className="button primary"
                  onClick={onStartPi}
                  disabled={busy}
                >
                  {busy ? "Vérification…" : "Démarrer le serveur"}
                </button>
              )}
              {piServerStatus?.running && (
                <button
                  className="button secondary"
                  onClick={onStopPi}
                  disabled={busy}
                >
                  {busy ? "Arrêt en cours…" : "Arrêter le serveur"}
                </button>
              )}
              <button
                className="text-button"
                onClick={onRefreshPi}
                disabled={busy}
              >
                {busy ? "Lecture…" : "Actualiser l’état et le journal"}
              </button>
              <button
                className="text-button"
                onClick={onCheckPiUpdate}
                disabled={checkingPiUpdate || busy}
              >
                {checkingPiUpdate
                  ? "Vérification des versions…"
                  : "Vérifier la compatibilité des mises à jour"}
              </button>
            </div>
          </div>
          {actionProgress && (
            <p className="pi-action-progress" aria-live="polite">
              {actionProgress}
            </p>
          )}
          {piActionMessage && (
            <p className="pi-action-message" aria-live="polite">
              {piActionMessage}
            </p>
          )}
          {(piUpdateCheck || piUpdateError) && (
            <div
              className={
                piUpdateCheck?.ready
                  ? "pi-update-check ready"
                  : "pi-update-check"
              }
              aria-live="polite"
            >
              <strong>
                {piUpdateError
                  ? "Vérification impossible"
                  : piUpdateCheck?.update_available
                    ? "Une mise à jour compatible est disponible"
                    : piUpdateCheck?.ready
                      ? "Les versions sont à jour"
                      : "Mise à jour automatique bloquée"}
              </strong>
              {piUpdateError ? (
                <p>{piUpdateError}</p>
              ) : piUpdateCheck ? (
                <>
                  <p>
                    Installé : Java {piUpdateCheck.current_java}, Bedrock{" "}
                    {piUpdateCheck.current_bedrock} · Disponible : Java{" "}
                    {piUpdateCheck.latest_java}, Bedrock{" "}
                    {piUpdateCheck.latest_bedrock}.
                  </p>
                  <p>
                    Pont multiplateforme installé : Geyser{" "}
                    {piUpdateCheck.current_geyser} / Floodgate{" "}
                    {piUpdateCheck.current_floodgate} · versions officielles
                    actuelles : Geyser {piUpdateCheck.latest_geyser} / Floodgate{" "}
                    {piUpdateCheck.latest_floodgate}.
                  </p>
                  {piUpdateCheck.reasons.length > 0 && (
                    <ul>
                      {piUpdateCheck.reasons.map((reason) => (
                        <li key={reason}>{reason}</li>
                      ))}
                    </ul>
                  )}
                  <small>
                    {piUpdateCheck.ready
                      ? piUpdateCheck.update_available
                        ? "La mise à jour groupée sera appliquée au prochain démarrage du serveur, avec une sauvegarde avant changement."
                        : "Aucune mise à jour à appliquer pour le moment."
                      : "Aucune mise à jour automatique ne sera lancée tant que ces points ne sont pas vérifiés."}
                  </small>
                </>
              ) : null}
            </div>
          )}
          {piStatusError && <p className="pi-error">{piStatusError}</p>}
          {piServerStatus &&
            !piServerStatus.autostart_configured &&
            !piStatusError && (
              <p className="pi-help">
                Le Pi ne confirme pas le démarrage automatique après
                redémarrage. Le serveur peut être relancé ici lorsqu’il est
                éteint.
              </p>
            )}
          {piServerStatus && !piServerStatus.ready && !piStatusError && (
            <p className="pi-help">
              {piServerStatus.running
                ? "Minecraft n’a pas encore confirmé son démarrage. Ouvre le journal pour voir la cause."
                : "Le Pi ne répond pas ou le serveur est arrêté. Vérifie qu’il est allumé et connecté au même réseau, puis actualise."}
            </p>
          )}
          {piServerStatus?.log && (
            <details className="pi-log">
              <summary>Journal récent · utile en cas de problème</summary>
              <pre>{piServerStatus.log}</pre>
            </details>
          )}
          <p className="pi-network-note">
            Adresse actuelle du Pi sur ton réseau. Pour qu’elle ne change
            jamais, une réservation dans la box doit être confirmée. Pour jouer
            depuis l’extérieur, il faut aussi configurer l’accès du routeur.
          </p>
        </section>
      )}
      <div className="hero">
        <div className="hero-copy">
          <div className="hero-kicker">
            <span /> {piInstallation ? "SERVEUR SUIVI" : "PARCOURS GUIDÉ"}
          </div>
          <h2>
            Joue ensemble.
            <br />
            <span>Sans perdre ton monde.</span>
          </h2>
          <p>
            Un serveur Java peut aussi accueillir les joueurs Bedrock. Prépare
            une installation neuve, puis sauvegarde ou restaure ton monde dans
            un autre dossier.
          </p>
          <div className="hero-actions">
            <button className="button primary" onClick={onInstall}>
              Vérifier mon installation <span>→</span>
            </button>
            <button className="text-button" onClick={onCompat}>
              Voir ce qui est compatible
            </button>
          </div>
        </div>
        <div
          className="block-scene"
          aria-label="Illustration de paysage en blocs"
        >
          <div className="sun" />
          <div className="cloud cloud-a" />
          <div className="cloud cloud-b" />
          <div className="mountain m-back" />
          <div className="mountain m-front" />
          <div className="hill" />
          <div className="tree tree-a">
            <i />
            <b />
          </div>
          <div className="tree tree-b">
            <i />
            <b />
          </div>
          <div className="scene-ground" />
          <div className="scene-caption">UN MONDE · TOUS TES APPAREILS</div>
        </div>
      </div>
      <div className="section-label">
        <span>TON PARCOURS SIMPLE</span>
        <i />
      </div>
      <div className="quick-grid">
        <button className="quick-card" onClick={onInstall}>
          <div className="quick-icon green">＋</div>
          <strong>Préparer le serveur</strong>
          <p>Vérifier la machine avant la préparation de l’installation.</p>
          <span className="quick-link">
            Démarrer <b>↗</b>
          </span>
        </button>
        <button className="quick-card" onClick={onBackup}>
          <div className="quick-icon ochre">▤</div>
          <strong>Garder mon monde</strong>
          <p>
            Voir le parcours pour déplacer une copie sur un nouvel ordinateur ou
            un Pi.
          </p>
          <span className="quick-link">
            Voir les sauvegardes <b>↗</b>
          </span>
        </button>
        <button className="quick-card" onClick={onCompat}>
          <div className="quick-icon blue">◈</div>
          <strong>Comprendre la compatibilité</strong>
          <p>
            Voir quels appareils, packs et extensions peuvent jouer ensemble.
          </p>
          <span className="quick-link">
            Lire le guide <b>↗</b>
          </span>
        </button>
      </div>
      <div className="bottom-row">
        <div className="machine-summary">
          <span className="mini-machine">▣</span>
          <div>
            <small>CET APPAREIL</small>
            <strong>
              {system
                ? `${system.board || system.os} · ${system.memory_gb || "?"} Go mémoire`
                : systemError
                  ? "Caractéristiques indisponibles"
                  : "Lecture de l’appareil…"}
            </strong>
          </div>
          <span className="summary-arrow">↗</span>
        </div>
        <div className="safety-note">
          <span>✳</span>
          <p>
            <strong>
              L’installation ne remplace jamais le serveur actuel.
            </strong>
            <br />
            Elle crée une nouvelle copie; les mises à jour du serveur existant
            restent bloquées tant que leur compatibilité n’est pas vérifiée.
          </p>
        </div>
      </div>
    </>
  );
}

function Compatibility({
  report,
  error,
}: {
  report: CompatibilityReport | null;
  error: string;
}) {
  const rows = [
    ["PC / Mac Java", "Profil Java", "Paper · version listée"],
    ["Windows Bedrock", "Profil Bedrock", "Geyser · version listée"],
    ["Téléphones / tablettes", "Profil Bedrock", "Geyser · version listée"],
    [
      "Consoles",
      "Avec étape en plus",
      "Leur menu limite les serveurs externes",
    ],
    [
      "Extensions serveur",
      "À vérifier",
      "Le profil doit confirmer leur compatibilité",
    ],
    [
      "Mods qui exigent un mod client",
      "Non universels",
      "Pas disponibles sur consoles et mobiles",
    ],
    [
      "Packs de textures",
      "Selon le format",
      "Un pack Bedrock dédié est parfois nécessaire",
    ],
  ];
  return (
    <div className="content-column">
      <div className="compat-banner">
        <div className="compat-emblem">
          J<span>＋</span>B
        </div>
        <div>
          <strong>
            Le principe : un serveur Java, une passerelle Bedrock.
          </strong>
          <p>
            Les appareils Bedrock peuvent rejoindre le même monde grâce à Geyser
            et Floodgate. Les versions affichées ci-dessous viennent des
            catalogues officiels consultés à cet instant; elles ne prouvent pas
            la compatibilité de tous les plugins ou packs de ton ancien monde.
          </p>
        </div>
      </div>
      <div className="table-card">
        <div className="table-heading">
          <div>
            <div className="eyebrow">LE MONDE COMMUN</div>
            <h3>Appareils et limites du profil</h3>
          </div>
          <span>Compatibilité annoncée</span>
        </div>
        {rows.map((r) => (
          <div className="compat-row" key={r[0]}>
            <strong>{r[0]}</strong>
            <span className={r[1] === "Oui" ? "badge yes" : "badge careful"}>
              {r[1]}
            </span>
            <small>{r[2]}</small>
          </div>
        ))}
      </div>
      <div className="release-summary">
        <div className="eyebrow">DERNIER CONTRÔLE DES CATALOGUES</div>
        {report ? (
          <>
            <div className="release-grid">
              <ReleaseItem label="PAPER" value={report.paper_version} />
              <ReleaseItem label="GEYSER" value={report.geyser_version} />
              <ReleaseItem label="FLOODGATE" value={report.floodgate_version} />
              <ReleaseItem
                label="CLIENTS BEDROCK"
                value={report.bedrock_versions}
              />
            </div>
            <small>
              {report.source} · vérifié{" "}
              {new Date(report.checked_at).toLocaleString("fr-FR")}
            </small>
          </>
        ) : error ? (
          <p className="inline-error">
            Vérification impossible. Toute installation qui dépend des versions
            récentes reste bloquée.
          </p>
        ) : (
          <p>Lecture des versions officielles…</p>
        )}
      </div>
      <div className="terracotta-note">
        <div className="terracotta-mark">!</div>
        <div>
          <div className="eyebrow">ERREUR TERRACOTTA · IPAD</div>
          <strong>Bedrock 1.26.62 · InitialConnection-149</strong>
          <p>
            L’erreur InitialConnection-149 seule ne permet pas d’identifier la
            cause. Ce panneau ne lit pas encore les journaux du serveur existant
            et ne prétend donc pas l’avoir corrigé. Pour l’installation neuve,
            l’application affiche et règle les versions et ports utilisés.
          </p>
        </div>
      </div>
      <div className="notice amber">
        <span>!</span>
        <p>
          <strong>
            Les mises à jour coordonnées sont disponibles pour les serveurs Pi
            gérés.
          </strong>
          <br />
          Après arrêt, le bouton « Démarrer » vérifie Java, Bedrock, Paper,
          Geyser et Floodgate. Il applique le groupe uniquement si le profil est
          géré, sauvegardé et vérifié au redémarrage; sinon il conserve la
          version présente. La mise à jour des serveurs locaux Mac/Windows n’est
          pas encore prise en charge.
        </p>
      </div>
      <div className="source-note">
        Les consoles ont des limites d’accès aux serveurs privés. Les mods côté
        client et certains packs ne sont pas universels. Aucune application ne
        peut rendre ces formats compatibles par simple mise à jour.
      </div>
    </div>
  );
}

function Backups({
  source,
  output,
  stopped,
  setStopped,
  archive,
  destination,
  busy,
  progress,
  result,
  error,
  onChooseSource,
  onChooseArchive,
  onChooseDestination,
  onBackup,
  onRestore,
}: {
  source: string;
  output: string;
  stopped: boolean;
  setStopped: (stopped: boolean) => void;
  archive: string;
  destination: string;
  busy: boolean;
  progress: ProgressMessage | null;
  result: OperationResult | null;
  error: string;
  onChooseSource: () => void;
  onChooseArchive: () => void;
  onChooseDestination: () => void;
  onBackup: () => void;
  onRestore: () => void;
}) {
  return (
    <div className="content-column">
      <div className="backup-hero">
        <div className="backup-illustration">▤</div>
        <div>
          <div className="eyebrow">TON MONDE T’APPARTIENT</div>
          <h2>Emporte tes constructions.</h2>
          <p>
            Une sauvegarde complète rassemble le monde, les réglages et la liste
            des extensions pour retrouver ta partie ailleurs.
          </p>
        </div>
      </div>
      <div className="operation-card">
        <div className="eyebrow">01 · SAUVEGARDER</div>
        <h3>Créer une copie complète</h3>
        <p>
          Arrête le serveur avant la copie. L’archive contient les mondes,
          réglages, plugins et données disponibles dans ce dossier.
        </p>
        <div className="folder-choice">
          <div>
            <strong>Dossier du serveur</strong>
            <small>{source || "Choisis le dossier Paper du serveur"}</small>
          </div>
          <button className="button secondary" onClick={onChooseSource}>
            Choisir
          </button>
        </div>
        <label className="eula-choice">
          <input
            type="checkbox"
            checked={stopped}
            onChange={(event) => setStopped(event.target.checked)}
          />
          <span>J’ai arrêté le serveur avant de commencer la sauvegarde.</span>
        </label>
        <button
          className="button primary action-button"
          disabled={busy || !source || !stopped}
          onClick={onBackup}
        >
          {busy ? "Sauvegarde en cours…" : "Créer et vérifier la sauvegarde"}
        </button>
        {output && <small className="path-note">Destination : {output}</small>}
      </div>
      <div className="operation-card">
        <div className="eyebrow">02 · RESTAURER</div>
        <h3>Reprendre un monde sauvegardé</h3>
        <p>
          La restauration crée une nouvelle copie dans un dossier distinct. Elle
          ne déplace ni n’écrase la sauvegarde ni le serveur existant.
        </p>
        <div className="folder-choice">
          <div>
            <strong>Archive à restaurer</strong>
            <small>{archive || "Choisis une archive .tar.gz"}</small>
          </div>
          <button className="button secondary" onClick={onChooseArchive}>
            Choisir
          </button>
        </div>
        <div className="folder-choice">
          <div>
            <strong>Emplacement de la nouvelle copie</strong>
            <small>{destination || "Choisis un dossier de destination"}</small>
          </div>
          <button className="button secondary" onClick={onChooseDestination}>
            Choisir
          </button>
        </div>
        <button
          className="button primary action-button"
          disabled={busy || !archive || !destination}
          onClick={onRestore}
        >
          {busy ? "Vérification en cours…" : "Vérifier et restaurer"}
        </button>
      </div>
      {progress && (
        <OperationStatus progress={progress} result={result} error={error} />
      )}
      {!progress && (result || error) && (
        <OperationStatus progress={null} result={result} error={error} />
      )}
      <div className="source-note">
        L’archive complète peut contenir des extensions sous licence. Monde
        Ensemble copie les fichiers sélectionnés; il ne récupère ni ne remplace
        les licences des extensions.
      </div>
    </div>
  );
}

function OperationStatus({
  progress,
  result,
  error,
}: {
  progress: ProgressMessage | null;
  result: OperationResult | null;
  error: string;
}) {
  return (
    <div className={error ? "inline-error" : "operation-success"}>
      {progress && (
        <>
          <div className="progress-head">
            <span>{progress.step}</span>
            <b>{progress.percent}%</b>
          </div>
          <div className="progress-track">
            <i style={{ width: `${progress.percent}%` }} />
          </div>
        </>
      )}
      {error && <p>{error}</p>}
      {result && (
        <>
          <strong>{result.detail}</strong>
          <small>{result.path}</small>
          {result.sha256 && <small>SHA-256 : {result.sha256}</small>}
        </>
      )}
    </div>
  );
}

function Advanced({
  system,
  localInstallation,
  localServerStatus,
  piInstallation,
  piServerStatus,
  piUpdateCheck,
  piUpdateError,
  busy,
  onRefresh,
  onCheckUpdates,
}: {
  system: SystemInfo | null;
  localInstallation: LocalInstallation | null;
  localServerStatus: LocalServerStatus | null;
  piInstallation: PiInstallation | null;
  piServerStatus: PiServerStatus | null;
  piUpdateCheck: PiUpdateCheck | null;
  piUpdateError: string;
  busy: boolean;
  onRefresh: () => void;
  onCheckUpdates: () => void;
}) {
  return (
    <div className="content-column">
      <div className="advanced-intro">
        <div className="advanced-mark">⚙</div>
        <div>
          <div className="eyebrow">MODE AVANCÉ</div>
          <h2>État précis, mêmes protections.</h2>
          <p>
            Ici tu vois les versions, les ports, le journal et les raisons qui
            autorisent ou reportent une mise à jour.
          </p>
        </div>
      </div>
      <div className="advanced-list">
        <div>
          <span>01</span>
          <p>
            <strong>Machine de cette application</strong>
            <small>
              {system
                ? `${system.os} ${system.version} · ${system.architecture} · mémoire ${system.memory_gb ?? "inconnue"} Go · disque libre ${system.free_disk_gb ?? "inconnu"} Go`
                : "Lecture en cours"}
            </small>
          </p>
          <b>Lecture seule</b>
        </div>
        <div>
          <span>02</span>
          <p>
            <strong>Serveur géré</strong>
            <small>
              {localInstallation
                ? `Local · ${localInstallation.serverPath}`
                : piInstallation
                  ? `${piInstallation.username}@${piInstallation.host} · ${piInstallation.serverFolder}`
                  : "Aucun serveur enregistré dans cette application"}
            </small>
          </p>
          <b>
            {localServerStatus?.ready || piServerStatus?.ready
              ? "Java + Bedrock prêts"
              : "État à vérifier"}
          </b>
        </div>
        <div>
          <span>03</span>
          <p>
            <strong>Versions installées</strong>
            <small>
              Java{" "}
              {localServerStatus?.minecraft_java ||
                piServerStatus?.minecraft_java ||
                "inconnue"}{" "}
              · Bedrock{" "}
              {localServerStatus?.minecraft_bedrock ||
                piServerStatus?.minecraft_bedrock ||
                "inconnue"}{" "}
              · Paper build {piServerStatus?.paper_build_id || "inconnu"} ·
              Geyser {piServerStatus?.geyser_version || "inconnu"} · Floodgate{" "}
              {piServerStatus?.floodgate_version || "inconnu"} · adresse{" "}
              {localServerStatus?.address ||
                piServerStatus?.address ||
                piInstallation?.address ||
                "inconnue"}{" "}
              · ports{" "}
              {localServerStatus?.java_port ?? piServerStatus?.java_port ?? "—"}{" "}
              /{" "}
              {localServerStatus?.bedrock_port ??
                piServerStatus?.bedrock_port ??
                "—"}
            </small>
          </p>
          <b>
            {piServerStatus?.autostart_configured
              ? "Démarrage automatique actif"
              : "Démarrage automatique non confirmé"}
          </b>
        </div>
        <div>
          <span>04</span>
          <p>
            <strong>Règle de mise à jour</strong>
            <small>
              Un serveur Pi géré est sauvegardé puis mis à jour en groupe au
              démarrage seulement si son profil, ses extensions gérées, le
              runtime et les versions officielles passent les contrôles.
            </small>
          </p>
          <b>Sinon, version présente conservée</b>
        </div>
      </div>
      {piInstallation && (
        <div className="operation-card advanced-log-card">
          <div className="eyebrow">OUTILS DU SERVEUR PI</div>
          <div className="pi-actions">
            <button
              className="button secondary"
              onClick={onRefresh}
              disabled={busy}
            >
              Actualiser le journal
            </button>
            <button
              className="button secondary"
              onClick={onCheckUpdates}
              disabled={busy}
            >
              Vérifier les versions compatibles
            </button>
          </div>
          {piUpdateError && <p className="pi-error">{piUpdateError}</p>}
          {piUpdateCheck && (
            <p className="path-note">
              Version actuelle : Java {piUpdateCheck.current_java}, Bedrock{" "}
              {piUpdateCheck.current_bedrock}, Geyser{" "}
              {piUpdateCheck.current_geyser}, Floodgate{" "}
              {piUpdateCheck.current_floodgate}.{" "}
              {piUpdateCheck.update_available
                ? "Nouvelle pile officielle détectée."
                : "Aucune nouvelle pile détectée."}
              {piUpdateCheck.reasons.length > 0 &&
                ` Contrôle bloquant : ${piUpdateCheck.reasons.join(" ")}`}
            </p>
          )}
          {piServerStatus?.log ? (
            <details className="pi-log" open>
              <summary>Journal récent du serveur</summary>
              <pre>{piServerStatus.log}</pre>
            </details>
          ) : (
            <p className="path-note">
              Le journal apparaîtra ici après son actualisation.
            </p>
          )}
        </div>
      )}
      <div className="notice amber">
        <span>!</span>
        <p>
          <strong>
            Les mods et packs personnalisés ne sont pas ajoutés automatiquement.
          </strong>
          <br />
          Cette version les bloque par prudence pendant une mise à jour; leur
          compatibilité doit être vérifiée avant de les gérer ici. Les clients
          Bedrock, consoles et packs Java ne partagent pas tous les mêmes
          formats.
        </p>
      </div>
    </div>
  );
}

export default App;
