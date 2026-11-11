import AppKit
import Combine
import CryptoKit
import Foundation

@MainActor
final class ManagerModel: ObservableObject {
    @Published var section: AppSection = .overview
    @Published var target: InstallTarget = .remote
    @Published var remoteHost = "danielstreng@MacMiniM4DS.local"
    @Published var destination = "~/Minecraft"
    @Published var restoreExistingWorld = true
    @Published var backupURL: URL?
    @Published var acceptedEULA = false
    @Published var bedrockAddress = "192.168.1.201"
    @Published var bedrockPort = "19133"
    @Published var isBusy = false
    @Published var progress = ""
    @Published var errorMessage: String?
    @Published var successMessage: String?
    @Published var plan: [DownloadedArtifact] = []
    @Published var statusText = "Vérification des versions…"
    @Published var compatibility: CompatibilityStatus?
    @Published var preparedDestination: String?

    let minecraftLine = "26.2"
    let bedrockClient = "26.62"
    let installedProfile: [CompatibilityItem] = [
        .init(id: "paper", name: "Paper", version: "26.2", purpose: "Serveur Java commun", state: "Ligne verrouillée"),
        .init(id: "geyser", name: "Geyser + Floodgate", version: "stable la plus récente", purpose: "Connexion Bedrock et identification", state: "À mettre à jour au déploiement"),
        .init(id: "via", name: "ViaVersion + ViaBackwards", version: "stable la plus récente", purpose: "Compatibilité des clients Java", state: "À mettre à jour au déploiement"),
        .init(id: "plugins", name: "Plugins du monde", version: "depuis la sauvegarde", purpose: "Conserve protections, mondes et contenus", state: "Jamais remplacés automatiquement")
    ]

    func refreshCompatibility() async {
        do {
            var request = URLRequest(url: URL(string: "https://raw.githubusercontent.com/GeyserMC/Geyser/master/README.md")!)
            request.setValue("MondeEnsembleManager/1.0", forHTTPHeaderField: "User-Agent")
            let (data, _) = try await URLSession.shared.data(for: request)
            let readme = String(decoding: data, as: UTF8.self)
            let bedrock = readme.range(of: #"Bedrock\s*\|\s*([^\n]+)"#, options: .regularExpression).map { String(readme[$0]) } ?? "Bedrock (à confirmer)"
            let java = readme.range(of: #"Java\s*\|\s*([^\n]+)"#, options: .regularExpression).map { String(readme[$0]) } ?? "Java (à confirmer)"
            compatibility = .init(bedrock: bedrock, java: java, checkedAt: .now, source: "GeyserMC officiel")
            statusText = "Catalogue officiel consulté"
        } catch {
            statusText = "Catalogue indisponible — aucune montée de version ne sera proposée"
        }
    }

    func chooseBackup(_ url: URL?) { backupURL = url }

    func prepareAndInstall() async {
        guard acceptedEULA else { errorMessage = "Coche l’acceptation du CLUF pour préparer un nouveau serveur."; return }
        guard target == .local || validRemoteHost else { errorMessage = "Indique l’adresse SSH du Mac distant, par exemple danielstreng@MacMiniM4DS.local."; return }
        guard !destination.trimmingCharacters(in: .whitespaces).isEmpty else { errorMessage = "Choisis un dossier de destination."; return }
        guard validDestinationCharacters else { errorMessage = "Le dossier accepte lettres, chiffres, /, point, tiret et souligné. Choisis un chemin simple sans espace."; return }
        guard !restoreExistingWorld || backupURL != nil || target == .remote else { errorMessage = "Pour garder tes constructions, sélectionne d’abord une sauvegarde. Choisir « Nouveau monde » créera un monde vide."; return }
        guard let bedrockPortNumber = Int(bedrockPort), (1...65535).contains(bedrockPortNumber), bedrockPortNumber != 25566 else { errorMessage = "Le port Bedrock doit être un numéro entre 1 et 65535, différent du port Java 25566."; return }
        isBusy = true; errorMessage = nil; successMessage = nil; plan = []; progress = "Lecture du dernier build stable Paper 26.2…"
        defer { isBusy = false }
        do {
            if compatibility == nil { await refreshCompatibility() }
            guard compatibility?.bedrock.contains(bedrockClient) == true else {
                throw ManagerError.catalog("Geyser ne confirme pas encore Bedrock \(bedrockClient), ou le catalogue n’est pas joignable. La réinstallation est bloquée; le serveur existant reste intact.")
            }
            let artifacts = try await fetchArtifacts()
            plan = artifacts
            progress = "Préparation des fichiers, vérification des archives et conservation de la sauvegarde…"
            let result = try await stageAndInstall(artifacts)
            preparedDestination = result
            successMessage = "Installation préparée dans \(result). L’installation précédente n’a pas été modifiée."
            progress = "Prêt. Le nouveau serveur n’a pas été démarré."
        } catch {
            errorMessage = error.localizedDescription
            progress = "Arrêt sûr : aucun remplacement de l’installation en service."
        }
    }

    private var validRemoteHost: Bool {
        let pieces = remoteHost.split(separator: "@", omittingEmptySubsequences: false)
        return pieces.count == 2 && pieces.allSatisfy { !$0.isEmpty && $0.allSatisfy { $0.isLetter || $0.isNumber || "._-".contains($0) } }
    }

    private var validDestinationCharacters: Bool {
        destination.trimmingCharacters(in: .whitespacesAndNewlines).range(of: "^[A-Za-z0-9_./~-]+$", options: .regularExpression) != nil
    }

    private func fetchArtifacts() async throws -> [DownloadedArtifact] {
        var request = URLRequest(url: URL(string: "https://fill.papermc.io/v3/projects/paper/versions/\(minecraftLine)/builds")!)
        request.setValue("MondeEnsembleManager/1.0 (https://danielstreng.fr)", forHTTPHeaderField: "User-Agent")
        let (paperData, _) = try await URLSession.shared.data(for: request)
        let builds = try JSONDecoder().decode([PaperBuild].self, from: paperData).filter { $0.channel == "STABLE" }.sorted { $0.id > $1.id }
        guard let paper = builds.first, let download = paper.downloads["server:default"] else { throw ManagerError.catalog("Paper ne publie aucun build stable pour la ligne \(minecraftLine). La version supérieure reste bloquée.") }
        let geyserURL = URL(string: "https://download.geysermc.org/v2/projects/geyser/versions/latest/builds/latest/downloads/spigot")!
        let floodgateURL = URL(string: "https://download.geysermc.org/v2/projects/floodgate/versions/latest/builds/latest/downloads/spigot")!
        async let via = latestGitHubJar(owner: "ViaVersion", repo: "ViaVersion", namePrefix: "viaversion")
        async let backwards = latestGitHubJar(owner: "ViaVersion", repo: "ViaBackwards", namePrefix: "viabackwards")
        return [
            .init(name: "Paper", version: "\(minecraftLine) build \(paper.id)", url: download.url, expectedSHA256: download.checksums["sha256"]),
            .init(name: "Geyser-Spigot", version: "dernière stable", url: geyserURL),
            .init(name: "Floodgate-Spigot", version: "dernière stable", url: floodgateURL),
            try await via,
            try await backwards
        ]
    }

    private func latestGitHubJar(owner: String, repo: String, namePrefix: String) async throws -> DownloadedArtifact {
        var req = URLRequest(url: URL(string: "https://api.github.com/repos/\(owner)/\(repo)/releases?per_page=20")!)
        req.setValue("MondeEnsembleManager/1.0", forHTTPHeaderField: "User-Agent")
        let (data, _) = try await URLSession.shared.data(for: req)
        let releases = try JSONDecoder().decode([GitHubRelease].self, from: data)
        guard let release = releases.first(where: { !$0.prerelease && $0.assets.contains { $0.name.lowercased().hasPrefix(namePrefix) && $0.name.lowercased().hasSuffix(".jar") } }),
              let asset = release.assets.first(where: { $0.name.lowercased().hasPrefix(namePrefix) && $0.name.lowercased().hasSuffix(".jar") }) else {
            throw ManagerError.catalog("Aucune version stable téléchargeable de \(repo) n’a été trouvée. Installation annulée.")
        }
        return .init(name: repo, version: release.tag_name, url: asset.browser_download_url)
    }

    private func stageAndInstall(_ artifacts: [DownloadedArtifact]) async throws -> String {
        let fm = FileManager.default
        let work = fm.temporaryDirectory.appendingPathComponent("MondeEnsemble-\(UUID().uuidString)", isDirectory: true)
        let stage = work.appendingPathComponent("serveur", isDirectory: true)
        try fm.createDirectory(at: stage, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: work) }
        let remoteBackup = target == .remote && restoreExistingWorld && backupURL == nil ? try findCompleteRemoteBackup() : nil
        var ready: [DownloadedArtifact] = []
        for var item in artifacts {
            progress = "Téléchargement et contrôle de \(item.name)…"
            var request = URLRequest(url: item.url)
            request.setValue("MondeEnsembleManager/1.0 (https://danielstreng.fr)", forHTTPHeaderField: "User-Agent")
            let (url, response) = try await URLSession.shared.download(for: request)
            if let response = response as? HTTPURLResponse, !(200...299).contains(response.statusCode) { throw ManagerError.download("\(item.name): réponse HTTP \(response.statusCode).") }
            let saved = work.appendingPathComponent(item.name.replacingOccurrences(of: "/", with: "-") + ".jar")
            try fm.moveItem(at: url, to: saved)
            guard (try fm.attributesOfItem(atPath: saved.path)[.size] as? NSNumber)?.intValue ?? 0 > 100_000 else { throw ManagerError.download("Le fichier \(item.name) est incomplet.") }
            try run("/usr/bin/unzip", ["-t", saved.path], timeout: 60)
            if let expected = item.expectedSHA256 {
                let actual = SHA256.hash(data: try Data(contentsOf: saved)).map { String(format: "%02x", $0) }.joined()
                guard actual.caseInsensitiveCompare(expected) == .orderedSame else { throw ManagerError.download("Le contrôle d’intégrité de Paper a échoué. Aucun fichier ne sera installé.") }
            }
            item.localURL = saved; ready.append(item)
        }
        progress = "Préparation du Java 25 nécessaire à Paper…"
        let runtimeURL = URL(string: "https://api.adoptium.net/v3/binary/latest/25/ga/mac/aarch64/jre/hotspot/normal/eclipse")!
        let (runtimeTemp, runtimeResponse) = try await URLSession.shared.download(from: runtimeURL)
        if let response = runtimeResponse as? HTTPURLResponse, !(200...299).contains(response.statusCode) { throw ManagerError.download("Java 25 n’a pas pu être téléchargé (HTTP \(response.statusCode)).") }
        try run("/usr/bin/tar", ["-tzf", runtimeTemp.path], timeout: 30)
        if let remoteBackup {
            progress = "Je vérifie que la sauvegarde contient aussi les réglages du monde…"
            let liveConfig = try capture("/usr/bin/ssh", ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8", remoteHost, "cat /opt/minecraft-server/server/plugins/Geyser-Spigot/config.yml"])
            let config = stage.appendingPathComponent("plugins/Geyser-Spigot/config.yml")
            try fm.createDirectory(at: config.deletingLastPathComponent(), withIntermediateDirectories: true)
            try liveConfig.write(to: config, atomically: true, encoding: .utf8)
            _ = remoteBackup
        }
        if restoreExistingWorld, let backupURL {
            progress = "Vérification de la sauvegarde avant restauration…"
            let entries = try validateArchive(backupURL)
            if backupURL.pathExtension.lowercased() == "zip" { try run("/usr/bin/ditto", ["-x", "-k", backupURL.path, stage.path], timeout: 120) }
            else if entries.contains(where: { $0.hasPrefix("server/") }) { try run("/usr/bin/tar", ["-xzf", backupURL.path, "-C", stage.path, "--strip-components=1"], timeout: 300) }
            else { try run("/usr/bin/tar", ["-xzf", backupURL.path, "-C", stage.path], timeout: 300) }
        }
        let pluginDir = stage.appendingPathComponent("plugins", isDirectory: true)
        try fm.createDirectory(at: pluginDir, withIntermediateDirectories: true)
        // Replace only the server and cross-play bridge jars. Existing world data, configurations,
        // licensed plugins, and other content plugins from the selected backup remain untouched.
        for file in (try? fm.contentsOfDirectory(at: pluginDir, includingPropertiesForKeys: nil)) ?? [] {
            let name = file.lastPathComponent.lowercased()
            if ["geyser", "floodgate", "viaversion", "viabackwards"].contains(where: { name.hasPrefix($0) }) { try fm.removeItem(at: file) }
        }
        guard let paper = ready.first(where: { $0.name == "Paper" })?.localURL else { throw ManagerError.download("Paper manque au plan.") }
        let serverJar = stage.appendingPathComponent("paper.jar")
        try fm.copyItem(at: paper, to: serverJar)
        for item in ready where item.name != "Paper" {
            guard let file = item.localURL else { throw ManagerError.download("Téléchargement manquant : \(item.name).") }
            try fm.copyItem(at: file, to: pluginDir.appendingPathComponent("\(item.name)-\(item.version).jar"))
        }
        try configureGeyser(in: pluginDir)
        try fm.createDirectory(at: stage.appendingPathComponent("runtime", isDirectory: true), withIntermediateDirectories: true)
        try run("/usr/bin/tar", ["-xzf", runtimeTemp.path, "-C", stage.appendingPathComponent("runtime").path], timeout: 90)
        guard let javaExecutable = try capture("/usr/bin/find", [stage.appendingPathComponent("runtime").path, "-path", "*/Contents/Home/bin/java", "-type", "f", "-print"]).split(whereSeparator: \.isNewline).first else {
            throw ManagerError.download("L’archive Java ne contient pas de runtime macOS utilisable.")
        }
        try run("/usr/bin/test", ["-x", String(javaExecutable)], timeout: 5)
        try createLaunchFiles(in: stage)
        let finalPath = try destinationURL().appendingPathComponent("Monde-Ensemble-\(Self.stamp)").path
        if target == .local {
            guard !fm.fileExists(atPath: finalPath) else { throw ManagerError.destination("Le dossier \(finalPath) existe déjà. Il reste intact.") }
            try fm.createDirectory(at: URL(fileURLWithPath: finalPath).deletingLastPathComponent(), withIntermediateDirectories: true)
            try fm.moveItem(at: stage, to: URL(fileURLWithPath: finalPath))
        } else if let remoteBackup {
            progress = "Je crée la nouvelle copie sur le Mac mini depuis la dernière sauvegarde complète…"
            try installRemoteBackup(remoteBackup, patch: stage, work: work, destination: finalPath)
        } else {
            progress = "Envoi du nouveau dossier sur le Mac distant…"
            let archive = work.appendingPathComponent("installation.tar.gz")
            try run("/usr/bin/tar", ["-czf", archive.path, "-C", stage.path, "."], timeout: 120)
            let stagingPath = finalPath + ".preparing"
            let remoteFinal = shellQuote(finalPath)
            let remoteStage = shellQuote(stagingPath)
            let host = remoteHost
            var stagingCreated = false
            do {
                try run("/usr/bin/ssh", ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8", host, "test ! -e \(remoteFinal) && test ! -e \(remoteStage) && mkdir -p \(remoteStage)"], timeout: 30)
                stagingCreated = true
                let remoteArchive = "\(host):\(shellQuote(stagingPath + "/.install.tar.gz"))"
                try run("/usr/bin/scp", ["-q", "-o", "BatchMode=yes", archive.path, remoteArchive], timeout: 180)
                try run("/usr/bin/ssh", ["-o", "BatchMode=yes", host, "tar -xzf \(shellQuote(stagingPath + "/.install.tar.gz")) -C \(remoteStage) && rm \(shellQuote(stagingPath + "/.install.tar.gz")) && mv \(remoteStage) \(remoteFinal)"], timeout: 180)
            } catch {
                if stagingCreated { try? run("/usr/bin/ssh", ["-o", "BatchMode=yes", host, "rm -rf \(remoteStage)"], timeout: 30) }
                throw ManagerError.remote("La nouvelle copie n’a pas été terminée. L’ancien serveur n’a pas été modifié. Détail : \(error.localizedDescription)")
            }
        }
        return finalPath
    }

    private func createLaunchFiles(in stage: URL) throws {
        let eula = stage.appendingPathComponent("eula.txt")
        try "# CLUF Minecraft accepté dans l’application\neula=true\n".write(to: eula, atomically: true, encoding: .utf8)
        let script = stage.appendingPathComponent("start.command")
        try "#!/bin/bash\nset -e\ncd \"$(dirname \"$0\")\"\nJAVA_BIN=\"$(find runtime -path '*/Contents/Home/bin/java' -type f -print -quit)\"\nif [ -z \"$JAVA_BIN\" ]; then echo 'Java 25 inclus introuvable.'; exit 1; fi\nexec \"$JAVA_BIN\" -Xms2G -Xmx4G -jar paper.jar --nogui\n".write(to: script, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: script.path)
        if !FileManager.default.fileExists(atPath: stage.appendingPathComponent("server.properties").path) {
            try "server-port=25566\nmotd=Monde Ensemble\nonline-mode=true\nenforce-secure-profile=false\n".write(to: stage.appendingPathComponent("server.properties"), atomically: true, encoding: .utf8)
        }
        let readme = """
        MONDE ENSEMBLE — installation préparée
        Minecraft Java : port 25566 (TCP)
        Minecraft Bedrock : port 19133 (UDP et TCP)
        Profil Paper verrouillé : 26.2
        Lance start.command pour démarrer ce nouveau dossier.
        Le dossier précédent n’a pas été modifié.

        La configuration Geyser doit écouter sur le port Bedrock ci-dessus.
        Les packs Java ne sont pas convertis automatiquement en packs Bedrock.
        Les mods qui exigent une installation côté joueur ne sont pas compatibles avec toutes les consoles et tous les mobiles.
        """
        try readme.write(to: stage.appendingPathComponent("LIRE AVANT DE JOUER.txt"), atomically: true, encoding: .utf8)
    }

    private func configureGeyser(in pluginDir: URL) throws {
        let config = pluginDir.appendingPathComponent("Geyser-Spigot/config.yml")
        try FileManager.default.createDirectory(at: config.deletingLastPathComponent(), withIntermediateDirectories: true)
        var yaml: String
        if let savedConfig = try? String(contentsOf: config, encoding: .utf8) {
            yaml = savedConfig
        } else {
            guard let geyserJar = try? FileManager.default.contentsOfDirectory(at: pluginDir, includingPropertiesForKeys: nil).first(where: { $0.lastPathComponent.hasPrefix("Geyser-Spigot") }),
                  let embedded = try? capture("/usr/bin/unzip", ["-p", geyserJar.path, "config.yml"]),
                  embedded.contains("bedrock:") else {
                throw ManagerError.catalog("Le modèle de configuration Geyser est introuvable. Rien n’a été installé.")
            }
            yaml = embedded
        }
        yaml = setYAML(value: bedrockPort, in: "bedrock", key: "port", source: yaml)
        yaml = setYAML(value: "nethernet", in: "bedrock", key: "transport", source: yaml)
        yaml = setYAML(value: "floodgate", in: "remote", key: "auth-type", source: yaml)
        try yaml.write(to: config, atomically: true, encoding: .utf8)
    }

    private func setYAML(value: String, in section: String, key: String, source: String) -> String {
        var lines = source.components(separatedBy: .newlines)
        guard let start = lines.firstIndex(where: {
            !$0.hasPrefix(" ") && !$0.hasPrefix("\t") &&
            $0.trimmingCharacters(in: .whitespaces).hasPrefix("\(section):")
        }) else {
            if lines.last != "" { lines.append("") }
            lines.append("\(section):")
            lines.append("  \(key): \(value)")
            return lines.joined(separator: "\n")
        }
        let end = ((start + 1)..<lines.count).first(where: { index in
            let line = lines[index]
            return !line.isEmpty && !line.hasPrefix(" ") && !line.hasPrefix("\t") && !line.hasPrefix("#")
        }) ?? lines.count
        if let existing = ((start + 1)..<end).first(where: {
            lines[$0].trimmingCharacters(in: .whitespaces).hasPrefix("\(key):")
        }) {
            let indentation = String(lines[existing].prefix(while: { $0 == " " }))
            lines[existing] = "\(indentation)\(key): \(value)"
        } else {
            lines.insert("  \(key): \(value)", at: end)
        }
        return lines.joined(separator: "\n")
    }

    private func validateArchive(_ archive: URL) throws -> [String] {
        let list: String
        if archive.pathExtension.lowercased() == "zip" { list = try capture("/usr/bin/unzip", ["-Z1", archive.path]) }
        else { list = try capture("/usr/bin/tar", ["-tzf", archive.path]) }
        return try validateArchiveListing(list, requirePluginSettings: true)
    }

    private func validateArchiveListing(_ list: String, requirePluginSettings: Bool = false) throws -> [String] {
        var entries = list.split(whereSeparator: \.isNewline).map(String.init)
        for value in entries {
            if value.hasPrefix("/") || value.split(separator: "/").contains("..") { throw ManagerError.backup("Archive refusée : chemin non sûr détecté.") }
        }
        if entries.contains(where: { $0.hasPrefix("server/") }) { entries = entries.map { $0.hasPrefix("server/") ? String($0.dropFirst(7)) : $0 } }
        var required = [
            "world/level.dat",
            "plugins/CoreProtect-CE-24.1.jar",
            "plugins/multiverse-core-5.8.1.jar",
            "plugins/InstancedDungeons-2.3.0.jar",
            "plugins/MythicMobs-5.13.0.jar"
        ]
        if requirePluginSettings {
            required += [
                "plugins/CoreProtect/config.yml",
                "plugins/CoreProtect/database.db",
                "plugins/Multiverse-Core/worlds.yml",
                "plugins/MythicMobs/config.yml"
            ]
        }
        let missing = required.filter { !entries.contains($0) }
        guard missing.isEmpty else {
            throw ManagerError.backup("La dernière sauvegarde ne contient pas tous les éléments nécessaires pour garder ton monde et tes extensions. Pour éviter toute perte, je bloque l’installation et ne touche à rien. L’ancien serveur reste intact.")
        }
        return entries
    }

    private func findCompleteRemoteBackup() throws -> String {
        progress = "Je cherche automatiquement la sauvegarde la plus récente sur le Mac mini…"
        let listCommand = "find /opt/minecraft-server/backups -maxdepth 1 -type f -name 'mc_*.tar.gz' -print | sort | tail -n 1"
        let path = try capture("/usr/bin/ssh", ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8", remoteHost, listCommand]).trimmingCharacters(in: .whitespacesAndNewlines)
        guard path.hasPrefix("/opt/minecraft-server/backups/mc_"), path.hasSuffix(".tar.gz") else {
            throw ManagerError.backup("Je ne trouve pas la sauvegarde automatique du Mac mini. Je n’ai rien remplacé.")
        }
        let listing = try capture("/usr/bin/ssh", ["-o", "BatchMode=yes", remoteHost, "tar -tzf \(shellQuote(path))"])
        _ = try validateArchiveListing(listing)
        return path
    }

    private func installRemoteBackup(_ backup: String, patch: URL, work: URL, destination: String) throws {
        let stagingPath = destination + ".preparing"
        let freeText = try capture("/usr/bin/ssh", ["-o", "BatchMode=yes", remoteHost, "df -Pk / | tail -1 | awk '{print $4}'"])
        let freeKB = Int64(freeText.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0
        let archiveSizeText = try capture("/usr/bin/ssh", ["-o", "BatchMode=yes", remoteHost, "stat -f %z \(shellQuote(backup))"])
        let archiveBytes = Int64(archiveSizeText.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0
        guard archiveBytes > 0, freeKB * 1024 > archiveBytes * 2 else {
            throw ManagerError.destination("Le Mac mini n’a pas assez de place libre pour créer une copie complète. Le serveur actuel reste intact.")
        }
        let archive = work.appendingPathComponent("installation.tar.gz")
        try run("/usr/bin/tar", ["-czf", archive.path, "-C", patch.path, "."], timeout: 120)
        let quotedDestination = shellQuote(destination)
        let quotedStage = shellQuote(stagingPath)
        var stagingCreated = false
        do {
            try run("/usr/bin/ssh", ["-o", "BatchMode=yes", remoteHost, "test ! -e \(quotedDestination) && test ! -e \(quotedStage) && mkdir -p \(quotedStage)"], timeout: 30)
            stagingCreated = true
            try run("/usr/bin/ssh", ["-o", "BatchMode=yes", remoteHost, "tar -xzf \(shellQuote(backup)) --strip-components=1 -C \(quotedStage)"], timeout: 600)
            let pluginSource = "/opt/minecraft-server/server/plugins/"
            let pluginTarget = stagingPath + "/plugins/"
            let refreshPluginData = "rsync -a --no-owner --no-group --exclude='/*.jar*' --exclude='/CoreProtect/database.db*' \(shellQuote(pluginSource)) \(shellQuote(pluginTarget)) && rm -f \(shellQuote(stagingPath + "/plugins/CoreProtect/database.db"))* && /usr/bin/sqlite3 \(shellQuote(pluginSource + "CoreProtect/database.db")) \".backup '\(stagingPath)/plugins/CoreProtect/database.db'\""
            try run("/usr/bin/ssh", ["-o", "BatchMode=yes", remoteHost, refreshPluginData], timeout: 240)
            let remoteArchive = "\(remoteHost):\(shellQuote(stagingPath + "/.install.tar.gz"))"
            try run("/usr/bin/scp", ["-q", "-o", "BatchMode=yes", archive.path, remoteArchive], timeout: 180)
            let removeOldBridges = "find \(shellQuote(stagingPath + "/plugins")) -maxdepth 1 -type f \\( -iname 'Geyser*' -o -iname 'floodgate*' -o -iname 'ViaVersion*' -o -iname 'ViaBackwards*' \\) -delete"
            let applyPatch = "\(removeOldBridges) && tar -xzf \(shellQuote(stagingPath + "/.install.tar.gz")) -C \(quotedStage) && rm \(shellQuote(stagingPath + "/.install.tar.gz")) && mv \(quotedStage) \(quotedDestination)"
            try run("/usr/bin/ssh", ["-o", "BatchMode=yes", remoteHost, applyPatch], timeout: 300)
        } catch {
            if stagingCreated { try? run("/usr/bin/ssh", ["-o", "BatchMode=yes", remoteHost, "rm -rf \(quotedStage)"], timeout: 30) }
            throw ManagerError.remote("La nouvelle copie n’a pas pu être terminée. Ton serveur actuel n’a pas été modifié. Détail : \(error.localizedDescription)")
        }
    }

    private func destinationURL() throws -> URL {
        var path = destination.trimmingCharacters(in: .whitespacesAndNewlines)
        if path.hasPrefix("~/") { path = NSHomeDirectory() + path.dropFirst() }
        guard path.hasPrefix("/") else { throw ManagerError.destination("Le dossier doit être un chemin complet, par exemple /Users/danielstreng/Minecraft.") }
        return URL(fileURLWithPath: path, isDirectory: true).standardizedFileURL
    }

    private func shellQuote(_ value: String) -> String { "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'" }

    private func run(_ executable: String, _ arguments: [String], timeout: TimeInterval) throws {
        let process = Process(); process.executableURL = URL(fileURLWithPath: executable); process.arguments = arguments
        let outputURL = FileManager.default.temporaryDirectory.appendingPathComponent("MondeEnsemble-process-\(UUID().uuidString).log")
        FileManager.default.createFile(atPath: outputURL.path, contents: nil)
        let output = try FileHandle(forWritingTo: outputURL)
        process.standardOutput = output; process.standardError = output
        try process.run()
        if !process.waitForExit(timeout: timeout) { process.terminate(); try? output.close(); try? FileManager.default.removeItem(at: outputURL); throw ManagerError.process("Délai dépassé pour \(URL(fileURLWithPath: executable).lastPathComponent).") }
        try output.close()
        guard process.terminationStatus == 0 else {
            let data = (try? Data(contentsOf: outputURL)) ?? Data()
            try? FileManager.default.removeItem(at: outputURL)
            throw ManagerError.process(String(decoding: data.suffix(900), as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines))
        }
        try? FileManager.default.removeItem(at: outputURL)
    }

    private func capture(_ executable: String, _ arguments: [String]) throws -> String {
        let process = Process(); process.executableURL = URL(fileURLWithPath: executable); process.arguments = arguments
        let out = Pipe(); process.standardOutput = out; process.standardError = out
        try process.run(); let data = out.fileHandleForReading.readDataToEndOfFile(); process.waitUntilExit()
        guard process.terminationStatus == 0 else { throw ManagerError.backup(String(decoding: data.suffix(600), as: UTF8.self)) }
        return String(decoding: data, as: UTF8.self)
    }

    private static var stamp: String { ISO8601DateFormatter().string(from: .now).replacingOccurrences(of: ":", with: "-") }
}

private enum ManagerError: LocalizedError {
    case catalog(String), download(String), backup(String), destination(String), remote(String), process(String)
    var errorDescription: String? {
        switch self {
        case .catalog(let s), .download(let s), .backup(let s), .destination(let s), .remote(let s), .process(let s): s
        }
    }
}

private extension Process {
    func waitForExit(timeout: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while isRunning && Date() < deadline { Thread.sleep(forTimeInterval: 0.08) }
        return !isRunning
    }
}
