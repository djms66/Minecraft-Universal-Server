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
type PluginTarget = "local" | "pi";
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
type PaperPluginSuggestion = {
  project_id: string;
  title: string;
  description: string;
  version: string;
  downloads: number;
  project_url: string;
  required_dependencies: number;
};
type PaperPluginInstallResult = {
  server_path: string;
  backup_path: string;
  backup_sha256: string;
  plugin_name: string;
  plugin_version: string;
  sha512: string;
};
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
  previousServerFolder?: string;
  address: string;
};
type LocalInstallation = { serverPath: string; previousServerPath?: string };
type LocalStartResult = { server_path: string; message: string };
type LocalServerStatus = {
  running: boolean;
  ready: boolean;
  java_port_open: boolean;
  minecraft_java: string;
  minecraft_bedrock: string;
  java_port: number;
  bedrock_port: number;
  bedrock_tcp_port_open: boolean;
  bedrock_udp_port_open: boolean;
  address: string;
  players: string;
  log: string;
};
type LocalWhitelistStatus = { enabled: boolean; entries: string };
type PiWhitelistStatus = { enabled: boolean; entries: string };
type BedrockPackInstallResult = {
  name: string;
  filename: string;
  minimum_engine_version: string;
  sha256: string;
  destination: string;
};
type GameSettings = {
  motd: string;
  gamemode: "survival" | "creative" | "adventure" | "spectator";
  difficulty: "peaceful" | "easy" | "normal" | "hard";
  max_players: number;
  view_distance: number;
  simulation_distance: number;
  pvp: boolean;
  allow_nether: boolean;
};
type PiServerStatus = {
  running: boolean;
  ready: boolean;
  java_port_open: boolean;
  bedrock_tcp_port_open: boolean;
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
  network_interface: string;
  mac_address: string;
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
  const [target, setTarget] = useState<"this" | "pi">("pi");
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
  const [piWhitelistStatus, setPiWhitelistStatus] =
    useState<PiWhitelistStatus | null>(null);
  const [piPlayerName, setPiPlayerName] = useState("");
  const [piPlayerPlatform, setPiPlayerPlatform] = useState<"java" | "bedrock">(
    "java",
  );
  const [piPlayerError, setPiPlayerError] = useState("");
  const [piPlayerMessage, setPiPlayerMessage] = useState("");
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
  const [pluginInstallError, setPluginInstallError] = useState("");
  const [pluginInstallResult, setPluginInstallResult] =
    useState<PaperPluginInstallResult | null>(null);
  const [localWhitelistStatus, setLocalWhitelistStatus] =
    useState<LocalWhitelistStatus | null>(null);
  const [localPlayerName, setLocalPlayerName] = useState("");
  const [localPlayerPlatform, setLocalPlayerPlatform] = useState<
    "java" | "bedrock"
  >("java");
  const [localUpdateCheck, setLocalUpdateCheck] =
    useState<PiUpdateCheck | null>(null);
  const [localUpdateError, setLocalUpdateError] = useState("");
  const [checkingLocalUpdate, setCheckingLocalUpdate] = useState(false);
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

  const createPiBackup = async () => {
    if (!piInstallation) return;
    resetOperation();
    setBusy(true);
    try {
      const output = await save({
        defaultPath: `Monde-Ensemble-Sauvegarde-Pi-${new Date().toISOString().slice(0, 10)}.tar.gz`,
        filters: [{ name: "Archive compressée", extensions: ["gz"] }],
      });
      if (!output) return;
      const result = await invoke<OperationResult>("backup_pi_server", {
        host: piInstallation.host,
        username: piInstallation.username,
        serverFolder: piInstallation.serverFolder,
        outputFile: output,
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
      if (localInstallation) {
        const currentStatus = await invoke<LocalServerStatus>(
          "inspect_local_server",
          { serverPath: localInstallation.serverPath },
        ).catch(() => null);
        if (currentStatus) setLocalServerStatus(currentStatus);
        if (
          currentStatus &&
          (currentStatus.running ||
            currentStatus.java_port_open ||
            currentStatus.bedrock_tcp_port_open ||
            currentStatus.bedrock_udp_port_open)
        ) {
          throw new Error(
            "Arrête d’abord le serveur local depuis son tableau de bord. Il reste sélectionné et aucun monde n’a été restauré.",
          );
        }
      }
      const result = await invoke<OperationResult>("restore_server", {
        archive: restoreArchive,
        destinationRoot: restoreDestination,
        progress: progressChannel(),
      });
      setOperationResult(result);
      try {
        const restoredStatus = await invoke<LocalServerStatus>(
          "inspect_local_server",
          { serverPath: result.path },
        );
        if (
          restoredStatus.running ||
          restoredStatus.java_port_open ||
          restoredStatus.bedrock_tcp_port_open ||
          restoredStatus.bedrock_udp_port_open
        ) {
          setOperationResult({
            ...result,
            detail: `${result.detail}. Les ports Minecraft sont occupés; la copie restaurée n’a pas été sélectionnée.`,
          });
          return;
        }
        setLocalInstallation({
          serverPath: result.path,
          previousServerPath: localInstallation?.serverPath,
        });
        setLocalServerStatus(restoredStatus);
        setLocalStatusError("");
        setLocalActionMessage(
          "Le monde restauré est chargé dans le tableau de bord. Démarre-le quand tu veux jouer; l’installation précédente reste accessible dans le mode avancé.",
        );
        setPage("home");
      } catch {
        setOperationResult({
          ...result,
          detail: `${result.detail}. Le dossier est restauré, mais son profil ne peut pas être chargé dans le tableau de bord. Le chemin ci-dessus permet de le retrouver.`,
        });
      }
    } catch (error) {
      setOperationError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const restoreWorldToPi = async () => {
    if (!piInstallation || !restoreArchive) return;
    resetOperation();
    setBusy(true);
    try {
      const currentStatus = await invoke<PiServerStatus>("inspect_pi_server", {
        host: piInstallation.host,
        username: piInstallation.username,
        serverFolder: piInstallation.serverFolder,
      });
      setPiServerStatus(currentStatus);
      if (
        currentStatus.running ||
        currentStatus.java_port_open ||
        currentStatus.bedrock_tcp_port_open ||
        currentStatus.bedrock_port_open
      ) {
        throw new Error(
          "Arrête d’abord le serveur Pi. Une restauration est bloquée tant qu’un serveur Minecraft utilise les ports.",
        );
      }
      const result = await invoke<OperationResult>("restore_pi_server", {
        host: piInstallation.host,
        username: piInstallation.username,
        archive: restoreArchive,
        progress: progressChannel(),
      });
      setOperationResult(result);
      const installation = {
        ...piInstallation,
        serverFolder: result.path,
        previousServerFolder: piInstallation.serverFolder,
      };
      try {
        const restoredStatus = await invoke<PiServerStatus>(
          "inspect_pi_server",
          {
            host: installation.host,
            username: installation.username,
            serverFolder: installation.serverFolder,
          },
        );
        setPiInstallation(installation);
        setPiServerStatus(restoredStatus);
        setPiStatusError("");
        setPiActionMessage(
          "La copie restaurée est sélectionnée. Elle est arrêtée et prête à vérifier avant de jouer; ton serveur précédent reste disponible dans le mode avancé.",
        );
        setPage("home");
      } catch {
        setOperationResult({
          ...result,
          detail: `${result.detail}. Le dossier a été créé, mais son tableau de bord n’a pas pu être rechargé. Nom du dossier : ${result.path}`,
        });
      }
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
      "Vérifie le support : " +
        device.name +
        ", " +
        device.size +
        ", identifiant " +
        device.id +
        ". Il sera entièrement effacé pour préparer le Raspberry Pi. Débranche les autres cartes et disques externes avant de continuer. Confirmer l’effacement ?",
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
              ? "Connexion au Pi préparé…"
              : `Je cherche encore ton Pi… tentative ${attempt + 1}/12`,
          percent: Math.min(30, 8 + attempt * 2),
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
      if (status.running) {
        try {
          setPiWhitelistStatus(
            await invoke<PiWhitelistStatus>("pi_whitelist_status", {
              host: piInstallation.host,
              username: piInstallation.username,
              serverFolder: piInstallation.serverFolder,
            }),
          );
          setPiPlayerError("");
        } catch (error) {
          setPiWhitelistStatus(null);
          setPiPlayerError(String(error));
        }
      } else {
        setPiWhitelistStatus(null);
      }
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

  const managePiPlayer = async (action: "add" | "remove") => {
    if (!piInstallation) return;
    setBusy(true);
    setPiPlayerError("");
    setPiPlayerMessage("Mise à jour de la liste des joueurs…");
    try {
      const response = await invoke<string>("change_pi_whitelist", {
        host: piInstallation.host,
        username: piInstallation.username,
        serverFolder: piInstallation.serverFolder,
        platform: piPlayerPlatform,
        action,
        playerName: piPlayerName,
      });
      setPiPlayerMessage(response || "La liste des joueurs a été actualisée.");
      setPiWhitelistStatus(
        await invoke<PiWhitelistStatus>("pi_whitelist_status", {
          host: piInstallation.host,
          username: piInstallation.username,
          serverFolder: piInstallation.serverFolder,
        }),
      );
      setPiPlayerName("");
    } catch (error) {
      setPiPlayerError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const preparePiPlayerManagement = async () => {
    if (!piInstallation) return;
    setBusy(true);
    setPiPlayerError("");
    setPiPlayerMessage("Préparation des réglages protégés…");
    try {
      setPiPlayerMessage(
        await invoke<string>("prepare_pi_player_management", {
          host: piInstallation.host,
          username: piInstallation.username,
          serverFolder: piInstallation.serverFolder,
        }),
      );
      setPiWhitelistStatus(null);
    } catch (error) {
      setPiPlayerError(String(error));
      setPiPlayerMessage("");
    } finally {
      setBusy(false);
    }
  };

  const setPiWhitelist = async (enabled: boolean) => {
    if (!piInstallation) return;
    setBusy(true);
    setPiPlayerError("");
    try {
      setPiPlayerMessage(
        await invoke<string>("set_pi_whitelist_enabled", {
          host: piInstallation.host,
          username: piInstallation.username,
          serverFolder: piInstallation.serverFolder,
          enabled,
        }),
      );
      setPiWhitelistStatus(
        await invoke<PiWhitelistStatus>("pi_whitelist_status", {
          host: piInstallation.host,
          username: piInstallation.username,
          serverFolder: piInstallation.serverFolder,
        }),
      );
    } catch (error) {
      setPiPlayerError(String(error));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if ((page === "home" || page === "advanced") && piInstallation)
      void refreshPiStatus(false);
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

  const refreshLocalWhitelist = async () => {
    if (!localInstallation || !localServerStatus?.running) {
      setLocalWhitelistStatus(null);
      return;
    }
    try {
      setLocalWhitelistStatus(
        await invoke<LocalWhitelistStatus>("local_whitelist_status", {
          serverPath: localInstallation.serverPath,
        }),
      );
    } catch (error) {
      setLocalStatusError(String(error));
    }
  };

  const manageLocalPlayer = async (action: "add" | "remove") => {
    if (!localInstallation) return;
    setBusy(true);
    setLocalStatusError("");
    setLocalActionMessage("Mise à jour de la liste des joueurs…");
    try {
      const response = await invoke<string>("change_local_whitelist", {
        serverPath: localInstallation.serverPath,
        platform: localPlayerPlatform,
        action,
        playerName: localPlayerName,
      });
      setLocalActionMessage(
        response || "La liste des joueurs a été actualisée.",
      );
      setLocalWhitelistStatus(
        await invoke<LocalWhitelistStatus>("local_whitelist_status", {
          serverPath: localInstallation.serverPath,
        }),
      );
      setLocalPlayerName("");
    } catch (error) {
      setLocalStatusError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const setLocalWhitelist = async (enabled: boolean) => {
    if (!localInstallation) return;
    setBusy(true);
    setLocalStatusError("");
    setLocalActionMessage("Modification de la protection du serveur…");
    try {
      setLocalActionMessage(
        await invoke<string>("set_local_whitelist_enabled", {
          serverPath: localInstallation.serverPath,
          enabled,
        }),
      );
      setLocalWhitelistStatus(
        await invoke<LocalWhitelistStatus>("local_whitelist_status", {
          serverPath: localInstallation.serverPath,
        }),
      );
    } catch (error) {
      setLocalStatusError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const checkLocalUpdate = async () => {
    if (!localInstallation) return;
    setCheckingLocalUpdate(true);
    setLocalUpdateError("");
    setLocalUpdateCheck(null);
    try {
      const report = await invoke<PiUpdateCheck>("check_local_update", {
        serverPath: localInstallation.serverPath,
      });
      setLocalUpdateCheck(report);
    } catch (error) {
      setLocalUpdateError(String(error));
    } finally {
      setCheckingLocalUpdate(false);
    }
  };

  useEffect(() => {
    if ((page === "home" || page === "advanced") && localInstallation)
      void refreshLocalStatus();
    // The saved local path is the only input to this read-only status refresh.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page, localInstallation?.serverPath]);

  useEffect(() => {
    if (page === "home" && localServerStatus?.running) {
      void refreshLocalWhitelist();
    } else {
      setLocalWhitelistStatus(null);
    }
    // RCON refresh is read-only; the saved local path is its only target.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page, localServerStatus?.running, localInstallation?.serverPath]);

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
      const result = await invoke<LocalStartResult>("start_local_server", {
        serverPath: localInstallation.serverPath,
        progress: progressChannel(),
      });
      if (result.server_path !== localInstallation.serverPath) {
        setLocalInstallation({
          serverPath: result.server_path,
          previousServerPath: localInstallation.previousServerPath,
        });
      }
      setLocalActionMessage(result.message);
      setLocalServerStatus(
        await invoke<LocalServerStatus>("inspect_local_server", {
          serverPath: result.server_path,
        }),
      );
    } catch (error) {
      setLocalStatusError(String(error));
    } finally {
      setOperationProgress(null);
      setBusy(false);
    }
  };

  const installPaperPlugin = async (
    target: PluginTarget,
    projectId: string,
    versionNumber: string,
  ) => {
    if (
      target === "local" &&
      (!localInstallation || localServerStatus?.running)
    )
      return;
    if (target === "pi" && (!piInstallation || piServerStatus?.running)) return;
    setBusy(true);
    setPluginInstallError("");
    setPluginInstallResult(null);
    setOperationProgress({
      step: "Sauvegarde et préparation d’une copie avec le plugin…",
      percent: 2,
    });
    try {
      const result =
        target === "local"
          ? await invoke<PaperPluginInstallResult>(
              "install_paper_plugin_copy",
              {
                serverPath: localInstallation!.serverPath,
                projectId,
                versionNumber,
                progress: progressChannel(),
              },
            )
          : await invoke<PaperPluginInstallResult>(
              "install_pi_paper_plugin_copy",
              {
                host: piInstallation!.host,
                username: piInstallation!.username,
                serverFolder: piInstallation!.serverFolder,
                projectId,
                versionNumber,
                progress: progressChannel(),
              },
            );
      setPluginInstallResult(result);
      if (target === "local") {
        setLocalInstallation({
          serverPath: result.server_path,
          previousServerPath:
            localInstallation!.previousServerPath ||
            localInstallation!.serverPath,
        });
        setLocalServerStatus(
          await invoke<LocalServerStatus>("inspect_local_server", {
            serverPath: result.server_path,
          }),
        );
      } else {
        setPiInstallation({
          ...piInstallation!,
          serverFolder: result.server_path,
          previousServerFolder:
            piInstallation!.previousServerFolder ||
            piInstallation!.serverFolder,
        });
        setPiServerStatus(
          await invoke<PiServerStatus>("inspect_pi_server", {
            host: piInstallation!.host,
            username: piInstallation!.username,
            serverFolder: result.server_path,
          }),
        );
      }
    } catch (error) {
      setPluginInstallError(String(error));
    } finally {
      setOperationProgress(null);
      setBusy(false);
    }
  };

  const restorePreviousLocalCopy = async () => {
    if (!localInstallation?.previousServerPath || localServerStatus?.running)
      return;
    setBusy(true);
    const candidate = localInstallation.serverPath;
    const previous = localInstallation.previousServerPath;
    try {
      const candidateStatus = await invoke<LocalServerStatus>(
        "inspect_local_server",
        { serverPath: candidate },
      );
      if (candidateStatus.running) {
        setLocalServerStatus(candidateStatus);
        setLocalStatusError(
          "Arrête d’abord le serveur candidat avant de revenir à la copie précédente.",
        );
        return;
      }
      const status = await invoke<LocalServerStatus>("inspect_local_server", {
        serverPath: previous,
      });
      setLocalInstallation({
        serverPath: previous,
        previousServerPath: candidate,
      });
      setLocalServerStatus(status);
      setLocalStatusError("");
      setLocalActionMessage(
        "Le dossier précédent est de nouveau sélectionné. L’autre copie reste conservée.",
      );
    } catch (error) {
      setLocalStatusError(
        `Le dossier précédent n’est pas accessible : ${error}`,
      );
    } finally {
      setBusy(false);
    }
  };

  const restorePreviousPiCopy = async () => {
    if (!piInstallation?.previousServerFolder || piServerStatus?.running)
      return;
    setBusy(true);
    const candidate = piInstallation.serverFolder;
    const previous = piInstallation.previousServerFolder;
    try {
      const candidateStatus = await invoke<PiServerStatus>(
        "inspect_pi_server",
        {
          host: piInstallation.host,
          username: piInstallation.username,
          serverFolder: candidate,
        },
      );
      if (candidateStatus.running) {
        setPiServerStatus(candidateStatus);
        setPiStatusError(
          "Arrête d’abord le serveur candidat avant de revenir à la copie précédente.",
        );
        return;
      }
      const status = await invoke<PiServerStatus>("inspect_pi_server", {
        host: piInstallation.host,
        username: piInstallation.username,
        serverFolder: previous,
      });
      setPiInstallation({
        ...piInstallation,
        serverFolder: previous,
        previousServerFolder: candidate,
      });
      setPiServerStatus(status);
      setPiStatusError("");
      setPiActionMessage(
        "Le dossier Pi précédent est de nouveau sélectionné. L’autre copie reste conservée.",
      );
    } catch (error) {
      setPiStatusError(
        `Le dossier Pi précédent n’est pas accessible : ${error}`,
      );
    } finally {
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

  const exportLocalLog = async () => {
    if (!localInstallation) return;
    const outputFile = await save({
      defaultPath: "Monde-Ensemble-journal-complet.log",
      filters: [{ name: "Journal du serveur", extensions: ["log"] }],
    });
    if (!outputFile) return;
    setBusy(true);
    setLocalStatusError("");
    try {
      setLocalActionMessage(
        await invoke<string>("export_local_server_log", {
          serverPath: localInstallation.serverPath,
          outputFile,
        }),
      );
    } catch (error) {
      setLocalStatusError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const exportPiLog = async () => {
    if (!piInstallation) return;
    const outputFile = await save({
      defaultPath: "Monde-Ensemble-journal-complet-Pi.log",
      filters: [{ name: "Journal du serveur", extensions: ["log"] }],
    });
    if (!outputFile) return;
    setBusy(true);
    setPiStatusError("");
    try {
      setPiActionMessage(
        await invoke<string>("export_pi_server_log", {
          host: piInstallation.host,
          username: piInstallation.username,
          serverFolder: piInstallation.serverFolder,
          outputFile,
        }),
      );
    } catch (error) {
      setPiStatusError(String(error));
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
          MONDE ENSEMBLE <span>·</span> {__APP_VERSION__}
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
              localUpdateCheck={localUpdateCheck}
              localUpdateError={localUpdateError}
              checkingLocalUpdate={checkingLocalUpdate}
              localWhitelistStatus={localWhitelistStatus}
              localPlayerName={localPlayerName}
              localPlayerPlatform={localPlayerPlatform}
              onLocalPlayerNameChange={setLocalPlayerName}
              onLocalPlayerPlatformChange={setLocalPlayerPlatform}
              onManageLocalPlayer={manageLocalPlayer}
              onSetLocalWhitelist={setLocalWhitelist}
              onRefreshLocal={refreshLocalStatus}
              onCheckLocalUpdate={checkLocalUpdate}
              onStartLocal={startLocalServer}
              onStopLocal={stopLocalServer}
              onExportLocalLog={exportLocalLog}
              piInstallation={piInstallation}
              piServerStatus={piServerStatus}
              piWhitelistStatus={piWhitelistStatus}
              piPlayerName={piPlayerName}
              piPlayerPlatform={piPlayerPlatform}
              piPlayerError={piPlayerError}
              piPlayerMessage={piPlayerMessage}
              onPiPlayerNameChange={setPiPlayerName}
              onPiPlayerPlatformChange={setPiPlayerPlatform}
              onManagePiPlayer={managePiPlayer}
              onSetPiWhitelist={setPiWhitelist}
              onPreparePiPlayerManagement={preparePiPlayerManagement}
              piStatusError={piStatusError}
              piActionMessage={piActionMessage}
              actionProgress={busy ? (operationProgress?.step ?? "") : ""}
              busy={busy}
              onRefreshPi={() => refreshPiStatus(true)}
              onStartPi={startPiServer}
              onStopPi={stopPiServer}
              onExportPiLog={exportPiLog}
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
                    <em>Pi neuf : préparation guidée dans l’application</em>
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
                    <div className="pi-fresh-entry">
                      <div className="eyebrow">
                        NOUVEAU PI · PARCOURS DÉBUTANT
                      </div>
                      <p>
                        Ton Pi peut être neuf, allumé et sans stockage : il n’a
                        encore rien à démarrer. Branche sa carte vide au Mac qui
                        exécute cette application. Elle y écrit le système, le
                        réseau et l’accès sécurisé ; aucun serveur ne s’installe
                        sur le Mac. Tu n’as pas à préparer le Pi ni à saisir son
                        IP ou à configurer SSH.
                      </p>
                      <button
                        className="button secondary"
                        type="button"
                        onClick={() =>
                          document
                            .getElementById("pi-fresh-title")
                            ?.scrollIntoView({
                              behavior: "smooth",
                              block: "start",
                            })
                        }
                      >
                        Préparer un Pi neuf depuis cette application
                      </button>
                    </div>
                    <div className="pi-separator">
                      <span>PI DÉJÀ CONFIGURÉ</span>
                    </div>
                    <div className="eyebrow">INSTALLATION DISTANTE</div>
                    <h3>Installer le serveur sur le Pi</h3>
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
                        Cette préparation se fait sur le Mac avec la carte SD ou
                        le SSD vide branché. Le Pi n’a pas besoin de système, de
                        Wi-Fi ni de SSH pour commencer. L’application choisit le
                        système compatible et vérifie le support avant de
                        l’effacer.
                      </p>
                      <ol
                        className="pi-first-steps"
                        aria-label="Progression de l’installation"
                      >
                        <li className={piFlashResult ? "complete" : "current"}>
                          <b>1</b>
                          <span>
                            <strong>Préparer la carte</strong>
                            <small>
                              {piFlashResult
                                ? "Système écrit et vérifié"
                                : "Sur ce Mac, depuis l’application"}
                            </small>
                          </span>
                        </li>
                        <li
                          className={
                            piInstallation
                              ? "complete"
                              : piFlashResult
                                ? "current"
                                : ""
                          }
                        >
                          <b>2</b>
                          <span>
                            <strong>Démarrer le Pi</strong>
                            <small>
                              {piInstallation
                                ? "Pi trouvé sur le réseau"
                                : "Insérer la carte Pi éteint, puis le rallumer"}
                            </small>
                          </span>
                        </li>
                        <li
                          className={
                            piServerStatus?.ready
                              ? "complete"
                              : piInstallation
                                ? "current"
                                : ""
                          }
                        >
                          <b>3</b>
                          <span>
                            <strong>Installer et contrôler</strong>
                            <small>
                              {piServerStatus?.ready
                                ? "Java et Bedrock répondent"
                                : "Serveur, versions et ports"}
                            </small>
                          </span>
                        </li>
                        <li className={piServerStatus?.ready ? "current" : ""}>
                          <b>4</b>
                          <span>
                            <strong>Rejoindre le monde</strong>
                            <small>
                              {piServerStatus?.ready
                                ? "À essayer dans Minecraft avec l’adresse affichée"
                                : "L’adresse apparaîtra après le contrôle"}
                            </small>
                          </span>
                        </li>
                      </ol>
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
                              Le mot de passe est placé temporairement dans un
                              fichier de démarrage ; l’application programme sa
                              suppression au premier démarrage. Pour ne pas
                              écrire le mot de passe sur la carte, utilise
                              plutôt un câble réseau.
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
                        <>
                          {piBootTargets.length > 1 && (
                            <p className="pi-help">
                              Pour éviter d’effacer le mauvais support,
                              débranche les autres cartes SD et disques
                              externes. L’application n’autorisera l’écriture
                              que lorsqu’un seul support sera détecté.
                            </p>
                          )}
                          <ul className="pi-storage-list">
                            {piBootTargets.map((device) => (
                              <li key={device.id}>
                                <div>
                                  <strong>{device.name}</strong>
                                  <span>
                                    {device.size} · {device.connection} ·{" "}
                                    {device.id}
                                  </span>
                                </div>
                                {piBootTargets.length === 1 &&
                                  piOsRecommendation &&
                                  device.size_bytes > 0 && (
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
                                {piBootTargets.length === 1 &&
                                  piOsRecommendation &&
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
                        </>
                      )}
                      {piOsRecommendation && (
                        <p className="pi-storage-guidance">
                          Il faut environ{" "}
                          {Math.ceil(
                            piOsRecommendation.image_size_gb +
                              piOsRecommendation.initial_server_storage_gb,
                          )}{" "}
                          Go pour le système et le serveur. Pour commencer
                          simplement, prends une carte microSD de 32 Go. Tu
                          pourras déplacer ton monde vers un autre support plus
                          tard avec une sauvegarde. Un SSD USB peut démarrer un
                          Pi 4 ou 5 si son boîtier est assez alimenté et si le
                          Pi autorise le démarrage USB. Certains anciens Pi 4
                          nécessitent une mise à jour de leur chargeur de
                          démarrage. L’application vérifie la capacité et
                          l’écriture; seul le premier démarrage confirme que le
                          Pi accepte ce SSD. Si tu veux éviter cette
                          incertitude, commence en microSD.
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
                            Éteins le Pi avant d’insérer la carte, puis relie-le
                            au même routeur (câble ou Wi-Fi configuré) et
                            rallume-le. Attends quelques minutes ; l’application
                            le recherchera et installera le serveur sur le Pi.
                            Le Mac ne sert qu’à préparer la carte et piloter
                            l’installation.
                          </p>
                          <button
                            className="button primary"
                            onClick={finishNewPi}
                            disabled={busy || !piEulaAccepted}
                          >
                            {busy
                              ? "Recherche automatique du Pi…"
                              : "Le Pi a redémarré · terminer l’installation"}
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
              piInstallation={piInstallation}
              piServerStatus={piServerStatus}
              stopped={backupStopped}
              setStopped={setBackupStopped}
              archive={restoreArchive}
              destination={restoreDestination}
              busy={busy}
              progress={operationProgress}
              result={operationResult}
              error={operationError}
              restoreBlocked={Boolean(
                localServerStatus?.running ||
                localServerStatus?.java_port_open ||
                localServerStatus?.bedrock_tcp_port_open ||
                localServerStatus?.bedrock_udp_port_open,
              )}
              onChooseSource={() => selectFolder(setBackupSource)}
              onChooseArchive={selectArchive}
              onChooseDestination={() => selectFolder(setRestoreDestination)}
              onBackup={createBackup}
              onBackupPi={createPiBackup}
              onRestore={restoreWorld}
              onRestorePi={restoreWorldToPi}
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
              onInstallPlugin={installPaperPlugin}
              onClearPluginResult={() => {
                setPluginInstallResult(null);
                setPluginInstallError("");
              }}
              pluginInstallError={pluginInstallError}
              pluginInstallResult={pluginInstallResult}
              canInstallLocalPlugin={Boolean(
                localInstallation &&
                !localServerStatus?.running &&
                localServerStatus?.minecraft_java,
              )}
              canInstallPiPlugin={Boolean(
                piInstallation &&
                !piServerStatus?.running &&
                piServerStatus?.minecraft_java,
              )}
              onRestorePreviousLocal={restorePreviousLocalCopy}
              onRestorePreviousPi={restorePreviousPiCopy}
              pluginProgress={operationProgress}
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
  localUpdateCheck,
  localUpdateError,
  checkingLocalUpdate,
  localWhitelistStatus,
  localPlayerName,
  localPlayerPlatform,
  onLocalPlayerNameChange,
  onLocalPlayerPlatformChange,
  onManageLocalPlayer,
  onSetLocalWhitelist,
  onRefreshLocal,
  onCheckLocalUpdate,
  onStartLocal,
  onStopLocal,
  onExportLocalLog,
  piInstallation,
  piServerStatus,
  piWhitelistStatus,
  piPlayerName,
  piPlayerPlatform,
  piPlayerError,
  piPlayerMessage,
  onPiPlayerNameChange,
  onPiPlayerPlatformChange,
  onManagePiPlayer,
  onSetPiWhitelist,
  onPreparePiPlayerManagement,
  piStatusError,
  piActionMessage,
  actionProgress,
  busy,
  onRefreshPi,
  onStartPi,
  onStopPi,
  onExportPiLog,
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
  localUpdateCheck: PiUpdateCheck | null;
  localUpdateError: string;
  checkingLocalUpdate: boolean;
  localWhitelistStatus: LocalWhitelistStatus | null;
  localPlayerName: string;
  localPlayerPlatform: "java" | "bedrock";
  onLocalPlayerNameChange: (value: string) => void;
  onLocalPlayerPlatformChange: (value: "java" | "bedrock") => void;
  onManageLocalPlayer: (action: "add" | "remove") => void;
  onSetLocalWhitelist: (enabled: boolean) => void;
  onRefreshLocal: () => void;
  onCheckLocalUpdate: () => void;
  onStartLocal: () => void;
  onStopLocal: () => void;
  onExportLocalLog: () => void;
  piInstallation: PiInstallation | null;
  piServerStatus: PiServerStatus | null;
  piWhitelistStatus: PiWhitelistStatus | null;
  piPlayerName: string;
  piPlayerPlatform: "java" | "bedrock";
  piPlayerError: string;
  piPlayerMessage: string;
  onPiPlayerNameChange: (value: string) => void;
  onPiPlayerPlatformChange: (value: "java" | "bedrock") => void;
  onManagePiPlayer: (action: "add" | "remove") => void;
  onSetPiWhitelist: (enabled: boolean) => void;
  onPreparePiPlayerManagement: () => void;
  piStatusError: string;
  piActionMessage: string;
  actionProgress: string;
  busy: boolean;
  onRefreshPi: () => void;
  onStartPi: () => void;
  onStopPi: () => void;
  onExportPiLog: () => void;
  onCheckPiUpdate: () => void;
  piUpdateCheck: PiUpdateCheck | null;
  piUpdateError: string;
  checkingPiUpdate: boolean;
  onInstall: () => void;
  onCompat: () => void;
  onBackup: () => void;
}) {
  const [copiedAddress, setCopiedAddress] = useState("");

  const copyAddress = async (value: string, label: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopiedAddress(`${label} copié.`);
    } catch {
      setCopiedAddress("Copie indisponible. Recopie l’adresse affichée.");
    }
  };

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
              <button
                className="address-copy"
                type="button"
                disabled={!localServerStatus?.address}
                onClick={() =>
                  copyAddress(
                    `${localServerStatus?.address}:${localServerStatus?.java_port || 25565}`,
                    "Adresse Java",
                  )
                }
              >
                Copier l’adresse Java
              </button>
            </div>
            <div>
              <small>TÉLÉPHONE · TABLETTE · BEDROCK</small>
              <strong>{localServerStatus?.address || "—"}</strong>
              <span>
                Port : {localServerStatus?.bedrock_port || "—"} · Ajouter un
                serveur
              </span>
              <button
                className="address-copy"
                type="button"
                disabled={!localServerStatus?.address}
                onClick={() =>
                  copyAddress(
                    localServerStatus?.address || "",
                    "Adresse Bedrock",
                  )
                }
              >
                Copier l’IP Bedrock
              </button>
            </div>
          </div>
          {copiedAddress && (
            <p className="address-copy-feedback" role="status">
              {copiedAddress}
            </p>
          )}
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
          <div className="pi-checks">
            <span className={localServerStatus?.ready ? "ok" : ""}>
              Java TCP{" "}
              {localServerStatus?.java_port_open ? "· ouvert" : "· fermé"}
            </span>
            <span
              className={localServerStatus?.bedrock_tcp_port_open ? "ok" : ""}
            >
              Signal Bedrock TCP{" "}
              {localServerStatus?.bedrock_tcp_port_open
                ? "· ouvert"
                : "· fermé"}
            </span>
            <span
              className={localServerStatus?.bedrock_udp_port_open ? "ok" : ""}
            >
              Jeu Bedrock UDP{" "}
              {localServerStatus?.bedrock_udp_port_open
                ? "· ouvert"
                : "· fermé"}
            </span>
          </div>
          <div className="pi-actions">
            <button
              className="text-button"
              onClick={onCheckLocalUpdate}
              disabled={busy || checkingLocalUpdate}
            >
              {checkingLocalUpdate
                ? "Vérification des versions et du serveur…"
                : "Vérifier une mise à jour sûre"}
            </button>
          </div>
          {localUpdateCheck && (
            <div
              className={`pi-update-check ${localUpdateCheck.ready ? "ready" : ""}`}
              aria-live="polite"
            >
              <strong>
                {localUpdateCheck.ready
                  ? localUpdateCheck.update_available
                    ? "Mise à jour coordonnée disponible"
                    : "Serveur à jour"
                  : "Mise à jour automatique bloquée pour protéger ce monde"}
              </strong>
              <span>
                Installé : Java {localUpdateCheck.current_java} · Bedrock{" "}
                {localUpdateCheck.current_bedrock}
              </span>
              {localUpdateCheck.ready && localUpdateCheck.update_available && (
                <span>
                  Disponible : Java {localUpdateCheck.latest_java} · Bedrock{" "}
                  {localUpdateCheck.latest_bedrock}. Le contrôle ne modifie pas
                  le serveur.
                </span>
              )}
              {localUpdateCheck.reasons.map((reason) => (
                <span key={reason}>{reason}</span>
              ))}
            </div>
          )}
          {localUpdateError && (
            <p className="pi-error" role="alert">
              {localUpdateError}
            </p>
          )}
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
          <section
            className="player-tools"
            aria-labelledby="local-players-title"
          >
            <div className="player-tools-heading">
              <div>
                <div className="eyebrow">ACCÈS AU MONDE</div>
                <h3 id="local-players-title">Joueurs autorisés</h3>
              </div>
              <span
                className={
                  localWhitelistStatus?.enabled
                    ? "player-state enabled"
                    : "player-state"
                }
              >
                {localWhitelistStatus
                  ? localWhitelistStatus.enabled
                    ? "Liste active"
                    : "Liste inactive"
                  : "Serveur à démarrer"}
              </span>
            </div>
            {localWhitelistStatus ? (
              <>
                <p className="player-tools-note">
                  {localWhitelistStatus.enabled
                    ? "Seuls les joueurs listés peuvent rejoindre."
                    : "La liste est inactive : toute personne connaissant l’adresse du serveur peut tenter de rejoindre."}
                </p>
                <div className="player-entry-row">
                  <label>
                    <span>Édition</span>
                    <select
                      value={localPlayerPlatform}
                      onChange={(event) =>
                        onLocalPlayerPlatformChange(
                          event.target.value as "java" | "bedrock",
                        )
                      }
                      disabled={busy}
                    >
                      <option value="java">Java · PC et Mac</option>
                      <option value="bedrock">
                        Bedrock · mobile, console, Windows
                      </option>
                    </select>
                  </label>
                  <label className="player-name-input">
                    <span>
                      {localPlayerPlatform === "java"
                        ? "Nom Java"
                        : "Gamertag Xbox"}
                    </span>
                    <input
                      value={localPlayerName}
                      onChange={(event) =>
                        onLocalPlayerNameChange(event.target.value)
                      }
                      placeholder={
                        localPlayerPlatform === "java"
                          ? "Ex. Alex_12"
                          : "Ex. Alex 12"
                      }
                      maxLength={16}
                      disabled={busy}
                    />
                  </label>
                  <button
                    className="button primary"
                    onClick={() => onManageLocalPlayer("add")}
                    disabled={busy || !localPlayerName.trim()}
                  >
                    Ajouter
                  </button>
                  <button
                    className="button secondary"
                    onClick={() => onManageLocalPlayer("remove")}
                    disabled={busy || !localPlayerName.trim()}
                  >
                    Retirer
                  </button>
                </div>
                {localPlayerPlatform === "bedrock" && (
                  <p className="player-tools-note">
                    Floodgate peut ajouter un gamertag après sa première
                    connexion à un serveur Geyser. Si la liste est déjà active,
                    il faut d’abord l’autoriser autrement; l’application ne
                    désactive jamais la protection pour le faire.
                  </p>
                )}
                <details className="player-list-output">
                  <summary>Voir la réponse du serveur</summary>
                  <pre>{localWhitelistStatus.entries}</pre>
                </details>
                <button
                  className="text-button"
                  onClick={() =>
                    onSetLocalWhitelist(!localWhitelistStatus.enabled)
                  }
                  disabled={busy}
                >
                  {localWhitelistStatus.enabled
                    ? "Désactiver la liste d’accès"
                    : "Activer la liste d’accès"}
                </button>
              </>
            ) : (
              <p className="player-tools-note">
                Démarre le serveur pour ajouter ou retirer un joueur. La liste
                d’accès protège ton monde lorsque tu l’actives.
              </p>
            )}
          </section>
          {localStatusError && (
            <p className="pi-error" role="alert">
              {localStatusError}
            </p>
          )}
          {localServerStatus?.log && (
            <>
              <details className="pi-log">
                <summary>Journal récent du serveur</summary>
                <pre>{localServerStatus.log}</pre>
              </details>
              <button
                className="text-button"
                onClick={onExportLocalLog}
                disabled={busy}
              >
                Exporter le journal complet
              </button>
            </>
          )}
          {localServerStatus?.running &&
            !localServerStatus.bedrock_tcp_port_open && (
              <p className="pi-help">
                Le signalement Bedrock TCP ne répond pas. Les clients Bedrock
                récents peuvent afficher Terracotta / InitialConnection-149 si
                NetherNet ou ce port n’est pas accessible.
              </p>
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
              <button
                className="address-copy"
                type="button"
                disabled={!piServerStatus?.address && !piInstallation.address}
                onClick={() =>
                  copyAddress(
                    `${piServerStatus?.address || piInstallation.address}:${piServerStatus?.java_port || 25565}`,
                    "Adresse Java",
                  )
                }
              >
                Copier l’adresse Java
              </button>
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
              <button
                className="address-copy"
                type="button"
                disabled={!piServerStatus?.address && !piInstallation.address}
                onClick={() =>
                  copyAddress(
                    piServerStatus?.address || piInstallation.address,
                    "Adresse Bedrock",
                  )
                }
              >
                Copier l’IP Bedrock
              </button>
            </div>
          </div>
          {copiedAddress && (
            <p className="address-copy-feedback" role="status">
              {copiedAddress}
            </p>
          )}
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
            <span className={piServerStatus?.bedrock_tcp_port_open ? "ok" : ""}>
              Signal Bedrock TCP{" "}
              {piServerStatus?.bedrock_tcp_port_open
                ? "· disponible"
                : "· non vérifié"}
            </span>
            <span className={piServerStatus?.bedrock_port_open ? "ok" : ""}>
              Jeu Bedrock UDP{" "}
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
              {piServerStatus.running && !piServerStatus.bedrock_tcp_port_open
                ? "Le signalement Bedrock TCP ne répond pas. Vérifie NetherNet dans Geyser et l’accès au port TCP Bedrock; des clients récents peuvent afficher Terracotta / InitialConnection-149."
                : piServerStatus.running
                  ? "Minecraft n’a pas encore confirmé son démarrage. Ouvre le journal pour voir la cause."
                  : "Le Pi ne répond pas ou le serveur est arrêté. Vérifie qu’il est allumé et connecté au même réseau, puis actualise."}
            </p>
          )}
          {piServerStatus?.log && (
            <>
              <details className="pi-log">
                <summary>Journal récent · utile en cas de problème</summary>
                <pre>{piServerStatus.log}</pre>
              </details>
              <button
                className="text-button"
                onClick={onExportPiLog}
                disabled={busy}
              >
                Exporter le journal complet
              </button>
            </>
          )}
          {piServerStatus?.address && !piStatusError ? (
            <details className="pi-network-guide">
              <summary>Garder cette adresse sur la durée</summary>
              <p>
                L’adresse affichée est celle du Pi sur ton Wi-Fi ou ton réseau
                câblé. Pour qu’elle reste la même, réserve-la une fois dans la
                Freebox :
              </p>
              <ol>
                <li>
                  Dans l’application Free, ouvre{" "}
                  <strong>Mes équipements Freebox</strong>, puis{" "}
                  <strong>Server internet</strong> et{" "}
                  <strong>Paramètres réseau avancés</strong> →{" "}
                  <strong>DHCP</strong>.
                </li>
                <li>
                  Ajoute un bail statique ou permanent pour le Raspberry Pi,
                  avec l’adresse IP et l’adresse matérielle ci-dessous.
                </li>
                <li>
                  Enregistre le bail et vérifie sa présence dans l’application
                  Free. Ici, « Actualiser » relit l’IP active, mais ne peut pas
                  vérifier l’état du bail dans la Freebox.
                </li>
              </ol>
              <div className="pi-network-values">
                <div>
                  <span>IP À RÉSERVER</span>
                  <code>{piServerStatus.address}</code>
                  <button
                    className="address-copy"
                    type="button"
                    onClick={() =>
                      copyAddress(piServerStatus.address, "Adresse IP")
                    }
                  >
                    Copier l’adresse IP
                  </button>
                </div>
                <div>
                  <span>ADRESSE MATÉRIELLE DU PI</span>
                  <code>{piServerStatus?.mac_address || "Non disponible"}</code>
                  {piServerStatus?.mac_address && (
                    <button
                      className="address-copy"
                      type="button"
                      onClick={() =>
                        copyAddress(
                          piServerStatus.mac_address,
                          "Adresse matérielle",
                        )
                      }
                    >
                      Copier l’adresse matérielle
                    </button>
                  )}
                </div>
              </div>
              {piServerStatus?.network_interface && (
                <p className="pi-network-note">
                  Connexion utilisée : {piServerStatus.network_interface}
                  {piServerStatus.network_interface.startsWith("eth")
                    ? " (câble réseau)"
                    : piServerStatus.network_interface.startsWith("wlan")
                      ? " (Wi‑Fi)"
                      : ""}
                  .
                </p>
              )}
              <p className="pi-network-note">
                C’est une adresse locale pour jouer sur le même réseau. Jouer
                depuis l’extérieur demande un réglage séparé du routeur.
              </p>
            </details>
          ) : (
            <p className="pi-network-note">
              Actualise l’état du Pi pour relire son adresse et afficher les
              informations nécessaires à sa réservation dans la Freebox.
            </p>
          )}
        </section>
      )}
      {piInstallation && (
        <section className="player-tools" aria-labelledby="pi-players-title">
          <div className="player-tools-heading">
            <div>
              <div className="eyebrow">ACCÈS AU MONDE · RASPBERRY PI</div>
              <h3 id="pi-players-title">Joueurs autorisés</h3>
            </div>
            <span
              className={
                piWhitelistStatus?.enabled
                  ? "player-state enabled"
                  : "player-state"
              }
            >
              {piWhitelistStatus
                ? piWhitelistStatus.enabled
                  ? "Liste active"
                  : "Liste inactive"
                : piServerStatus?.running
                  ? "À vérifier"
                  : "Serveur à démarrer"}
            </span>
          </div>
          {piWhitelistStatus ? (
            <>
              <p className="player-tools-note">
                {piWhitelistStatus.enabled
                  ? "Seuls les joueurs listés peuvent rejoindre."
                  : "La liste est inactive : toute personne connaissant l’adresse peut tenter de rejoindre."}
              </p>
              <div className="player-entry-row">
                <label>
                  <span>Édition</span>
                  <select
                    value={piPlayerPlatform}
                    onChange={(event) =>
                      onPiPlayerPlatformChange(
                        event.target.value as "java" | "bedrock",
                      )
                    }
                    disabled={busy}
                  >
                    <option value="java">Java · PC et Mac</option>
                    <option value="bedrock">
                      Bedrock · mobile, console, Windows
                    </option>
                  </select>
                </label>
                <label className="player-name-input">
                  <span>
                    {piPlayerPlatform === "java" ? "Nom Java" : "Gamertag Xbox"}
                  </span>
                  <input
                    value={piPlayerName}
                    onChange={(event) =>
                      onPiPlayerNameChange(event.target.value)
                    }
                    placeholder={
                      piPlayerPlatform === "java"
                        ? "Ex. Alex_12"
                        : "Ex. Alex 12"
                    }
                    maxLength={16}
                    disabled={busy}
                  />
                </label>
                <button
                  className="button primary"
                  onClick={() => onManagePiPlayer("add")}
                  disabled={busy || !piPlayerName.trim()}
                >
                  Ajouter
                </button>
                <button
                  className="button secondary"
                  onClick={() => onManagePiPlayer("remove")}
                  disabled={busy || !piPlayerName.trim()}
                >
                  Retirer
                </button>
              </div>
              {piPlayerPlatform === "bedrock" && (
                <p className="player-tools-note">
                  Un gamertag Bedrock doit avoir rejoint une première fois un
                  serveur Geyser. La protection ne sera jamais désactivée pour
                  contourner cette règle.
                </p>
              )}
              <details className="player-list-output">
                <summary>Voir la réponse du serveur</summary>
                <pre>{piWhitelistStatus.entries}</pre>
              </details>
              <button
                className="text-button"
                onClick={() => onSetPiWhitelist(!piWhitelistStatus.enabled)}
                disabled={busy}
              >
                {piWhitelistStatus.enabled
                  ? "Désactiver la liste d’accès"
                  : "Activer la liste d’accès"}
              </button>
            </>
          ) : (
            <>
              <p className="player-tools-note">
                {piPlayerError ||
                  (piServerStatus?.running
                    ? "La liste sera disponible après vérification du contrôle sécurisé."
                    : "Démarre le serveur pour gérer la liste des joueurs.")}
              </p>
              {piPlayerError.includes("antérieure à la gestion sécurisée") && (
                <>
                  <p className="player-tools-note">
                    Cette préparation conserve le monde, crée deux copies de
                    réglages et ne fonctionne que serveur arrêté.
                  </p>
                  <button
                    className="button secondary"
                    onClick={onPreparePiPlayerManagement}
                    disabled={busy || piServerStatus?.running}
                  >
                    {piServerStatus?.running
                      ? "Arrête le serveur avant la préparation"
                      : "Préparer la gestion des joueurs"}
                  </button>
                </>
              )}
            </>
          )}
          {piPlayerMessage && (
            <p className="pi-action-message" aria-live="polite">
              {piPlayerMessage}
            </p>
          )}
          {piPlayerError && piWhitelistStatus && (
            <p className="pi-error" role="alert">
              {piPlayerError}
            </p>
          )}
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
      "Étape externe requise",
      "Le jeu ne permet pas d’ajouter directement une IP privée",
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
      <section className="console-guide" aria-labelledby="console-guide-title">
        <div className="eyebrow">CONSOLES · CONFIGURATION À PART</div>
        <h3 id="console-guide-title">
          Switch, Xbox et PlayStation demandent une étape supplémentaire
        </h3>
        <p>
          Minecraft sur console ne propose pas l’ajout direct d’une adresse de
          serveur privé. Le monde reste hébergé sur le Mac, le PC ou le Pi; il
          faut seulement aider la console à le trouver.
        </p>
        <div className="console-guide-grid">
          <div>
            <strong>À la maison · relais LAN</strong>
            <p>
              Une application compatible sur téléphone, tablette ou PC fait
              apparaître le serveur dans la liste des parties locales. Mets
              l’appareil et la console sur le même Wi-Fi, puis saisis l’adresse
              Bedrock et le port affichés dans le tableau de bord.
            </p>
          </div>
          <div>
            <strong>Autre méthode · menu DNS</strong>
            <p>
              Un service tiers peut ouvrir un menu de serveurs à partir d’un
              serveur vedette. Il faut configurer le DNS sur la console; cette
              application ne change aucun réglage réseau. Si la connexion
              Internet de la console se dégrade, remets le DNS en automatique.
            </p>
          </div>
          <div>
            <strong>Mode expert · compte Xbox dédié</strong>
            <p>
              MCXboxBroadcast peut faire apparaître le serveur dans la liste
              d’amis de la console. Cette méthode demande de configurer un
              compte Xbox séparé et un relais côté serveur; Monde Ensemble ne
              crée pas ce compte et ne configure pas ce relais.
            </p>
          </div>
        </div>
        <small>
          Ces méthodes externes ne sont ni installées ni testées par Monde
          Ensemble.{" "}
          <a
            href="https://geysermc.org/wiki/geyser/using-geyser-with-consoles/"
            target="_blank"
            rel="noreferrer"
          >
            Lire le guide officiel de GeyserMC
          </a>
        </small>
      </section>
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
          <p>Essaie dans cet ordre :</p>
          <ol>
            <li>
              Sur le même Wi-Fi que le serveur, utilise l’adresse et le port
              Bedrock affichés dans le tableau de bord; évite le réseau invité.
            </li>
            <li>
              Si cela marche à la maison mais pas à l’extérieur, le routeur doit
              laisser passer le port Bedrock en TCP et UDP. L’application ne
              modifie pas le routeur.
            </li>
            <li>
              Si cela échoue aussi à la maison, vérifie « Signal Bedrock TCP »
              et « Jeu Bedrock UDP » dans le tableau, puis ouvre ou exporte le
              journal du serveur.
            </li>
          </ol>
          <small>
            Plage Bedrock actuellement publiée par Geyser :{" "}
            {report?.bedrock_versions ?? "non vérifiée pour l’instant"}. Le
            tableau « Compatibilité » relit cette plage à chaque contrôle.
          </small>
        </div>
      </div>
      <div className="notice amber">
        <span>!</span>
        <p>
          <strong>
            Les mises à jour coordonnées protègent les serveurs locaux et Pi
            gérés.
          </strong>
          <br />
          Au prochain démarrage, l’application vérifie ensemble Java, Bedrock,
          Paper, Geyser et Floodgate. Elle ne met à jour qu’un profil reconnu,
          compatible et sauvegardable, puis vérifie que le serveur redémarre. Si
          un contrôle échoue, la copie précédente reste intacte et le serveur
          garde la version actuelle. Le bouton « Vérifier une mise à jour sûre »
          affiche les versions sans rien modifier.
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
  piInstallation,
  piServerStatus,
  stopped,
  setStopped,
  archive,
  destination,
  busy,
  progress,
  result,
  error,
  restoreBlocked,
  onChooseSource,
  onChooseArchive,
  onChooseDestination,
  onBackup,
  onBackupPi,
  onRestore,
  onRestorePi,
}: {
  source: string;
  output: string;
  piInstallation: PiInstallation | null;
  piServerStatus: PiServerStatus | null;
  stopped: boolean;
  setStopped: (stopped: boolean) => void;
  archive: string;
  destination: string;
  busy: boolean;
  progress: ProgressMessage | null;
  result: OperationResult | null;
  error: string;
  restoreBlocked: boolean;
  onChooseSource: () => void;
  onChooseArchive: () => void;
  onChooseDestination: () => void;
  onBackup: () => void;
  onBackupPi: () => void;
  onRestore: () => void;
  onRestorePi: () => void;
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
      {piInstallation && (
        <div className="operation-card">
          <div className="eyebrow">SERVEUR RASPBERRY PI</div>
          <h3>Copier le monde du Pi ici</h3>
          <p>
            Arrête d’abord le serveur depuis son tableau de bord. L’application
            vérifie l’espace, prépare une copie complète sur le Pi, la transfère
            de façon sécurisée et contrôle son empreinte.
          </p>
          <button
            className="button primary action-button"
            disabled={busy || Boolean(piServerStatus?.running)}
            onClick={onBackupPi}
          >
            {piServerStatus?.running
              ? "Arrête le serveur avant la sauvegarde"
              : busy
                ? "Sauvegarde en cours…"
                : "Choisir où enregistrer la sauvegarde du Pi"}
          </button>
        </div>
      )}
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
          disabled={busy || !archive || !destination || restoreBlocked}
          onClick={onRestore}
        >
          {restoreBlocked
            ? "Arrête le serveur avant de restaurer"
            : busy
              ? "Vérification en cours…"
              : "Vérifier et restaurer"}
        </button>
      </div>
      {piInstallation && (
        <div className="operation-card">
          <div className="eyebrow">TRANSFÉRER SUR LE RASPBERRY PI</div>
          <h3>Restaurer aussi sur le Pi</h3>
          <p>
            Utilise l’archive choisie ci-dessus. Monde Ensemble vérifie son
            intégrité, adapte Java au Pi et crée un dossier serveur neuf. Le
            serveur actuel reste conservé; la nouvelle copie reste arrêtée.
          </p>
          <button
            className="button primary action-button"
            disabled={
              busy ||
              !archive ||
              Boolean(
                piServerStatus?.running ||
                piServerStatus?.java_port_open ||
                piServerStatus?.bedrock_tcp_port_open ||
                piServerStatus?.bedrock_port_open,
              )
            }
            onClick={onRestorePi}
          >
            {piServerStatus?.running ||
            piServerStatus?.java_port_open ||
            piServerStatus?.bedrock_tcp_port_open ||
            piServerStatus?.bedrock_port_open
              ? "Arrête le serveur Pi avant la restauration"
              : busy
                ? "Vérification et transfert en cours…"
                : "Vérifier et restaurer sur le Pi"}
          </button>
        </div>
      )}
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

function GameSettingsPanel({
  localInstallation,
  localServerStatus,
  piInstallation,
  piServerStatus,
  busy,
}: {
  localInstallation: LocalInstallation | null;
  localServerStatus: LocalServerStatus | null;
  piInstallation: PiInstallation | null;
  piServerStatus: PiServerStatus | null;
  busy: boolean;
}) {
  const [target, setTarget] = useState<PluginTarget>(
    localInstallation ? "local" : "pi",
  );
  const [settings, setSettings] = useState<GameSettings | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const serverRunning =
    target === "local" ? localServerStatus?.running : piServerStatus?.running;
  const serverVerified =
    target === "local" ? Boolean(localServerStatus) : Boolean(piServerStatus);

  useEffect(() => {
    let cancelled = false;
    setSettings(null);
    setError("");
    setMessage("");
    const command =
      target === "local" && localInstallation
        ? invoke<GameSettings>("local_game_settings", {
            serverPath: localInstallation.serverPath,
          })
        : target === "pi" && piInstallation
          ? invoke<GameSettings>("pi_game_settings", {
              host: piInstallation.host,
              username: piInstallation.username,
              serverFolder: piInstallation.serverFolder,
            })
          : null;
    if (!command) return;
    setLoading(true);
    void command
      .then((value) => {
        if (!cancelled) setSettings(value);
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [
    target,
    localInstallation?.serverPath,
    piInstallation?.host,
    piInstallation?.username,
    piInstallation?.serverFolder,
  ]);

  const change = <K extends keyof GameSettings>(
    key: K,
    value: GameSettings[K],
  ) => {
    setSettings((current) =>
      current ? { ...current, [key]: value } : current,
    );
    setMessage("");
  };

  const saveSettings = async () => {
    if (!settings || serverRunning || !serverVerified) return;
    setSaving(true);
    setError("");
    setMessage("");
    try {
      const result =
        target === "local" && localInstallation
          ? await invoke<string>("save_local_game_settings", {
              serverPath: localInstallation.serverPath,
              settings,
            })
          : target === "pi" && piInstallation
            ? await invoke<string>("save_pi_game_settings", {
                host: piInstallation.host,
                username: piInstallation.username,
                serverFolder: piInstallation.serverFolder,
                settings,
              })
            : "";
      setMessage(result);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSaving(false);
    }
  };

  const hasServer = Boolean(localInstallation || piInstallation);
  return (
    <section className="operation-card advanced-log-card game-settings-card">
      <div className="eyebrow">MODE AVANCÉ · RÉGLAGES DU MONDE</div>
      <h3>Configurer le jeu</h3>
      <p className="path-note">
        Ces options modifient uniquement les règles du jeu. Une copie complète
        de <code>server.properties</code> est créée avant chaque enregistrement.
        Les ports, la sécurité, la liste d’accès et le contrôle du serveur
        restent protégés par l’application.
      </p>
      {!hasServer ? (
        <p className="path-note">
          Installe ou enregistre d’abord un serveur pour afficher ses réglages.
        </p>
      ) : (
        <>
          {localInstallation && piInstallation ? (
            <label className="plugin-target-picker">
              Serveur à configurer
              <select
                value={target}
                onChange={(event) =>
                  setTarget(event.target.value as PluginTarget)
                }
                disabled={loading || saving || busy}
              >
                <option value="local">Ordinateur · serveur local</option>
                <option value="pi">Raspberry Pi · serveur distant</option>
              </select>
            </label>
          ) : (
            <p className="settings-target-label">
              Cible : {localInstallation ? "serveur local" : "Raspberry Pi"}
            </p>
          )}
          {loading && <p className="path-note">Lecture des réglages…</p>}
          {error && (
            <p className="pi-error" role="alert">
              {error}
            </p>
          )}
          {settings && (
            <>
              <div className="game-settings-grid">
                <label className="game-setting-wide">
                  Nom affiché dans la liste Minecraft
                  <input
                    value={settings.motd}
                    maxLength={120}
                    onChange={(event) => change("motd", event.target.value)}
                    disabled={saving || busy}
                  />
                </label>
                <label>
                  Mode de jeu par défaut
                  <select
                    value={settings.gamemode}
                    onChange={(event) =>
                      change(
                        "gamemode",
                        event.target.value as GameSettings["gamemode"],
                      )
                    }
                    disabled={saving || busy}
                  >
                    <option value="survival">Survie</option>
                    <option value="creative">Créatif</option>
                    <option value="adventure">Aventure</option>
                    <option value="spectator">Spectateur</option>
                  </select>
                </label>
                <label>
                  Difficulté
                  <select
                    value={settings.difficulty}
                    onChange={(event) =>
                      change(
                        "difficulty",
                        event.target.value as GameSettings["difficulty"],
                      )
                    }
                    disabled={saving || busy}
                  >
                    <option value="peaceful">Paisible</option>
                    <option value="easy">Facile</option>
                    <option value="normal">Normale</option>
                    <option value="hard">Difficile</option>
                  </select>
                </label>
                <label>
                  Joueurs maximum
                  <input
                    type="number"
                    min={1}
                    max={1000}
                    value={settings.max_players}
                    onChange={(event) =>
                      change("max_players", Number(event.target.value))
                    }
                    disabled={saving || busy}
                  />
                </label>
                <label>
                  Distance d’affichage · chunks
                  <input
                    type="number"
                    min={2}
                    max={16}
                    value={settings.view_distance}
                    onChange={(event) =>
                      change("view_distance", Number(event.target.value))
                    }
                    disabled={saving || busy}
                  />
                </label>
                <label>
                  Distance de simulation · chunks
                  <input
                    type="number"
                    min={2}
                    max={16}
                    value={settings.simulation_distance}
                    onChange={(event) =>
                      change("simulation_distance", Number(event.target.value))
                    }
                    disabled={saving || busy}
                  />
                </label>
              </div>
              <p className="path-note">
                Des distances plus grandes chargent davantage de terrain et
                peuvent ralentir un Raspberry Pi. Commence à 8 chunks
                d’affichage et 6 de simulation, puis augmente si le serveur
                reste fluide.
              </p>
              <div className="game-settings-toggles">
                <label>
                  <input
                    type="checkbox"
                    checked={settings.pvp}
                    onChange={(event) => change("pvp", event.target.checked)}
                    disabled={saving || busy}
                  />
                  <span>Combat entre joueurs (PvP)</span>
                </label>
                <label>
                  <input
                    type="checkbox"
                    checked={settings.allow_nether}
                    onChange={(event) =>
                      change("allow_nether", event.target.checked)
                    }
                    disabled={saving || busy}
                  />
                  <span>Activer le Nether</span>
                </label>
              </div>
              {!serverVerified ? (
                <p className="path-note">
                  Attends la confirmation de l’état du serveur avant de modifier
                  ses réglages.
                </p>
              ) : serverRunning ? (
                <p className="path-note">
                  Arrête le serveur depuis « Mon monde » avant d’enregistrer.
                  Rien ne change pendant une partie.
                </p>
              ) : (
                <button
                  className="button primary"
                  onClick={() => void saveSettings()}
                  disabled={saving || busy || loading}
                >
                  {saving
                    ? "Sauvegarde des réglages…"
                    : "Enregistrer les réglages"}
                </button>
              )}
              {message && <p className="settings-save-message">{message}</p>}
              <small className="path-note">
                Le redémarrage du serveur est nécessaire. En cas de problème, la
                copie précédente de la configuration reste dans le dossier du
                serveur.
              </small>
            </>
          )}
        </>
      )}
    </section>
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
  onInstallPlugin,
  onClearPluginResult,
  pluginInstallError,
  pluginInstallResult,
  canInstallLocalPlugin,
  canInstallPiPlugin,
  onRestorePreviousLocal,
  onRestorePreviousPi,
  pluginProgress,
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
  onInstallPlugin: (
    target: PluginTarget,
    projectId: string,
    versionNumber: string,
  ) => void;
  onClearPluginResult: () => void;
  pluginInstallError: string;
  pluginInstallResult: PaperPluginInstallResult | null;
  canInstallLocalPlugin: boolean;
  canInstallPiPlugin: boolean;
  onRestorePreviousLocal: () => void;
  onRestorePreviousPi: () => void;
  pluginProgress: ProgressMessage | null;
  onRefresh: () => void;
  onCheckUpdates: () => void;
}) {
  const [pluginQuery, setPluginQuery] = useState("");
  const [pluginTarget, setPluginTarget] = useState<PluginTarget>(
    localInstallation ? "local" : "pi",
  );
  const [pluginSuggestions, setPluginSuggestions] = useState<
    PaperPluginSuggestion[]
  >([]);
  const [pluginLoading, setPluginLoading] = useState(false);
  const [pluginError, setPluginError] = useState("");
  const [packTarget, setPackTarget] = useState<PluginTarget>(
    localInstallation ? "local" : "pi",
  );
  const [packBusy, setPackBusy] = useState(false);
  const [packError, setPackError] = useState("");
  const [packResult, setPackResult] = useState<BedrockPackInstallResult | null>(
    null,
  );
  const pluginMinecraftVersion =
    pluginTarget === "local"
      ? localServerStatus?.minecraft_java || ""
      : piServerStatus?.minecraft_java || "";
  const searchPlugins = async () => {
    setPluginLoading(true);
    setPluginError("");
    try {
      const suggestions = await invoke<PaperPluginSuggestion[]>(
        "search_paper_plugins",
        {
          minecraftVersion: pluginMinecraftVersion,
          query: pluginQuery,
        },
      );
      setPluginSuggestions(suggestions);
    } catch (error) {
      setPluginError(String(error));
      setPluginSuggestions([]);
    } finally {
      setPluginLoading(false);
    }
  };
  const installBedrockPack = async () => {
    if (busy || packBusy) return;
    setPackError("");
    setPackResult(null);
    let selected: string | string[] | null;
    try {
      selected = await open({
        multiple: false,
        filters: [
          {
            name: "Pack de ressources Bedrock",
            extensions: ["mcpack", "zip"],
          },
        ],
      });
    } catch (error) {
      setPackError(String(error));
      return;
    }
    if (!selected || Array.isArray(selected)) return;
    setPackBusy(true);
    try {
      const result =
        packTarget === "local"
          ? await invoke<BedrockPackInstallResult>(
              "install_local_bedrock_pack",
              {
                serverPath: localInstallation!.serverPath,
                packPath: selected,
              },
            )
          : await invoke<BedrockPackInstallResult>("install_pi_bedrock_pack", {
              host: piInstallation!.host,
              username: piInstallation!.username,
              serverFolder: piInstallation!.serverFolder,
              packPath: selected,
            });
      setPackResult(result);
    } catch (error) {
      setPackError(String(error));
    } finally {
      setPackBusy(false);
    }
  };
  const packTargetExists =
    packTarget === "local"
      ? Boolean(localInstallation)
      : Boolean(piInstallation);
  const packTargetRunning =
    packTarget === "local"
      ? Boolean(localServerStatus?.running)
      : Boolean(piServerStatus?.running);
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
      <GameSettingsPanel
        localInstallation={localInstallation}
        localServerStatus={localServerStatus}
        piInstallation={piInstallation}
        piServerStatus={piServerStatus}
        busy={busy}
      />
      <section className="operation-card advanced-log-card">
        <div className="eyebrow">PACKS BEDROCK · GEYSER</div>
        <h3>Ajouter un pack de textures Bedrock</h3>
        <p className="path-note">
          Choisis un fichier <code>.mcpack</code> ou <code>.zip</code>. Monde
          Ensemble vérifie son manifeste Bedrock, son type de pack et sa version
          minimale déclarée, puis l’ajoute au dossier Geyser sans remplacer un
          fichier existant. Le serveur doit être arrêté; redémarre-le ensuite
          pour envoyer le pack aux joueurs Bedrock.
        </p>
        <p className="path-note">
          Un pack Java est un format différent et n’est pas converti par Geyser.
          La version minimale déclarée ne garantit pas que chaque appareil ou
          console acceptera le pack. Les mises à jour automatiques resteront
          suspendues tant que ce pack personnalisé est présent.
        </p>
        {localInstallation && piInstallation ? (
          <label className="plugin-target-picker">
            Serveur à configurer
            <select
              value={packTarget}
              onChange={(event) => {
                setPackTarget(event.target.value as PluginTarget);
                setPackError("");
                setPackResult(null);
              }}
              disabled={packBusy || busy}
            >
              <option value="local">Ordinateur · serveur local</option>
              <option value="pi">Raspberry Pi · serveur distant</option>
            </select>
          </label>
        ) : (
          <p className="path-note">
            Serveur :{" "}
            {localInstallation
              ? "local"
              : piInstallation
                ? "Raspberry Pi"
                : "aucun serveur enregistré"}
            .
          </p>
        )}
        {!packTargetExists ? (
          <p className="path-note">Installe d’abord un serveur géré.</p>
        ) : packTargetRunning ? (
          <p className="path-note">
            Arrête le serveur depuis « Mon monde » avant d’ajouter le pack.
          </p>
        ) : (
          <button
            className="button secondary"
            onClick={() => void installBedrockPack()}
            disabled={packBusy || busy}
          >
            {packBusy
              ? "Vérification et copie du pack…"
              : "Choisir un pack Bedrock"}
          </button>
        )}
        {packError && (
          <p className="pi-error" role="alert">
            {packError}
          </p>
        )}
        {packResult && (
          <div className="pack-install-result" role="status">
            <strong>{packResult.name} · ajouté</strong>
            <span>
              {packResult.filename} · Bedrock minimum déclaré :{" "}
              {packResult.minimum_engine_version}
            </span>
            <code>{packResult.sha256}</code>
            <small>{packResult.destination}</small>
            <small>
              Redémarre le serveur et essaie avec un client Bedrock compatible.
            </small>
          </div>
        )}
      </section>
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
      <section className="operation-card advanced-log-card">
        <div className="eyebrow">PLUGINS PAPER · COPIE PROTÉGÉE</div>
        <h3>Ajouter sans toucher à l’original</h3>
        <p className="path-note">
          Choisis le serveur, puis cherche les plugins annoncés pour sa version
          Minecraft Java et Paper. L’ajout crée une sauvegarde puis une copie
          complète distincte. Il faut démarrer cette copie pour vérifier le
          plugin.
        </p>
        <label className="plugin-target-picker">
          Serveur à protéger
          <select
            value={pluginTarget}
            onChange={(event) => {
              setPluginTarget(event.target.value as PluginTarget);
              setPluginSuggestions([]);
              setPluginError("");
              onClearPluginResult();
            }}
          >
            {localInstallation && (
              <option value="local">Ordinateur · serveur local</option>
            )}
            {piInstallation && (
              <option value="pi">Raspberry Pi · serveur distant</option>
            )}
          </select>
        </label>
        <div className="pi-actions">
          <input
            aria-label="Rechercher une extension Paper"
            value={pluginQuery}
            maxLength={80}
            onChange={(event) => setPluginQuery(event.target.value)}
            onKeyDown={(event) => {
              if (
                event.key === "Enter" &&
                pluginMinecraftVersion &&
                !pluginLoading
              )
                void searchPlugins();
            }}
            placeholder="Ex. protection, carte, permissions"
          />
          <button
            className="button secondary"
            onClick={() => void searchPlugins()}
            disabled={!pluginMinecraftVersion || pluginLoading || busy}
          >
            {pluginLoading ? "Recherche…" : "Chercher"}
          </button>
        </div>
        {pluginProgress && (
          <OperationStatus progress={pluginProgress} result={null} error="" />
        )}
        {!pluginMinecraftVersion && (
          <p className="path-note">
            Ouvre le tableau du serveur choisi pour charger sa version exacte.
          </p>
        )}
        {pluginError && <p className="pi-error">{pluginError}</p>}
        {pluginSuggestions.length > 0 && (
          <ul className="plugin-suggestion-list">
            {pluginSuggestions.map((plugin) => (
              <li key={plugin.project_id}>
                <div>
                  <strong>{plugin.title}</strong>
                  <small>
                    {plugin.version} ·{" "}
                    {plugin.downloads.toLocaleString("fr-FR")} téléchargements
                  </small>
                  <p>{plugin.description}</p>
                  {plugin.required_dependencies > 0 && (
                    <small>
                      Cette version annonce {plugin.required_dependencies}{" "}
                      dépendance(s) obligatoire(s).
                    </small>
                  )}
                  <button
                    className="button secondary"
                    onClick={() =>
                      onInstallPlugin(
                        pluginTarget,
                        plugin.project_id,
                        plugin.version,
                      )
                    }
                    disabled={
                      (pluginTarget === "local"
                        ? !canInstallLocalPlugin
                        : !canInstallPiPlugin) ||
                      busy ||
                      plugin.required_dependencies > 0
                    }
                  >
                    {plugin.required_dependencies > 0
                      ? "Dépendance obligatoire non gérée"
                      : "Préparer une copie avec ce plugin"}
                  </button>
                </div>
                <a href={plugin.project_url} target="_blank" rel="noreferrer">
                  Détails
                </a>
              </li>
            ))}
          </ul>
        )}
        {pluginSuggestions.length === 0 && !pluginLoading && !pluginError && (
          <p className="path-note">
            Aucun résultat affiché avant une recherche.
          </p>
        )}
        {pluginInstallError && (
          <p className="pi-error" role="alert">
            {pluginInstallError}
          </p>
        )}
        {pluginInstallResult && (
          <div className="pi-update-check ready" aria-live="polite">
            <strong>Copie candidate créée et contrôlée</strong>
            <span>
              {pluginInstallResult.plugin_name} · version{" "}
              {pluginInstallResult.plugin_version}
            </span>
            <span>
              La copie candidate est sélectionnée. Démarre-la depuis « Mon monde
              » pour vérifier son lancement. L’original et sa sauvegarde restent
              conservés.
            </span>
            <span>Sauvegarde : {pluginInstallResult.backup_path}</span>
            <span>
              SHA-256 sauvegarde : {pluginInstallResult.backup_sha256}
            </span>
          </div>
        )}
        {localInstallation?.previousServerPath && (
          <div className="pi-actions">
            <button
              className="button secondary"
              onClick={onRestorePreviousLocal}
              disabled={busy || Boolean(localServerStatus?.running)}
            >
              Revenir à la copie précédente
            </button>
            <small>
              L’autre copie est conservée. Arrête d’abord le serveur candidat
              s’il est démarré.
            </small>
          </div>
        )}
        {piInstallation?.previousServerFolder && (
          <div className="pi-actions">
            <button
              className="button secondary"
              onClick={onRestorePreviousPi}
              disabled={busy || Boolean(piServerStatus?.running)}
            >
              Revenir à la copie Pi précédente
            </button>
            <small>
              La copie candidate et la sauvegarde restent conservées. Arrête
              d’abord le serveur Pi candidat s’il est démarré.
            </small>
          </div>
        )}
        <p className="path-note">
          La compatibilité déclarée ne garantit pas le comportement avec Geyser,
          les joueurs Bedrock ou les consoles. Un plugin tiers peut empêcher le
          démarrage; dans ce cas l’original reste disponible. Les mises à jour
          automatiques restent bloquées tant que ce plugin est présent.
        </p>
      </section>
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
