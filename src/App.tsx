import { useEffect, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
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
  geyser_version: string;
  floodgate_version: string;
  bedrock_versions: string;
  java_version: string;
  checked_at: string;
  source: string;
};
type ProgressMessage = { step: string; percent: number };
type OperationResult = { path: string; detail: string; sha256: string | null };

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
  const [connection, setConnection] = useState("");
  const [busy, setBusy] = useState(false);
  const [compatibility, setCompatibility] =
    useState<CompatibilityReport | null>(null);
  const [compatibilityError, setCompatibilityError] = useState("");
  const [eulaAccepted, setEulaAccepted] = useState(false);
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
    if (page !== "compat" && page !== "install") return;
    setCompatibilityError("");
    invoke<CompatibilityReport>("get_compatibility")
      .then(setCompatibility)
      .catch((e) => setCompatibilityError(String(e)));
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
    setConnection("Vérification de l’accès…");
    try {
      setConnection(
        await invoke<string>("probe_ssh", { host: piHost, username: piUser }),
      );
    } catch (e) {
      setConnection(String(e));
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
          MONDE ENSEMBLE <span>·</span> 0.3.0
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
                    <h3>Tu pars de zéro sur le Raspberry Pi ?</h3>
                    <p>
                      Prépare-le avec Raspberry Pi Imager. L’outil indique les
                      systèmes compatibles avec le modèle choisi; pour le
                      serveur, privilégie Raspberry Pi OS 64 bits s’il est
                      proposé. Choisis ensuite la carte ou le SSD par son nom et
                      sa capacité, puis active l’accès distant. L’image efface
                      le support sélectionné : vérifie-le soigneusement.
                    </p>
                    <button
                      className="button secondary"
                      onClick={() =>
                        openUrl("https://www.raspberrypi.com/software/")
                      }
                    >
                      Ouvrir Raspberry Pi Imager ↗
                    </button>
                    <div className="pi-separator">
                      <span>PI DÉJÀ PRÉPARÉ</span>
                    </div>
                    <h3>Relier le Raspberry Pi</h3>
                    <p>
                      Le Pi doit être allumé sur le même réseau et l’accès
                      sécurisé déjà activé. Aucun mot de passe ne sera demandé
                      ni enregistré ici.
                    </p>
                    <div className="form-row">
                      <label>
                        Nom ou adresse du Pi
                        <input
                          value={piHost}
                          onChange={(e) => setPiHost(e.target.value)}
                          placeholder="raspberrypi.local"
                          autoComplete="off"
                        />
                      </label>
                      <label>
                        Nom du compte
                        <input
                          value={piUser}
                          onChange={(e) => setPiUser(e.target.value)}
                          placeholder="ex. daniel"
                          autoComplete="off"
                        />
                      </label>
                    </div>
                    <button
                      className="button secondary"
                      onClick={testPi}
                      disabled={busy || !piHost || !piUser}
                    >
                      {busy ? "Vérification…" : "Vérifier la connexion"}
                    </button>
                    {connection && (
                      <div className="connection-result">{connection}</div>
                    )}
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
          {page === "advanced" && <Advanced system={system} />}
        </section>
        <footer className="footer">
          <span>Conçu pour garder ton monde intact.</span>
          <span>Cible : macOS · Windows · Raspberry Pi OS ARM64</span>
        </footer>
      </main>
    </div>
  );
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
        {busy ? "Préparation en cours…" : "Créer le nouveau serveur"}
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
            Le serveur actuel est resté intact. Cette copie n’a pas été
            démarrée.
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
  onInstall,
  onCompat,
  onBackup,
}: {
  system: SystemInfo | null;
  systemError: string;
  onInstall: () => void;
  onCompat: () => void;
  onBackup: () => void;
}) {
  return (
    <>
      <div className="hero">
        <div className="hero-copy">
          <div className="hero-kicker">
            <span /> PARCOURS SANS ÉCRITURE
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
    ["PC / Mac Java", "Oui", "Serveur Java"],
    ["Windows Bedrock", "Oui", "Par le pont Geyser"],
    ["Téléphones / tablettes", "Oui", "Par le pont Geyser"],
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
            <h3>Qui peut rejoindre ?</h3>
          </div>
          <span>Profil guidé</span>
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
            Les mises à jour du serveur existant ne sont pas encore activées.
          </strong>
          <br />
          Cette version prépare une installation neuve et protège les copies par
          sauvegarde/restauration. Elle ne remplace pas Paper ni les extensions
          du monde en service. Pour éviter une perte de monde ou de plugins, une
          mise à jour n’est proposée que lorsque le contrôle de compatibilité
          complet sera disponible.
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

function Advanced({ system }: { system: SystemInfo | null }) {
  return (
    <div className="content-column">
      <div className="advanced-intro">
        <div className="advanced-mark">⚙</div>
        <div>
          <div className="eyebrow">MODE AVANCÉ</div>
          <h2>Plus de contrôle, les mêmes garde-fous.</h2>
          <p>
            Les réglages disponibles apparaissent seulement quand leur effet et
            leur retour arrière sont connus.
          </p>
        </div>
      </div>
      <div className="advanced-list">
        <div>
          <span>01</span>
          <p>
            <strong>Versions et compatibilité</strong>
            <small>
              Profil actif, profils proposés et raison des mises à jour
              bloquées.
            </small>
          </p>
          <b>Catalogue requis</b>
        </div>
        <div>
          <span>02</span>
          <p>
            <strong>Extensions et packs</strong>
            <small>
              Ajouter des joueurs, extensions et ressources avec vérification
              préalable.
            </small>
          </p>
          <b>Vérification avant ajout</b>
        </div>
        <div>
          <span>03</span>
          <p>
            <strong>Journal détaillé</strong>
            <small>
              Diagnostic exportable sans mots de passe ni secrets d’accès.
            </small>
          </p>
          <b>Journal local</b>
        </div>
        <div>
          <span>04</span>
          <p>
            <strong>Machine détectée</strong>
            <small>
              {system
                ? `${system.os} ${system.version} · ${system.architecture} · ${system.memory_gb ?? "?"} Go`
                : "Lecture en cours"}
            </small>
          </p>
          <b>Lecture seule</b>
        </div>
      </div>
      <div className="notice amber">
        <span>!</span>
        <p>
          <strong>Une mise à jour n’est jamais forcée.</strong>
          <br />
          Si le catalogue ne confirme pas la compatibilité de chaque extension
          de ton monde, le profil actuel reste proposé.
        </p>
      </div>
    </div>
  );
}

export default App;
