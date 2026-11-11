import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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

  useEffect(() => {
    invoke<SystemInfo>("inspect_system")
      .then(setSystem)
      .catch((e) => setSystemError(String(e)));
  }, []);

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
    home: "Cette préversion explique le parcours et contrôle les appareils; elle ne modifie pas encore le serveur.",
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
            <strong>Mode vérification</strong>
            <small>Aucune écriture sur le serveur</small>
          </div>
        </div>
        <div className="side-version">
          MONDE ENSEMBLE <span>·</span> 0.2
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div className="crumb">
            MONDE ENSEMBLE <span>/</span> {pageTitle[page].toUpperCase()}
          </div>
          <div className="top-status">
            <span>LECTURE SEULE · VERSION DE TRAVAIL</span>
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
                  <SystemPanel system={system} error={systemError} />
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
                    <strong>Rien n’est modifié à cette étape.</strong>
                    <br />
                    L’application mesure la machine et vérifie l’espace
                    disponible avant de préparer quoi que ce soit.
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
          {page === "compat" && <Compatibility />}
          {page === "backup" && <Backups />}
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
            Un serveur Java peut aussi accueillir les joueurs Bedrock. Cette
            première version prépare le parcours et vérifie les machines; elle
            n’installe ni ne met encore à jour le serveur.
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
            <strong>Cette version ne change rien au serveur.</strong>
            <br />
            Elle détecte la machine, explique les règles et vérifie un accès SSH
            existant.
          </p>
        </div>
      </div>
    </>
  );
}

function Compatibility() {
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
            et Floodgate. Cette version explique le principe; le contrôle des
            versions et extensions sera activé dans le moteur serveur.
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
      <div className="terracotta-note">
        <div className="terracotta-mark">!</div>
        <div>
          <div className="eyebrow">ERREUR TERRACOTTA · IPAD</div>
          <strong>Bedrock 1.26.62 · InitialConnection-149</strong>
          <p>
            L’erreur seule ne révèle pas sa cause. Le dernier relevé connu du
            serveur indiquait Paper 26.2 et Geyser sur le port 19133 avec
            RakNet; ces informations doivent être relues avant toute correction.
            Cette préversion n’accède pas aux journaux et n’a rien modifié.
          </p>
        </div>
      </div>
      <div className="notice amber">
        <span>!</span>
        <p>
          <strong>Les mises à jour groupées sont le but du moteur.</strong>
          <br />
          Avant d’activer une mise à jour, il devra confirmer que la passerelle
          Bedrock et chaque extension installée sont prêtes. Si une pièce
          manque, il gardera la version actuelle.
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

function Backups() {
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
      <div className="notice amber">
        <span>i</span>
        <p>
          <strong>Dans cette version, ces étapes sont des explications.</strong>
          <br />
          Aucune sauvegarde ni restauration n’est lancée. Le moteur de copie
          sécurisée reste à construire.
        </p>
      </div>
      <div className="backup-options">
        <div className="backup-option">
          <span>01</span>
          <div>
            <strong>Créer une sauvegarde</strong>
            <small>
              Le serveur devra s’arrêter proprement avant la copie du monde et
              de ses réglages.
            </small>
          </div>
          <b>À construire</b>
        </div>
        <div className="backup-option">
          <span>02</span>
          <div>
            <strong>Déplacer vers un autre appareil</strong>
            <small>
              Mac, PC ou Raspberry Pi : contrôler le disque choisi et son espace
              libre avant le transfert.
            </small>
          </div>
          <b>À construire</b>
        </div>
        <div className="backup-option">
          <span>03</span>
          <div>
            <strong>Restaurer après réinstallation</strong>
            <small>
              Préparer la nouvelle copie à côté de l’ancien serveur; demander
              une vérification avant la bascule.
            </small>
          </div>
          <b>À construire</b>
        </div>
      </div>
      <div className="source-note">
        Les réglages et données propres à certaines extensions peuvent se
        trouver hors des fichiers du monde. La sauvegarde complète devra les
        inclure et signaler les composants sous licence.
      </div>
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
