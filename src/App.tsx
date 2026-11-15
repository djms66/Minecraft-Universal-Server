import { useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import brandLogo from "./assets/logo_minecraft_universal_server.png";
import {
  localeDirection,
  localeOptions,
  LocalizedTree,
  type Locale,
} from "./i18n";
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

function countryName(locale: Locale, region: string): string {
  return new Intl.DisplayNames([locale], { type: "region" }).of(region) ?? region;
}

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
  bedrock_transport_migration: boolean;
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
type ProgressMessage = {
  step: string;
  percent: number;
  indeterminate?: boolean;
};
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
  download_sha256?: string;
  download_size_gb: number;
  image_size_gb: number;
  devices: string[];
  init_format: string;
  minimum_server_memory_gb: number;
  initial_server_storage_gb: number;
  checked_at: string;
};
type PiOsCatalog = {
  recommended: PiOsRecommendation;
  options: PiOsRecommendation[];
};
type PiFlashResult = {
  detail: string;
  log_path: string;
  imager_version: string;
  hostname: string;
  target_id: string;
  target_name: string;
  target_size_bytes: number;
  target_model: string;
  expected_devices: string[];
};

const PI_FIRST_BOOT_WAIT_MS = 150_000;
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
  bedrock_nethernet_ready: boolean;
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
  java_resource_pack_url: string;
  java_resource_pack_sha1: string;
  java_resource_pack_required: boolean;
};
type PiServerStatus = {
  running: boolean;
  ready: boolean;
  java_port_open: boolean;
  bedrock_tcp_port_open: boolean;
  bedrock_port_open: boolean;
  bedrock_nethernet_ready: boolean;
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
  { id: "home", label: "Mon serveur", icon: "▦" },
  { id: "install", label: "Préparer le serveur", icon: "＋" },
  { id: "compat", label: "Compatibilité", icon: "◈" },
  { id: "backup", label: "Sauvegardes", icon: "▤" },
];

function readSavedPiInstallation(): PiInstallation | null {
  try {
    const value = JSON.parse(
      localStorage.getItem("monde-ensemble.pi-installation") || "null",
    ) as Partial<PiInstallation> | null;
    return value &&
      value.host &&
      value.username &&
      value.serverFolder &&
      value.address
      ? (value as PiInstallation)
      : null;
  } catch {
    return null;
  }
}

function readSavedLocalInstallation(): LocalInstallation | null {
  try {
    const value = JSON.parse(
      localStorage.getItem("monde-ensemble.local-installation") || "null",
    ) as Partial<LocalInstallation> | null;
    return value?.serverPath ? (value as LocalInstallation) : null;
  } catch {
    return null;
  }
}

function hasSavedServerRegistration() {
  return Boolean(readSavedLocalInstallation() || readSavedPiInstallation());
}

function App() {
  const [locale, setLocale] = useState<Locale>(() => {
    const stored = localStorage.getItem("monde-ensemble.locale");
    return localeOptions.some(([code]) => code === stored)
      ? (stored as Locale)
      : "fr";
  });
  const [page, setPage] = useState<Page>(() =>
    hasSavedServerRegistration() ? "home" : "install",
  );
  const [advanced, setAdvanced] = useState(false);
  const [showExistingPiFlow, setShowExistingPiFlow] = useState(false);
  const [justInstalled, setJustInstalled] = useState(false);
  const [installNotice, setInstallNotice] = useState("");
  const piCardWriteStarted = useRef(false);
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [systemError, setSystemError] = useState("");
  const [target, setTarget] = useState<"this" | "pi">("pi");
  const [piHost, setPiHost] = useState("");
  const [piUser, setPiUser] = useState("");
  const [piVerified, setPiVerified] = useState(false);
  const [piDiscovering, setPiDiscovering] = useState(false);
  const [piCandidates, setPiCandidates] = useState<DiscoveredPi[]>([]);
  const [piBootTargets, setPiBootTargets] = useState<PiBootTarget[]>([]);
  const [pendingPiTarget, setPendingPiTarget] = useState<PiBootTarget | null>(
    null,
  );
  const [pendingEmergencyErase, setPendingEmergencyErase] =
    useState<PiBootTarget | null>(null);
  const [piEmergencyEraseMode, setPiEmergencyEraseMode] = useState(false);
  const [piBootScanState, setPiBootScanState] = useState<
    "idle" | "scanning" | "done"
  >("idle");
  const [piBootScanError, setPiBootScanError] = useState("");
  const [piWifiSsid, setPiWifiSsid] = useState("");
  const [piWifiPassword, setPiWifiPassword] = useState("");
  const [piWifiCountry, setPiWifiCountry] = useState("FR");
  // A verified card session is recovered from the app's write log on launch,
  // so closing the app cannot strand a successfully prepared Raspberry Pi.
  const [piFlashResult, setPiFlashResult] = useState<PiFlashResult | null>(null);
  const [piFlashNeedsRecovery, setPiFlashNeedsRecovery] = useState(false);
  const [piFlashTargetAttempt, setPiFlashTargetAttempt] =
    useState<PiBootTarget | null>(null);
  const [piWriteCancelRequested, setPiWriteCancelRequested] = useState(false);
  const piFlashWritingStarted = useRef(false);
  const [piBootWaitStartedAt, setPiBootWaitStartedAt] = useState<number | null>(
    null,
  );
  const [piBootWaitElapsedMs, setPiBootWaitElapsedMs] = useState(0);
  const finishNewPiAutoStart = useRef(false);
  const finishNewPiRef = useRef<() => Promise<void>>(async () => {});
  const [piOsRecommendation, setPiOsRecommendation] =
    useState<PiOsRecommendation | null>(null);
  const [piOsOptions, setPiOsOptions] = useState<PiOsRecommendation[]>([]);
  const [piExpertMode, setPiExpertMode] = useState(false);
  const [piExpertModel, setPiExpertModel] = useState("");
  const [piExpertMemory, setPiExpertMemory] = useState("");
  const [piExpertImageUrl, setPiExpertImageUrl] = useState("");
  const [piFlashExpectedModel, setPiFlashExpectedModel] = useState("");
  const [piFlashExpectedDevices, setPiFlashExpectedDevices] = useState<
    string[]
  >([]);
  const [piOsLoading, setPiOsLoading] = useState(false);
  const [piOsError, setPiOsError] = useState("");
  const piExpertOptions = piOsOptions.filter(
    (option) =>
      piOsMatchesGeneration(option.devices, piExpertModel) &&
      Number(piExpertMemory) >= option.minimum_server_memory_gb,
  );
  const activePiOs = piExpertMode
    ? (piExpertOptions.find(
        (option) => option.image_url === piExpertImageUrl,
      ) ?? null)
    : piOsRecommendation;
  const [piInstallation, setPiInstallation] = useState<PiInstallation | null>(
    readSavedPiInstallation,
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
    useState<LocalInstallation | null>(readSavedLocalInstallation);
  const hasManagedServer = Boolean(localInstallation || piInstallation);
  const visibleNav = hasManagedServer
    ? nav
    : [{ id: "install" as const, label: "Installer", icon: "＋" }];
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
  const [piStatusRefreshing, setPiStatusRefreshing] = useState(false);
  const piStatusRefreshingRef = useRef(false);
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
    localStorage.setItem("monde-ensemble.locale", locale);
    document.documentElement.lang = locale;
    document.documentElement.dir = localeDirection(locale);
  }, [locale]);

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
    let active = true;
    localStorage.removeItem("monde-ensemble.pi-flash-session");
    localStorage.removeItem("monde-ensemble.pi-force-fresh");
    invoke<PiFlashResult | null>("recover_prepared_pi")
      .then((result) => {
        if (!active || !result) return;
        setPiFlashResult(result);
        setPiFlashExpectedModel("");
        setPiFlashExpectedDevices(result.expected_devices);
        setPiUser("minecraft");
        setTarget("pi");
        setPage("install");
        setConnection("Carte déjà préparée · reprise de l’installation sans nouvel effacement");
      })
      .catch(() => {
        // Recovery is best-effort; a missing or incomplete log leaves the
        // normal first-install flow untouched.
      });
    return () => {
      active = false;
    };
  }, []);

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
    invoke<PiOsCatalog>("get_pi_os_catalog")
      .then((catalog) => {
        if (active) {
          setPiOsRecommendation(catalog.recommended);
          setPiOsOptions(catalog.options);
        }
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
    channel.onmessage = (message) => {
      if (/^(Écriture et vérification de la carte|Écriture de Raspberry Pi OS|Vérification de la carte écrite)/.test(message.step)) {
        piFlashWritingStarted.current = true;
      }
      const macOsWriteWait =
        system?.os === "macos" &&
        message.step.startsWith("Écriture et vérification de la carte");
      setOperationProgress(
        macOsWriteWait
          ? {
              ...message,
              step: "Image prête. Écriture et vérification en cours. Ne retire pas la carte.",
              indeterminate: true,
            }
          : message,
      );
    };
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
      filters: [{ name: "Sauvegarde de serveur Minecraft", extensions: ["gz"] }],
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
    setInstallNotice("");
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
      let serverReady = false;
      try {
        const status = await invoke<LocalServerStatus>("inspect_local_server", {
          serverPath: result.path,
        });
        setLocalServerStatus(status);
        serverReady = status.ready;
        setLocalStatusError("");
      } catch (error) {
        setLocalStatusError(String(error));
      }
      setPage("home");
      setJustInstalled(true);
      setInstallNotice(
        serverReady
          ? "Installation terminée : le serveur est prêt."
          : "Installation terminée : consulte le résumé pour vérifier le démarrage du serveur.",
      );
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
    if (busy || piBootScanState === "scanning") return;
    setPiBootScanState("scanning");
    setPiBootScanError("");
    setPiBootTargets([]);
    try {
      const targets = await invoke<PiBootTarget[]>("discover_pi_boot_targets");
      setPiBootTargets(targets);
      setPiBootScanState("done");
    } catch (error) {
      setPiBootScanError(String(error));
      setPiBootScanState("idle");
    }
  };

  const reopenPiCardPreparation = async () => {
    if (busy) return;
    setBusy(true);
    setOperationError("");
    try {
      await invoke("clear_prepared_pi_recovery");
      setPiFlashResult(null);
      setPiFlashExpectedModel("");
      setPiFlashExpectedDevices([]);
      setPiBootTargets([]);
      setPiBootScanState("idle");
      setPiBootScanError("");
      setPiEulaAccepted(false);
      setPiEmergencyEraseMode(false);
      setConnection(
        "Préparation rouverte · configure le Wi-Fi puis détecte la carte pour la réécrire",
      );
    } catch (error) {
      setOperationError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const restartPiSetupFromZero = async () => {
    if (!piFlashResult || busy) return;
    const sizeGb = (piFlashResult.target_size_bytes / 1_000_000_000).toFixed(1);
    const confirmed = window.confirm(
      `Recommencer la préparation sur « ${piFlashResult.target_name} » (${sizeGb} Go, ${piFlashResult.target_id}) ? Rien ne sera écrit maintenant. L’application demandera une nouvelle confirmation avant d’effacer la carte.`,
    );
    if (!confirmed) return;

    try {
      await invoke("clear_prepared_pi_recovery");
    } catch (error) {
      setOperationError(String(error));
      return;
    }

    setPiFlashResult(null);
    setPiFlashExpectedModel("");
    setPiFlashExpectedDevices([]);
    setPiBootTargets([]);
    setPiBootScanState("idle");
    setPiBootScanError("");
    setPiWifiSsid("");
    setPiWifiPassword("");
    setPiWifiCountry("FR");
    setPiEulaAccepted(false);
    setConnection("Nouvelle préparation depuis zéro · carte à détecter");
  };

  const confirmEmergencyErase = async () => {
    const device = pendingEmergencyErase;
    if (!device || busy) return;
    setBusy(true);
    setPendingEmergencyErase(null);
    setOperationError("");
    setOperationProgress({
      step: "Effacement sécurisé du support amovible…",
      percent: 50,
      indeterminate: true,
    });
    try {
      const result = await invoke<string>("erase_removable_support", {
        targetId: device.id,
        targetName: device.name,
        targetSizeBytes: device.size_bytes,
        confirmErase: true,
      });
      setPiFlashResult(null);
      setPiFlashNeedsRecovery(false);
      setPiFlashTargetAttempt(null);
      setPiFlashExpectedModel("");
      setPiFlashExpectedDevices([]);
      setPiBootTargets([]);
      setPiBootScanState("idle");
      setPiBootScanError("");
      setPiEulaAccepted(false);
      setPiEmergencyEraseMode(false);
      setConnection(result);
    } catch (error) {
      setOperationError(String(error));
    } finally {
      setOperationProgress(null);
      setBusy(false);
    }
  };

  const flashPiOs = async (device: PiBootTarget) => {
    if (busy || !activePiOs) return;
    const minimumGb = Math.ceil(
      activePiOs.image_size_gb + activePiOs.initial_server_storage_gb,
    );
    if (device.size_bytes < minimumGb * 1_000_000_000) {
      setOperationError(
        `Ce support est trop petit. Prévois au moins ${minimumGb} Go.`,
      );
      return;
    }
    setOperationError("");
    if (system?.os === "macos") {
      setBusy(true);
      setOperationProgress({
        step: "Vérification de l’outil d’écriture avant l’effacement…",
        percent: 5,
      });
      try {
        await invoke<string>("check_macos_pi_writer");
      } catch (error) {
        setOperationError(String(error));
        setOperationProgress(null);
        setBusy(false);
        return;
      }
      setBusy(false);
      setOperationProgress(null);
    }
    setPendingPiTarget(device);
  };

  const confirmFlashPiOs = async () => {
    const device = pendingPiTarget;
    if (busy || piCardWriteStarted.current || !device || !activePiOs) return;
    piCardWriteStarted.current = true;
    piFlashWritingStarted.current = false;
    setPiFlashTargetAttempt(device);
    setPiFlashNeedsRecovery(false);
    setPiWriteCancelRequested(false);
    setPendingPiTarget(null);
    resetOperation();
    setPiFlashResult(null);
    setBusy(true);
    try {
      const result = await invoke<PiFlashResult>("prepare_pi_sd", {
        targetId: device.id,
        targetName: device.name,
        targetSizeBytes: device.size_bytes,
        targetModel: piExpertMode ? piExpertModel : "",
        imageUrl: activePiOs.image_url,
        imageSha256: activePiOs.image_sha256,
        wifiSsid: piWifiSsid,
        wifiPassword: piWifiPassword,
        wifiCountry: piWifiCountry,
        confirmErase: true,
        progress: progressChannel(),
      });
      setPiFlashResult(result);
      setPiFlashNeedsRecovery(false);
      setPiFlashExpectedModel(result.target_model);
      setPiFlashExpectedDevices(result.expected_devices);
      setPiWifiPassword("");
    } catch (error) {
      setOperationError(String(error));
      setOperationProgress(null);
      setPiFlashNeedsRecovery(piFlashWritingStarted.current);
    } finally {
      setBusy(false);
      setPiWriteCancelRequested(false);
      piCardWriteStarted.current = false;
    }
  };

  const cancelPiFlash = async () => {
    if (!busy || piWriteCancelRequested || system?.os !== "macos") return;
    const confirmed = window.confirm(
      "Arrêter maintenant l’écriture Raspberry Pi Imager ? La carte peut rester incomplète. L’application la laissera connectée et indiquera comment la récupérer.",
    );
    if (!confirmed) return;
    setPiWriteCancelRequested(true);
    try {
      await invoke("cancel_pi_sd");
    } catch (error) {
      setPiWriteCancelRequested(false);
      setOperationError(`Impossible d’arrêter l’écriture : ${String(error)}`);
    }
  };

  const installPiAt = async (host: string, username: string) => {
    resetOperation();
    setInstallNotice("");
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
        setPiFlashResult(null);
        let serverReady = false;
        try {
          const status = await invoke<PiServerStatus>("inspect_pi_server", {
            host: installation.host,
            username: installation.username,
            serverFolder: installation.serverFolder,
          });
          setPiServerStatus(status);
          serverReady = status.ready;
          setPiStatusError("");
        } catch (error) {
          setPiStatusError(
            `Le serveur est installé, mais son tableau de bord n’a pas pu être actualisé : ${String(error)}`,
          );
        }
        setPage("home");
        setJustInstalled(true);
        setInstallNotice(
          serverReady
            ? "Installation terminée : le serveur est prêt."
            : "Installation terminée : consulte le résumé pour vérifier le démarrage du serveur.",
        );
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
      const actualGeneration = device.model.match(
        /Raspberry Pi\s+([3-5])\b/i,
      )?.[1];
      const expectedGeneration = piFlashExpectedModel.match(/^pi([3-5])$/)?.[1];
      const selectedSystemSupportsPi = piFlashExpectedDevices.some(
        (entry) =>
          entry.toLowerCase().startsWith(`pi${actualGeneration}-`) &&
          entry.toLowerCase().includes("64bit"),
      );
      if (
        !actualGeneration ||
        (expectedGeneration && actualGeneration !== expectedGeneration) ||
        (piFlashExpectedDevices.length > 0 && !selectedSystemSupportsPi)
      ) {
        throw new Error(
          `Le Pi détecté (${device.model}) ne correspond pas au modèle prévu pour ce système. Aucun serveur n’a été installé. Vérifie le modèle et choisis un système compatible avant de recommencer.`,
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

  finishNewPiRef.current = finishNewPi;

  useEffect(() => {
    if (!piFlashResult || !piEulaAccepted || piBootWaitStartedAt === null) {
      setPiBootWaitElapsedMs(0);
      return;
    }

    const updateElapsedTime = () => {
      setPiBootWaitElapsedMs(
        Math.min(Date.now() - piBootWaitStartedAt, PI_FIRST_BOOT_WAIT_MS),
      );
    };
    updateElapsedTime();
    const interval = window.setInterval(updateElapsedTime, 250);
    return () => window.clearInterval(interval);
  }, [piFlashResult?.hostname, piEulaAccepted, piBootWaitStartedAt]);

  const piBootWaitComplete = piBootWaitElapsedMs >= PI_FIRST_BOOT_WAIT_MS;
  const piBootWaitProgress = Math.min(
    100,
    (piBootWaitElapsedMs / PI_FIRST_BOOT_WAIT_MS) * 100,
  );
  const piBootWaitRemainingSeconds = Math.ceil(
    (PI_FIRST_BOOT_WAIT_MS - piBootWaitElapsedMs) / 1000,
  );

  useEffect(() => {
    if (
      !piBootWaitComplete ||
      !piEulaAccepted ||
      !piFlashResult ||
      busy ||
      finishNewPiAutoStart.current
    ) {
      return;
    }
    finishNewPiAutoStart.current = true;
    void finishNewPiRef.current();
  }, [piBootWaitComplete, piEulaAccepted, piFlashResult, busy]);

  const refreshPiStatus = async (includeLogs = false) => {
    if (!piInstallation || piStatusRefreshingRef.current) return;
    piStatusRefreshingRef.current = true;
    setPiStatusRefreshing(true);
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
      piStatusRefreshingRef.current = false;
      setPiStatusRefreshing(false);
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
    home: "Mon serveur",
    install: hasManagedServer ? "Préparer un serveur" : "Préparer mon serveur",
    compat: "Ce qui peut jouer ensemble",
    backup: "Sauvegarder et déplacer",
    advanced: "Réglages avancés",
  };
  const serverConfirmedReady = Boolean(
    localServerStatus?.ready || piServerStatus?.ready,
  );
  const pageIntro: Record<Page, string> = {
    home: "Vérifie l’état du serveur et retrouve l’adresse à saisir dans Minecraft.",
    install: hasManagedServer
      ? serverConfirmedReady
        ? "Créer une installation séparée. Le serveur en ligne ne sera pas modifié."
        : "Le profil enregistré est conservé. L’état du serveur n’est pas confirmé; la nouvelle installation reste séparée."
      : "On avance ensemble, étape par étape, jusqu’à ta première connexion.",
    compat:
      "Les règles qui rendront les mises à jour sûres, expliquées sans jargon.",
    backup:
      "Tes constructions et réglages doivent pouvoir te suivre sur une autre machine.",
    advanced: "Les réglages avancés restent séparés de l’installation guidée.",
  };
  const installStep = justInstalled ? 4 : piFlashResult ? 3 : busy ? 2 : 1;

  return (
    <LocalizedTree locale={locale}>
    <div className="app-shell" lang={locale} dir={localeDirection(locale)}>
      <aside className="sidebar">
        <div className="brand">
          <img
            className="brand-logo"
            src={brandLogo}
            alt="Minecraft Universal Server — One world • Java + Bedrock • Any device"
          />
        </div>
        <div className="side-label">
          {hasManagedServer ? "TON SERVEUR" : "PREMIÈRE INSTALLATION"}
        </div>
        <nav>
          {visibleNav.map((item) => (
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
        {hasManagedServer && (
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
        )}
        <div className="server-mini">
          <span
            className={hasManagedServer ? "status-dot" : "status-dot empty"}
          />
          <div>
            <strong>
              {hasManagedServer
                ? serverConfirmedReady
                  ? "Serveur prêt"
                  : "Connexion enregistrée"
                : "Aucun serveur installé"}
            </strong>
            <small>
              {hasManagedServer
                ? serverConfirmedReady
                  ? "État confirmé en ligne"
                  : "Serveur non confirmé en ligne"
                : "Le guide va partir de zéro"}
            </small>
          </div>
        </div>
        <div className="side-version">
          M.U.S. <span>·</span> {__APP_VERSION__}
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div className="crumb">
            MINECRAFT UNIVERSAL SERVER <span>/</span> {pageTitle[page].toUpperCase()}
          </div>
          <div className="top-status">
            <label className="locale-picker">
              <span>Langue</span>
              <select
                value={locale}
                onChange={(event) => setLocale(event.target.value as Locale)}
                aria-label="Langue de l’application"
              >
                {localeOptions.map(([code, label]) => (
                  <option key={code} value={code}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
            <span className="build-version">Version {__APP_VERSION__}</span>
            <span>
              {hasManagedServer
                ? serverConfirmedReady
                  ? "SERVEUR EN LIGNE · MONDE PROTÉGÉ"
                  : "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER"
                : "ÉTAPE 1 · AUCUN SERVEUR INSTALLÉ"}
            </span>
            <div className="beginner-pill">
              <span>✦</span> MODE {advanced ? "AVANCÉ" : "DÉBUTANT"}
            </div>
          </div>
        </header>
        {installNotice && (
          <div className="install-notice" role="status" aria-live="assertive">
            <span aria-hidden="true">✓</span>
            <strong>{installNotice}</strong>
            <button
              type="button"
              aria-label="Fermer le message d’installation terminée"
              onClick={() => setInstallNotice("")}
            >
              Fermer
            </button>
          </div>
        )}
        <section className="page-wrap">
          <div className="page-heading">
            <div>
              <div className="eyebrow">TON SERVEUR MINECRAFT</div>
              <h1>{pageTitle[page]}</h1>
              <p>{pageIntro[page]}</p>
            </div>
            <div className="world-stamp" aria-hidden="true">
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
                locale={locale}
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
              piStatusRefreshing={piStatusRefreshing}
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
              justInstalled={justInstalled}
              onContinueFromInstall={() => setJustInstalled(false)}
              onCompat={() => setPage("compat")}
              onBackup={() => setPage("backup")}
            />
          )}
          {page === "install" && (
            <section className="install-layout">
              <div className="wizard-main">
                <ol className="install-progress" aria-label="Étapes d’installation">
                  {[
                    "Choisir où",
                    "Préparer",
                    "Démarrer",
                    "Jouer",
                  ].map((label, index) => {
                    const step = index + 1;
                    const state = step < installStep ? "complete" : step === installStep ? "current" : "";
                    return (
                      <li key={label} className={state} aria-current={step === installStep ? "step" : undefined}>
                        <span>{step < installStep ? "✓" : step}</span>
                        <strong>{label}</strong>
                      </li>
                    );
                  })}
                </ol>
                <h2 className="install-question">Où installer le serveur ?</h2>
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
                    <SystemPanel locale={locale} system={system} error={systemError} />
                    <LocalInstallPanel
                      locale={locale}
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
                    <div className="pi-wireless-fields pi-wifi-section">
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
                              Le Wi-Fi sera configuré pour le premier démarrage.
                            </small>
                            <details className="pi-wifi-security">
                              <summary>À savoir sur le mot de passe Wi-Fi</summary>
                              <p>
                                Il est écrit temporairement sur la carte pour
                                configurer le premier démarrage, puis sa
                                suppression est programmée. Pour ne pas l’écrire
                                sur la carte, relie le Pi au routeur avec un
                                câble Ethernet.
                              </p>
                            </details>
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
                                <option value="FR">{countryName(locale, "FR")}</option>
                                <option value="BE">{countryName(locale, "BE")}</option>
                                <option value="CH">{countryName(locale, "CH")}</option>
                                <option value="CA">{countryName(locale, "CA")}</option>
                                <option value="US">{countryName(locale, "US")}</option>
                                <option value="GB">{countryName(locale, "GB")}</option>
                                <option value="DE">{countryName(locale, "DE")}</option>
                                <option value="ES">{countryName(locale, "ES")}</option>
                                <option value="IT">{countryName(locale, "IT")}</option>
                                <option value="NL">{countryName(locale, "NL")}</option>
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
                    <div className="pi-fresh-entry">
                      <div className="eyebrow">
                        NOUVEAU PI · PARCOURS DÉBUTANT
                      </div>
                      <p>
                        Pi neuf ? Branche sa carte ici. L’application prépare
                        son système puis installe le serveur.
                      </p>
                      <button
                        className="text-button pi-existing-toggle"
                        type="button"
                        onClick={() => setShowExistingPiFlow((show) => !show)}
                      >
                        {showExistingPiFlow
                          ? "Masquer le parcours d’un Pi déjà configuré"
                          : "Mon Pi a déjà son système et le Wi-Fi"}
                      </button>
                    </div>
                    {showExistingPiFlow && (
                      <>
                        <div className="pi-separator">
                          <span>PI DÉJÀ CONFIGURÉ</span>
                        </div>
                        <div className="eyebrow">INSTALLATION DISTANTE</div>
                        <h3>Installer le serveur sur le Pi</h3>
                        <p>
                          L’application va trouver le Pi sur le réseau, vérifier
                          son modèle, sa mémoire et l’espace disponible, puis
                          installer et démarrer le serveur. Elle crée un nouveau
                          dossier sans supprimer les fichiers déjà présents.
                        </p>
                        <p className="pi-setup-help">
                          Pour éviter de te faire saisir une adresse IP,
                          commence par « Trouver mon Pi ». Il faut seulement
                          connaître le nom du compte créé sur le Pi. Cette
                          version exige que son accès sécurisé soit déjà activé;
                          elle ne te demandera ni ne conservera de mot de passe.
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
                              Le compte choisi lors de la première configuration
                              du Raspberry Pi.
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
                            locale={locale}
                            progress={operationProgress}
                            result={operationResult}
                            error={operationError}
                          />
                        )}
                      </>
                    )}
                    <details className="pi-os-recommendation pi-os-details" open={Boolean(piOsError)} aria-live="polite">
                      <summary>Système automatique : {piOsRecommendation?.name ?? "vérification en cours"}</summary>
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
                            Le Pi 3B+ est pris en charge avec un profil léger
                            automatique : mémoire Java réduite et portée de jeu
                            limitée. Pour plus de joueurs ou d’extensions, un Pi
                            4 ou 5 avec 4 Go ou plus est recommandé.
                            L’application choisit le profil après avoir vérifié
                            le Pi.
                          </small>
                        </>
                      )}
                    </details>
                    <section
                      className="pi-first-boot"
                      aria-labelledby="pi-fresh-title"
                    >
                      <h4 id="pi-fresh-title">
                        Préparer la carte du Raspberry Pi
                      </h4>
                      <p>
                        Renseigne le Wi-Fi si nécessaire, puis détecte la carte.
                        Elle ne sera effacée qu’après confirmation.
                      </p>


                      <details
                        className="pi-advanced-storage"
                        onToggle={(event) =>
                          setPiExpertMode(event.currentTarget.open)
                        }
                      >
                        <summary>
                          Mode expert · choisir le système
                        </summary>
                        <p>
                          Le choix automatique reste recommandé. Les systèmes
                          proposés viennent du catalogue officiel et sont
                          filtrés selon le modèle et la mémoire choisis.
                          L’application vérifiera le vrai modèle du Pi avant
                          d’installer le serveur.
                        </p>
                        <div className="pi-wireless-fields">
                          <div>
                            <label htmlFor="pi-expert-model">
                              Modèle du Raspberry Pi
                            </label>
                            <select
                              id="pi-expert-model"
                              value={piExpertModel}
                              onChange={(event) => {
                                setPiExpertModel(event.target.value);
                                setPiExpertMemory("");
                                setPiExpertImageUrl("");
                              }}
                              disabled={busy || Boolean(piFlashResult)}
                            >
                              <option value="">Choisir le modèle…</option>
                              <option value="pi3">Pi 3 / 3B / 3B+</option>
                              <option value="pi4">Pi 4</option>
                              <option value="pi5">Pi 5</option>
                            </select>
                          </div>
                          {piExpertModel && (
                            <div>
                              <label htmlFor="pi-expert-memory">
                                Mémoire de ce Pi
                              </label>
                              <select
                                id="pi-expert-memory"
                                value={piExpertMemory}
                                onChange={(event) => {
                                  setPiExpertMemory(event.target.value);
                                  setPiExpertImageUrl("");
                                }}
                                disabled={busy || Boolean(piFlashResult)}
                              >
                                <option value="">Choisir la mémoire…</option>
                                {(piExpertModel === "pi3"
                                  ? [1]
                                  : piExpertModel === "pi4"
                                    ? [1, 2, 4, 8]
                                    : [2, 4, 8, 16]
                                ).map((memory) => (
                                  <option key={memory} value={memory}>
                                    {memory} Go
                                  </option>
                                ))}
                              </select>
                            </div>
                          )}
                          {piExpertMemory && (
                            <div>
                              <label htmlFor="pi-expert-os">
                                Système à installer
                              </label>
                              <select
                                id="pi-expert-os"
                                value={piExpertImageUrl}
                                onChange={(event) =>
                                  setPiExpertImageUrl(event.target.value)
                                }
                                disabled={
                                  busy || Boolean(piFlashResult || piOsLoading)
                                }
                              >
                                <option value="">
                                  Choisir un système compatible…
                                </option>
                                {piExpertOptions.map((option) => (
                                  <option
                                    key={option.image_url}
                                    value={option.image_url}
                                  >
                                    {option.name} ·{" "}
                                    {new Date(
                                      `${option.release_date}T12:00:00`,
                                    ).toLocaleDateString("fr-FR")}
                                  </option>
                                ))}
                              </select>
                              {piExpertOptions.length === 0 && (
                                <small>
                                  Aucun système vérifié du catalogue ne
                                  correspond. Referme cette section pour garder
                                  le choix automatique.
                                </small>
                              )}
                              {piExpertOptions.length === 1 &&
                                piOsRecommendation?.image_url ===
                                  piExpertOptions[0].image_url && (
                                  <small>
                                    Pour cette combinaison de modèle et de
                                    mémoire, le catalogue ne propose qu’un seul
                                    système vérifié avec installation
                                    automatisée. Le choix automatique reste
                                    identique.
                                  </small>
                                )}
                            </div>
                          )}
                        </div>
                        {activePiOs && piExpertMode && (
                          <p className="pi-help">
                            Sélection : <strong>{activePiOs.name}</strong> ·{" "}
                            {activePiOs.image_size_gb} Go écrits · stockage
                            requis estimé :{" "}
                            {Math.ceil(
                              activePiOs.image_size_gb +
                                activePiOs.initial_server_storage_gb,
                            )}{" "}
                            Go.
                          </p>
                        )}
                      </details>
                      {activePiOs && (
                        <>
                          <p className="pi-storage-guidance">
                            Pour commencer, utilise une microSD vide d’au moins
                            32 Go. L’application vérifie l’espace avant toute
                            écriture.
                          </p>
                          <details className="pi-advanced-storage">
                            <summary>
                              Démarrer depuis un SSD USB
                            </summary>
                            <p>
                              Le démarrage USB dépend du modèle du Pi, de son
                              micrologiciel et de l’alimentation du boîtier. La
                              détection vérifie l’espace et l’écriture; seul le
                              premier démarrage confirme que le Pi accepte ce
                              SSD. Pour une première installation, la microSD
                              est le choix le plus simple.
                            </p>
                          </details>
                        </>
                      )}
                      <div className="pi-detect-support">
                        <button
                          className="button secondary"
                          onClick={() => {
                            setPiEmergencyEraseMode(false);
                            void findPiBootTargets();
                          }}
                          disabled={
                            busy ||
                            piBootScanState === "scanning"
                          }
                        >
                          {piBootScanState === "scanning"
                            ? "Recherche des supports…"
                            : "Détecter la carte SD ou le SSD"}
                        </button>
                        {system?.os === "macos" && (
                          <button
                            className="button destructive pi-emergency-erase-button"
                            type="button"
                            onClick={() => {
                              setPiEmergencyEraseMode(true);
                              void findPiBootTargets();
                            }}
                            disabled={busy || piBootScanState === "scanning"}
                          >
                            Effacer une carte SD ou une clé USB · urgence
                          </button>
                        )}
                        {piEmergencyEraseMode && (
                          <p className="pi-emergency-erase-hint" role="status">
                            Choisis le support à formater en ExFAT. Son nom et sa
                            capacité seront confirmés avant l’effacement; le
                            serveur du Pi n’est pas touché.
                          </p>
                        )}
                        {piBootScanState === "scanning" && (
                          <p
                            className="pi-scan-status"
                            role="status"
                            aria-live="polite"
                          >
                            Recherche des cartes SD et disques externes sur ce
                            Mac. Cela peut prendre quelques secondes.
                          </p>
                        )}
                        {piBootScanError && (
                          <p className="pi-error" role="alert">
                            {piBootScanError}
                          </p>
                        )}
                        {piBootScanState === "done" &&
                          piBootTargets.length === 0 && (
                            <p className="pi-help" role="status">
                              Aucune carte SD ni aucun disque amovible n’a été
                              détecté. Vérifie qu’elle est bien insérée, puis
                              relance la détection. Son format Mac ne pose pas
                              de problème. Rien n’a été écrit.
                            </p>
                          )}
                      </div>
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
                                    {device.size} · {device.connection}
                                  </span>
                                </div>
                                {piEmergencyEraseMode && (
                                  <button
                                    className="button destructive pi-flash-button"
                                    type="button"
                                    onClick={() => setPendingEmergencyErase(device)}
                                    disabled={busy || device.size_bytes <= 0}
                                  >
                                    Effacer ce support en ExFAT
                                  </button>
                                )}
                                {piBootTargets.length === 1 &&
                                  activePiOs &&
                                  device.size_bytes > 0 && (
                                    <em
                                      className={
                                        device.size_bytes >=
                                        Math.ceil(
                                          activePiOs.image_size_gb +
                                            activePiOs.initial_server_storage_gb,
                                        ) *
                                          1_000_000_000
                                          ? "storage-ready"
                                          : "storage-small"
                                      }
                                    >
                                      {device.size_bytes >=
                                      Math.ceil(
                                        activePiOs.image_size_gb +
                                          activePiOs.initial_server_storage_gb,
                                      ) *
                                        1_000_000_000
                                        ? "Assez de place pour l’OS et le serveur"
                                        : `Trop petit · prévois au moins ${Math.ceil(activePiOs.image_size_gb + activePiOs.initial_server_storage_gb)} Go`}
                                    </em>
                                  )}
                                {piBootTargets.length === 1 &&
                                  activePiOs &&
                                  device.size_bytes >=
                                    Math.ceil(
                                      activePiOs.image_size_gb +
                                        activePiOs.initial_server_storage_gb,
                                    ) *
                                      1_000_000_000 && (
                                    <button
                                      className="button secondary pi-flash-button"
                                      onClick={() => flashPiOs(device)}
                                      disabled={
                                        busy ||
                                        Boolean(piFlashResult) ||
                                        !activePiOs.image_sha256 ||
                                        (piExpertMode && !piExpertModel) ||
                                        (piExpertMode && !piExpertMemory) ||
                                        (piExpertMode && !piExpertImageUrl)
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
                      {(operationProgress || operationError) && (
                        <OperationStatus
                          locale={locale}
                          progress={operationProgress}
                          result={null}
                          error={operationError}
                        />
                      )}
                      {busy && system?.os === "macos" && piFlashWritingStarted.current && (
                        <div className="pi-write-control" aria-live="polite">
                          <p>
                            {piWriteCancelRequested
                              ? "Arrêt en cours… garde la carte connectée."
                              : "L’écriture dure plus longtemps que prévu ? Tu peux l’arrêter ici."}
                          </p>
                          <button
                            className="button secondary"
                            type="button"
                            onClick={() => void cancelPiFlash()}
                            disabled={piWriteCancelRequested}
                          >
                            {piWriteCancelRequested ? "Arrêt en cours…" : "Arrêter l’écriture"}
                          </button>
                        </div>
                      )}
                      {piFlashNeedsRecovery && piFlashTargetAttempt && (
                        <section className="pi-flash-recovery" role="alert" aria-live="polite">
                          <strong>La carte n’a peut-être pas fini d’être écrite.</strong>
                          <p>
                            Le support concerné est <b>{piFlashTargetAttempt.name}</b> · {piFlashTargetAttempt.size} · {piFlashTargetAttempt.id}. Ne le retire pas pendant l’arrêt d’Imager.
                          </p>
                          <ol>
                            <li>Quand l’écriture est arrêtée, éjecte la carte depuis le Finder, débranche-la, puis reconnecte-la.</li>
                            <li>Relance « Détecter la carte SD ou le SSD » et sélectionne uniquement ce support.</li>
                            <li>Si elle reste invisible, vérifie le petit verrou LOCK de l’adaptateur SD. Sur Mac, l’Utilitaire de disque peut afficher le support physique via Présentation → Afficher tous les appareils. Vérifie son nom et sa capacité avant toute action d’effacement.</li>
                          </ol>
                          {system?.os === "macos" && (
                            <button
                              className="button secondary"
                              type="button"
                              onClick={() => void invoke("open_macos_disk_utility").catch((error) => setOperationError(String(error)))}
                            >
                              Ouvrir l’Utilitaire de disque
                            </button>
                          )}
                          <small>Aucun effacement ni changement de format n’est lancé automatiquement.</small>
                        </section>
                      )}
                      {piFlashResult && (
                        <div className="pi-flash-success" aria-live="polite">
                          <strong>Carte prête · vérification terminée</strong>
                          <p>{piFlashResult.detail}</p>
                          <ol className="pi-first-boot-steps">
                            <li>Éteins le Raspberry Pi et insère la carte.</li>
                            <li>
                              Relie-le au même routeur que ce Mac avec un câble
                              Ethernet. Si tu as configuré le Wi-Fi plus haut,
                              il se connectera au démarrage.
                            </li>
                            <li>
                              Rallume le Pi et laisse-le allumé. Pour son
                              premier démarrage, l’application attendra 2 min
                              30 s après l’acceptation du contrat, puis cherchera
                              le Pi et lancera l’installation automatiquement.
                            </li>
                          </ol>
                          <div className="pi-first-boot-timer">
                            <small>
                              {!piEulaAccepted
                                ? "Délai de démarrage : 2 min 30 s"
                                : piBootWaitComplete
                                  ? "Délai terminé · recherche automatique du Pi…"
                                  : `Démarrage du Pi · ${Math.floor(piBootWaitRemainingSeconds / 60)} min ${String(piBootWaitRemainingSeconds % 60).padStart(2, "0")} s restantes`}
                            </small>
                            <div
                              className="pi-first-boot-timer-track"
                              role="progressbar"
                              aria-label="Délai de démarrage du Raspberry Pi"
                              aria-valuemin={0}
                              aria-valuemax={150}
                              aria-valuenow={Math.floor(
                                piBootWaitElapsedMs / 1000,
                              )}
                            >
                              <span
                                style={{ width: `${piBootWaitProgress}%` }}
                              />
                            </div>
                          </div>
                          <label className="eula-choice">
                            <input
                              type="checkbox"
                              checked={piEulaAccepted}
                              disabled={busy}
                              onChange={(event) => {
                                const accepted = event.target.checked;
                                setPiEulaAccepted(accepted);
                                setPiBootWaitElapsedMs(0);
                                setPiBootWaitStartedAt(
                                  accepted ? Date.now() : null,
                                );
                                finishNewPiAutoStart.current = false;
                              }}
                            />
                            <span>
                              J’accepte le contrat de licence Minecraft pour ce
                              serveur.
                            </span>
                          </label>
                          {!piEulaAccepted && (
                            <small className="pi-eula-hint">
                              Coche cette case pour autoriser l’installation du
                              serveur Minecraft.
                            </small>
                          )}
                          <button
                            className="button primary"
                            onClick={() => void finishNewPi()}
                            disabled={
                              busy || !piEulaAccepted || !piBootWaitComplete
                            }
                          >
                            {busy
                              ? "Recherche du Pi (jusqu’à 1 min)…"
                              : piBootWaitComplete
                                ? "Réessayer la détection du Pi"
                                : "Attendre le démarrage du Pi"}
                          </button>
                          <small className="pi-eula-hint">
                            Si le Pi n’est pas trouvé, laisse-le allumé, vérifie
                            qu’il est relié au même routeur, puis relance la
                            détection avec le bouton.
                          </small>
                          <small>
                            Outil officiel Raspberry Pi Imager{" "}
                            {piFlashResult.imager_version} · journal :{" "}
                            {piFlashResult.log_path}
                          </small>
                          {piFlashResult.target_id && (
                            <button
                              className="button secondary pi-restart-setup"
                              onClick={restartPiSetupFromZero}
                              disabled={busy}
                            >
                              Repartir de zéro · effacer et réinstaller
                            </button>
                          )}
                          <small>
                            Pour reprendre avec une nouvelle configuration
                            Wi-Fi, rouvre la préparation ci-dessous. Aucun
                            support n’est modifié par cette action; l’effacement
                            sera confirmé séparément après sa détection.
                          </small>
                          <button
                            className="button secondary pi-restart-setup"
                            type="button"
                            onClick={() => void reopenPiCardPreparation()}
                            disabled={busy}
                          >
                            Recommencer la préparation de la carte
                          </button>
                        </div>
                      )}
                    </section>
                  </div>
                )}
                <div className={serverConfirmedReady ? "notice safe" : "notice amber"}>
                  <span>{hasManagedServer ? "✓" : "!"}</span>
                  <p>
                    {serverConfirmedReady ? (
                      <>
                        <strong>Le serveur en ligne reste intact.</strong>
                        <br />
                        Une installation neuve est créée à côté de l’existante.
                      </>
                    ) : hasManagedServer ? (
                      <>
                        <strong>Le profil enregistré est conservé.</strong>
                        <br />
                        Le serveur n’est pas confirmé en ligne. La carte ne sera
                        écrite qu’après sa détection et ta confirmation.
                      </>
                    ) : (
                      <>
                        <strong>La carte choisie sera effacée.</strong>
                        <br />
                        Rien ne sera écrit avant ta confirmation du support
                        détecté.
                      </>
                    )}
                  </p>
                </div>
              </div>
              <aside className="wizard-aside">
                {!hasManagedServer && target === "pi" ? (
                  <>
                    <div className="aside-cap">À PRÉPARER</div>
                    <ul className="beginner-needs">
                      <li>
                        Raspberry Pi 3B+ ou plus récent, avec son alimentation
                      </li>
                      <li>
                        Carte microSD vide de 32 Go ou plus, branchée à ce Mac
                      </li>
                      <li>Wi-Fi et mot de passe, ou câble réseau</li>
                    </ul>
                    <div className="aside-bottom">
                      Je vérifie chaque étape. La carte ne sera effacée qu’après
                      confirmation.
                    </div>
                  </>
                ) : (
                  <>
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
                      {hasManagedServer && (
                        <>
                          <br />
                          Ton monde existant reste intact.
                        </>
                      )}
                    </div>
                  </>
                )}
              </aside>
            </section>
          )}
          {page === "compat" && (
            <Compatibility locale={locale} report={compatibility} error={compatibilityError} />
          )}
          {page === "backup" && (
            <Backups
              locale={locale}
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
              locale={locale}
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
          {pendingPiTarget && (
            <div className="erase-confirm-backdrop">
              <section
                className="erase-confirm"
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="erase-confirm-title"
                aria-describedby="erase-confirm-warning"
              >
                <div className="eyebrow">CONFIRMATION REQUISE</div>
                <h2 id="erase-confirm-title">Effacer cette carte SD ?</h2>
                <p>
                  Vérifie que le support détecté est bien celui du Raspberry Pi.
                </p>
                <dl className="erase-confirm-device">
                  <div>
                    <dt>Carte détectée</dt>
                    <dd>{pendingPiTarget.name}</dd>
                  </div>
                  <div>
                    <dt>Capacité</dt>
                    <dd>{pendingPiTarget.size}</dd>
                  </div>
                  <div>
                    <dt>Branchée sur</dt>
                    <dd>{pendingPiTarget.connection}</dd>
                  </div>
                </dl>
                <p className="erase-confirm-warning" id="erase-confirm-warning">
                  Tout le contenu de cette carte sera effacé. Les autres disques
                  ne seront pas touchés.
                </p>
                <div className="erase-confirm-actions">
                  <button
                    className="button secondary"
                    type="button"
                    autoFocus
                    onClick={() => setPendingPiTarget(null)}
                  >
                    Annuler
                  </button>
                  <button
                    className="button destructive"
                    type="button"
                    onClick={confirmFlashPiOs}
                    disabled={busy}
                  >
                    {busy
                      ? "Préparation en cours…"
                      : "Effacer la carte et continuer"}
                  </button>
                </div>
              </section>
            </div>
          )}
          {pendingEmergencyErase && (
            <div className="erase-confirm-backdrop">
              <section
                className="erase-confirm"
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="emergency-erase-title"
                aria-describedby="emergency-erase-warning"
              >
                <div className="eyebrow">EFFACEMENT D’URGENCE</div>
                <h2 id="emergency-erase-title">Effacer ce support ?</h2>
                <p>
                  macOS demandera son autorisation administrateur avant de le
                  formater en ExFAT.
                </p>
                <dl className="erase-confirm-device">
                  <div>
                    <dt>Support détecté</dt>
                    <dd>{pendingEmergencyErase.name}</dd>
                  </div>
                  <div>
                    <dt>Capacité</dt>
                    <dd>{pendingEmergencyErase.size}</dd>
                  </div>
                  <div>
                    <dt>Identifiant</dt>
                    <dd>{pendingEmergencyErase.id}</dd>
                  </div>
                  <div>
                    <dt>Branché sur</dt>
                    <dd>{pendingEmergencyErase.connection}</dd>
                  </div>
                </dl>
                <p className="erase-confirm-warning" id="emergency-erase-warning">
                  Tout le contenu de ce support sera supprimé. Vérifie son nom
                  et sa capacité. Le disque interne du Mac et le serveur du Pi
                  ne seront pas effacés.
                </p>
                <div className="erase-confirm-actions">
                  <button
                    className="button secondary"
                    type="button"
                    autoFocus
                    onClick={() => setPendingEmergencyErase(null)}
                  >
                    Annuler
                  </button>
                  <button
                    className="button destructive"
                    type="button"
                    onClick={() => void confirmEmergencyErase()}
                    disabled={busy}
                  >
                    Effacer ce support maintenant
                  </button>
                </div>
              </section>
            </div>
          )}
        </section>
        <footer className="footer">
          <span>Conçu pour garder ton monde intact.</span>
          <span>Cible : macOS · Windows · Raspberry Pi OS ARM64</span>
        </footer>
      </main>
    </div>
    </LocalizedTree>
  );
}

function piOsMatchesGeneration(devices: string[], model: string) {
  const generation = model.match(/^pi([3-5])$/)?.[1];
  return Boolean(
    generation &&
    devices.some(
      (device) =>
        device.toLowerCase().startsWith(`pi${generation}-`) &&
        device.toLowerCase().includes("64bit"),
    ),
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
  locale,
  system,
  error,
}: {
  locale: Locale;
  system: SystemInfo | null;
  error: string;
}) {
  return (
    <LocalizedTree locale={locale}>
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
              locale={locale}
              label="MÉMOIRE"
              value={
                system.memory_gb ? `${system.memory_gb} Go` : "Non détectée"
              }
              note="Mémoire vive"
            />
            <Spec
              locale={locale}
              label="DISQUE"
              value={system.disk_gb ? `${system.disk_gb} Go` : "Non détecté"}
              note="Taille totale"
            />
            <Spec
              locale={locale}
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
    </LocalizedTree>
  );
}

function LocalInstallPanel({
  locale,
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
  locale: Locale;
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
    <LocalizedTree locale={locale}>
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
          locale={locale}
          label="MINECRAFT JAVA"
          value={compatibility?.java_version ?? "Vérification…"}
        />
        <ReleaseItem
          locale={locale}
          label="MINECRAFT BEDROCK"
          value={compatibility?.bedrock_versions ?? "Vérification…"}
        />
        <ReleaseItem
          locale={locale}
          label="PAPER"
          value={compatibility?.paper_version ?? "Vérification…"}
        />
        <ReleaseItem
          locale={locale}
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
    </LocalizedTree>
  );
}

function ReleaseItem({ label, value, locale }: { label: string; value: string; locale: Locale }) {
  return (
    <LocalizedTree locale={locale}>
    <div className="release-item">
      <small>{label}</small>
      <strong>{value}</strong>
    </div>
    </LocalizedTree>
  );
}

function ExternalAccessGuide({
  locale,
  address,
  javaPort,
  bedrockPort,
  onCopyAddress,
}: {
  locale: Locale;
  address: string;
  javaPort: number;
  bedrockPort: number;
  onCopyAddress: (value: string, label: string) => void;
}) {
  if (!address || address === "127.0.0.1") {
    return (
      <LocalizedTree locale={locale}>
      <p className="pi-network-note">
        Actualise le serveur pour afficher son adresse sur le réseau avant de
        préparer l’accès depuis l’extérieur.
      </p>
      </LocalizedTree>
    );
  }

  return (
    <LocalizedTree locale={locale}>
    <details className="pi-network-guide">
      <summary>Jouer avec des amis depuis l’extérieur</summary>
      <p>
        Les voyants du tableau confirment les services sur le serveur. Pour des
        amis hors de la maison, la Freebox doit aussi transmettre les ports vers
        l’adresse locale ci-dessous.
      </p>
      <ol>
        <li>
          Garde cette adresse stable en réservant l’IP locale du serveur dans le
          DHCP de la Freebox.
        </li>
        <li>
          Dans l’application Free, ouvre <strong>Réseau</strong>, puis{" "}
          <strong>… → Paramètres réseau avancés → Redirection de port</strong>.
        </li>
        <li>
          Crée la règle Bedrock vers <strong>{address}</strong>, port externe et
          interne <strong>{bedrockPort}</strong>, en <strong>TCP et UDP</strong>
          . Si la Freebox demande un protocole par règle, crée une règle TCP et
          une règle UDP.
        </li>
        <li>
          Pour les joueurs Java hors de la maison, ajoute aussi le port{" "}
          <strong>{javaPort} en TCP</strong>, vers la même adresse.
        </li>
        <li>
          Donne aux joueurs l’adresse IP publique indiquée dans l’application
          Free, dans <strong>Server internet → Autres informations</strong>, et
          le port Bedrock <strong>{bedrockPort}</strong>. Teste depuis un autre
          réseau, par exemple en coupant le Wi‑Fi du téléphone.
        </li>
      </ol>
      <div className="pi-network-values">
        <div>
          <span>IP LOCALE À SÉLECTIONNER DANS LA RÈGLE</span>
          <code>{address}</code>
          <button
            className="address-copy"
            type="button"
            onClick={() => onCopyAddress(address, "IP locale du serveur")}
          >
            Copier l’IP locale
          </button>
        </div>
        <div>
          <span>PORTS DU SERVEUR</span>
          <code>
            Bedrock {bedrockPort} · Java {javaPort}
          </code>
        </div>
      </div>
      <p className="pi-network-note">
        Cette étape ne change rien au serveur. N’active pas la DMZ et ne
        redirige pas les ports d’administration.
      </p>
      <a
        href="https://assistance.free.fr/articles/1395"
        target="_blank"
        rel="noreferrer"
      >
        Voir le guide officiel Free pour la redirection de port
      </a>
    </details>
    </LocalizedTree>
  );
}

function Spec({
  locale,
  label,
  value,
  note,
}: {
  locale: Locale;
  label: string;
  value: string;
  note: string;
}) {
  return (
    <LocalizedTree locale={locale}>
    <div className="spec">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{note}</small>
    </div>
    </LocalizedTree>
  );
}

function Home({
  locale,
  justInstalled,
  onContinueFromInstall,
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
  piStatusRefreshing,
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
  locale: Locale;
  justInstalled: boolean;
  onContinueFromInstall: () => void;
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
  piStatusRefreshing: boolean;
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

  if (justInstalled) {
    const isPi = Boolean(piInstallation);
    const ready = isPi ? piServerStatus?.ready : localServerStatus?.ready;
    const address = isPi
      ? piServerStatus?.address || piInstallation?.address || ""
      : localServerStatus?.address || "";
    const javaPort = isPi
      ? piServerStatus?.java_port || 25565
      : localServerStatus?.java_port || 25565;
    const bedrockPort = isPi
      ? piServerStatus?.bedrock_port || 19132
      : localServerStatus?.bedrock_port || 19132;
    const javaVersion = isPi
      ? piServerStatus?.minecraft_java
      : localServerStatus?.minecraft_java;
    const bedrockVersion = isPi
      ? piServerStatus?.minecraft_bedrock
      : localServerStatus?.minecraft_bedrock;
    const statusError = isPi ? piStatusError : localStatusError;

    return (
      <LocalizedTree locale={locale}>
      <section
        className="install-finish"
        aria-labelledby="install-finish-title"
        role="status"
        aria-live="assertive"
      >
        <div className="install-finish-mark" aria-hidden="true">
          {ready ? "✓" : "…"}
        </div>
        <div className="eyebrow">
          INSTALLATION {ready ? "VÉRIFIÉE" : "TERMINÉE"}
        </div>
        <h2 id="install-finish-title">
          {ready ? "Ton serveur est prêt." : "Ton serveur est installé."}
        </h2>
        <p>
          {ready
            ? "Utilise l’adresse ci-dessous dans Minecraft. Garde cette fenêtre ouverte pendant le premier essai."
            : statusError
              ? "L’installation est terminée, mais le contrôle du serveur a échoué. Le tableau de bord indiquera ce qu’il faut vérifier."
              : "Le serveur termine son démarrage. Le tableau de bord permet de vérifier quand il répond."}
        </p>
        {address ? (
          <div className="install-finish-addresses">
            <div>
              <span>PC ET MAC · MINECRAFT JAVA</span>
              <strong>
                {address}:{javaPort}
              </strong>
              <button
                className="address-copy"
                type="button"
                onClick={() =>
                  copyAddress(`${address}:${javaPort}`, "Adresse Java")
                }
              >
                Copier l’adresse
              </button>
            </div>
            <div>
              <span>MOBILE, CONSOLE ET WINDOWS · BEDROCK</span>
              <strong>{address}</strong>
              <small>Port : {bedrockPort}</small>
              <button
                className="address-copy"
                type="button"
                onClick={() => copyAddress(address, "Adresse Bedrock")}
              >
                Copier l’adresse
              </button>
            </div>
          </div>
        ) : (
          <p className="install-finish-missing">
            L’adresse est en cours de vérification. Le tableau de bord affichera
            l’état exact.
          </p>
        )}
        <p className="install-finish-versions">
          Installé : Java {javaVersion || "à vérifier"} · Bedrock compatible{" "}
          {bedrockVersion || "à vérifier"}
        </p>
        <p className="install-finish-next">
          Ensuite, ouvre le tableau de bord et choisis « Ajouter ou retirer un
          joueur » pour gérer la liste d’accès.
        </p>
        {copiedAddress && <p role="status">{copiedAddress}</p>}
        <button
          className="button primary"
          type="button"
          onClick={onContinueFromInstall}
        >
          Terminé · ouvrir le tableau de bord
        </button>
      </section>
      </LocalizedTree>
    );
  }

  return (
    <LocalizedTree locale={locale}>
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
          <a className="player-shortcut" href="#local-players-title">
            <span aria-hidden="true">＋</span> Ajouter ou retirer un joueur
          </a>
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
            <span className={localServerStatus?.bedrock_nethernet_ready ? "ok" : ""}>
              Geyser NetherNet{" "}
              {localServerStatus?.bedrock_nethernet_ready
                ? "· prêt"
                : "· à corriger"}
            </span>
          </div>
          <ExternalAccessGuide
            locale={locale}
            address={localServerStatus?.address || ""}
            javaPort={localServerStatus?.java_port || 25565}
            bedrockPort={localServerStatus?.bedrock_port || 19132}
            onCopyAddress={copyAddress}
          />
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
              {localUpdateCheck.bedrock_transport_migration && (
                <span>
                  Geyser doit être migré vers NetherNet pour les clients Bedrock
                  récents. Cette correction sera incluse dans la sauvegarde et
                  la mise à jour coordonnées.
                </span>
              )}
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
                        Bedrock · Xbox, PlayStation, Switch, mobile, Windows
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
                <p className="player-tools-note edition-hint">
                  Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox,
                  PlayStation, Switch, téléphone, tablette ou Minecraft pour
                  Windows. Utilise le nom de l’édition choisie.
                </p>
                {localPlayerPlatform === "bedrock" && (
                  <p className="player-tools-note">
                    Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone,
                    tablette ou Minecraft Bedrock sur Windows. Le gamertag Xbox
                    n’est pas un nom Java. Si Floodgate indique que le joueur
                    n’existe pas, il doit d’abord avoir rejoint une fois ce
                    serveur Geyser; la protection ne sera pas désactivée par
                    surprise.
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
          <a className="player-shortcut" href="#pi-players-title">
            <span aria-hidden="true">＋</span> Ajouter ou retirer un joueur
          </a>
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
            <span className={piServerStatus?.bedrock_nethernet_ready ? "ok" : ""}>
              Geyser NetherNet{" "}
              {piServerStatus?.bedrock_nethernet_ready
                ? "· prêt"
                : "· à corriger"}
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
                disabled={busy || piStatusRefreshing}
              >
                {piStatusRefreshing
                  ? "Lecture…"
                  : "Actualiser l’état et le journal"}
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
                  {piUpdateCheck.bedrock_transport_migration && (
                    <p>
                      Geyser doit être migré vers NetherNet pour Bedrock récent.
                      L’application inclura ce réglage dans la sauvegarde et la
                      mise à jour coordonnées.
                    </p>
                  )}
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
          <ExternalAccessGuide
            locale={locale}
            address={piServerStatus?.address || ""}
            javaPort={piServerStatus?.java_port || 25565}
            bedrockPort={piServerStatus?.bedrock_port || 19132}
            onCopyAddress={copyAddress}
          />
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
                  : piStatusError
                    ? "Pi inaccessible"
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
                      Bedrock · Xbox, PlayStation, Switch, mobile, Windows
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
              <p className="player-tools-note edition-hint">
                Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox,
                PlayStation, Switch, téléphone, tablette ou Minecraft pour
                Windows. Utilise le nom de l’édition choisie.
              </p>
              {piPlayerPlatform === "bedrock" && (
                <p className="player-tools-note">
                  Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone,
                  tablette ou Minecraft Bedrock sur Windows. Le gamertag Xbox
                  n’est pas un nom Java. Si Floodgate indique que le joueur
                  n’existe pas, il doit d’abord avoir rejoint une fois ce
                  serveur Geyser; la protection ne sera pas désactivée par
                  surprise.
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
                  (piStatusError
                    ? "L’application ne joint pas le Pi. Vérifie qu’il est allumé et sur le même réseau, puis actualise l’état avant de gérer la whitelist."
                    : piServerStatus?.running
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
    </LocalizedTree>
  );
}

function rangeIncludesVersion(
  range: string | undefined,
  target: string,
): boolean | null {
  if (!range) return null;
  const candidates = Array.from(
    range.matchAll(/(\d+(?:\.\d+){1,2})(?:\s*[-–]\s*(\d+(?:\.\d+){1,2}))?/g),
  );
  if (candidates.length === 0) return null;

  const compare = (left: string, right: string) => {
    const leftParts = left.split(".").map(Number);
    const rightParts = right.split(".").map(Number);
    for (
      let index = 0;
      index < Math.max(leftParts.length, rightParts.length);
      index += 1
    ) {
      const difference = (leftParts[index] ?? 0) - (rightParts[index] ?? 0);
      if (difference !== 0) return Math.sign(difference);
    }
    return 0;
  };

  return candidates.some(([, start, end]) => {
    if (!start) return false;
    return (
      compare(target, start) >= 0 &&
      (end ? compare(target, end) <= 0 : compare(target, start) === 0)
    );
  });
}

function Compatibility({
  locale,
  report,
  error,
}: {
  locale: Locale;
  report: CompatibilityReport | null;
  error: string;
}) {
  const terracottaClientSupported = rangeIncludesVersion(
    report?.bedrock_versions,
    "26.62",
  );
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
    <LocalizedTree locale={locale}>
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
              compte Xbox séparé et un relais côté serveur; Minecraft Universal Server ne
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
              <ReleaseItem locale={locale} label="PAPER" value={report.paper_version} />
              <ReleaseItem locale={locale} label="GEYSER" value={report.geyser_version} />
              <ReleaseItem locale={locale} label="FLOODGATE" value={report.floodgate_version} />
              <ReleaseItem
                locale={locale}
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
            L’application configure le pont Bedrock sur ton serveur et contrôle
            NetherNet automatiquement. Tu n’as pas à ajouter un serveur de test
            dans Minecraft.
          </p>
          <ol>
            <li>
              Sur le tableau de bord, vérifie « Geyser NetherNet », « Signal
              Bedrock TCP » et « Jeu Bedrock UDP ».
            </li>
            <li>
              S’ils sont tous prêts et que l’iPad reçoit encore cette erreur,
              ne modifie ni le monde ni la liste blanche : l’erreur peut venir
              de cette version de Minecraft Bedrock.
            </li>
          </ol>
          <small>
            Plage Bedrock actuellement publiée par Geyser :{" "}
            {report?.bedrock_versions ?? "non vérifiée pour l’instant"}. Le
            tableau « Compatibilité » relit cette plage à chaque contrôle.
          </small>
          <p className="terracotta-diagnosis">
            {terracottaClientSupported === true
              ? "La version 1.26.62 de ta capture est comprise dans la plage annoncée par le catalogue actuel. Cela ne prouve pas que ton serveur utilise déjà ce build ni que le signal réseau passe."
              : terracottaClientSupported === false
                ? "La plage annoncée par le catalogue actuel ne comprend pas la version 1.26.62 de ta capture. Cette passerelle ne peut donc pas encore confirmer sa compatibilité."
                : "La plage du catalogue n’a pas pu être comparée à la version 1.26.62 de ta capture."}{" "}
            Le build Geyser, NetherNet et les deux ports sont vérifiés par
            l’application à chaque installation et mise à jour coordonnées.
            Cette erreur ne signifie pas que le monde est abîmé.
          </p>
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
    </LocalizedTree>
  );
}

function Backups({
  locale,
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
  locale: Locale;
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
    <LocalizedTree locale={locale}>
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
            Utilise l’archive choisie ci-dessus. Minecraft Universal Server vérifie son
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
        <OperationStatus locale={locale} progress={progress} result={result} error={error} />
      )}
      {!progress && (result || error) && (
        <OperationStatus locale={locale} progress={null} result={result} error={error} />
      )}
      <div className="source-note">
        L’archive complète peut contenir des extensions sous licence. Monde
        Ensemble copie les fichiers sélectionnés; il ne récupère ni ne remplace
        les licences des extensions.
      </div>
    </div>
    </LocalizedTree>
  );
}

function OperationStatus({
  locale,
  progress,
  result,
  error,
}: {
  locale: Locale;
  progress: ProgressMessage | null;
  result: OperationResult | null;
  error: string;
}) {
  const indeterminate = Boolean(progress?.indeterminate);
  return (
    <LocalizedTree locale={locale}>
    <div className={error ? "inline-error" : "operation-success"}>
      {progress && (
        <>
          <div className="progress-head">
            <span>{progress.step}</span>
            <b>{indeterminate ? "EN COURS" : `${progress.percent}%`}</b>
          </div>
          <div
            className={`progress-track${indeterminate ? " indeterminate" : ""}`}
            role="progressbar"
            aria-valuemin={indeterminate ? undefined : 0}
            aria-valuemax={indeterminate ? undefined : 100}
            aria-valuenow={indeterminate ? undefined : progress.percent}
            aria-valuetext={
              indeterminate
                ? "Opération en cours, progression non disponible"
                : undefined
            }
          >
            <i
              style={
                indeterminate ? undefined : { width: `${progress.percent}%` }
              }
            />
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
    </LocalizedTree>
  );
}

function GameSettingsPanel({
  locale,
  localInstallation,
  localServerStatus,
  piInstallation,
  piServerStatus,
  busy,
}: {
  locale: Locale;
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
    <LocalizedTree locale={locale}>
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
              <div className="java-resource-pack-settings">
                <div className="eyebrow">PACK DE RESSOURCES · JAVA</div>
                <p className="path-note">
                  Pour les joueurs Java, indique un lien HTTPS direct vers le
                  fichier ZIP hébergé en ligne. Minecraft Universal Server ne l’héberge pas
                  et ne vérifie pas que ce lien est accessible depuis Internet.
                  Ce réglage ne convertit pas le pack pour Bedrock.
                </p>
                <div className="game-settings-grid">
                  <label className="game-setting-wide">
                    Adresse HTTPS directe du ZIP
                    <input
                      type="url"
                      value={settings.java_resource_pack_url}
                      maxLength={2048}
                      placeholder="https://…/pack.zip"
                      onChange={(event) => {
                        const url = event.target.value.trim();
                        change("java_resource_pack_url", url);
                        if (!url) {
                          change("java_resource_pack_sha1", "");
                          change("java_resource_pack_required", false);
                        }
                      }}
                      disabled={saving || busy}
                    />
                  </label>
                  <label className="game-setting-wide">
                    SHA-1 du ZIP · 40 caractères hexadécimaux
                    <input
                      value={settings.java_resource_pack_sha1}
                      maxLength={40}
                      autoCapitalize="none"
                      spellCheck={false}
                      placeholder="Facultatif, mais recommandé"
                      onChange={(event) =>
                        change(
                          "java_resource_pack_sha1",
                          event.target.value.trim(),
                        )
                      }
                      disabled={saving || busy}
                    />
                  </label>
                </div>
                <div className="game-settings-toggles">
                  <label>
                    <input
                      type="checkbox"
                      checked={settings.java_resource_pack_required}
                      onChange={(event) =>
                        change(
                          "java_resource_pack_required",
                          event.target.checked,
                        )
                      }
                      disabled={
                        saving || busy || !settings.java_resource_pack_url
                      }
                    />
                    <span>
                      Refuser l’accès Java si le joueur refuse ce pack
                    </span>
                  </label>
                </div>
                {settings.java_resource_pack_required && (
                  <small className="path-note">
                    Un joueur Java qui refuse le pack sera déconnecté. Enlève
                    cette option si tu veux laisser le choix.
                  </small>
                )}
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
    </LocalizedTree>
  );
}

function Advanced({
  locale,
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
  locale: Locale;
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
    <LocalizedTree locale={locale}>
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
                ? <>
                    {system.os} {system.version} · {system.architecture} ·{" "}
                    mémoire {system.memory_gb ?? "inconnue"} Go · disque libre{" "}
                    {system.free_disk_gb ?? "inconnu"} Go
                  </>
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
        locale={locale}
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
          <OperationStatus locale={locale} progress={pluginProgress} result={null} error="" />
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
    </LocalizedTree>
  );
}

export default App;
