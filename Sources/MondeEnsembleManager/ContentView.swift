import SwiftUI
import UniformTypeIdentifiers

private enum Palette {
    static let ink = Color(red: 0.11, green: 0.16, blue: 0.19)
    static let muted = Color(red: 0.43, green: 0.50, blue: 0.53)
    static let paper = Color(red: 0.96, green: 0.965, blue: 0.945)
    static let panel = Color.white
    static let line = Color(red: 0.87, green: 0.89, blue: 0.87)
    static let green = Color(red: 0.10, green: 0.43, blue: 0.36)
    static let paleGreen = Color(red: 0.88, green: 0.94, blue: 0.90)
    static let amber = Color(red: 0.68, green: 0.38, blue: 0.12)
}

struct ContentView: View {
    @EnvironmentObject private var model: ManagerModel
    @State private var showBackupPicker = false

    var body: some View {
        HStack(spacing: 0) {
            sidebar
            VStack(spacing: 0) {
                header
                ScrollView {
                    VStack(alignment: .leading, spacing: 24) {
                        switch model.section {
                        case .overview: overview
                        case .compatibility: compatibility
                        case .reinstall: reinstall
                        case .connection: connection
                        }
                    }
                    .padding(.horizontal, 34)
                    .padding(.top, 30)
                    .padding(.bottom, 42)
                    .frame(maxWidth: 880, alignment: .leading)
                    .frame(maxWidth: .infinity)
                }
            }
            .background(Palette.paper)
        }
        .background(Palette.paper)
        .fileImporter(isPresented: $showBackupPicker, allowedContentTypes: [.archive, .zip, .gzip, .data], allowsMultipleSelection: false) { result in
            switch result {
            case .success(let urls): model.chooseBackup(urls.first)
            case .failure(let error): model.errorMessage = error.localizedDescription
            }
        }
        .alert("Impossible de continuer", isPresented: Binding(get: { model.errorMessage != nil }, set: { if !$0 { model.errorMessage = nil } })) {
            Button("Compris", role: .cancel) { model.errorMessage = nil }
        } message: { Text(model.errorMessage ?? "") }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                ZStack {
                    RoundedRectangle(cornerRadius: 11).fill(Color.white.opacity(0.12))
                    Image(systemName: "globe.americas.fill").font(.system(size: 18, weight: .semibold)).foregroundStyle(Color(red: 0.65, green: 0.86, blue: 0.75))
                }.frame(width: 38, height: 38)
                VStack(alignment: .leading, spacing: 2) {
                    Text("MONDE").font(.system(size: 14, weight: .black, design: .rounded)).tracking(1.1)
                    Text("ENSEMBLE").font(.system(size: 10, weight: .bold, design: .rounded)).tracking(2.5).foregroundStyle(.white.opacity(0.6))
                }
            }.foregroundStyle(.white).padding(.horizontal, 22).padding(.top, 27).padding(.bottom, 42)

            Text("SERVEUR").font(.system(size: 10, weight: .bold)).tracking(1.6).foregroundStyle(.white.opacity(0.44)).padding(.horizontal, 22).padding(.bottom, 10)
            ForEach(AppSection.allCases) { item in
                Button { withAnimation(.easeOut(duration: 0.18)) { model.section = item } } label: {
                    HStack(spacing: 12) {
                        Image(systemName: item.symbol).font(.system(size: 15, weight: .medium)).frame(width: 18)
                        Text(item.rawValue).font(.system(size: 13, weight: model.section == item ? .semibold : .regular))
                        Spacer(minLength: 0)
                    }
                    .foregroundStyle(model.section == item ? .white : .white.opacity(0.65))
                    .padding(.horizontal, 13).padding(.vertical, 11)
                    .background(model.section == item ? Color.white.opacity(0.12) : .clear, in: RoundedRectangle(cornerRadius: 8))
                }
                .buttonStyle(.plain).padding(.horizontal, 12).padding(.bottom, 3)
            }
            Spacer()
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 7) {
                    Circle().fill(model.statusText.contains("indisponible") ? Color.orange : Color(red: 0.52, green: 0.83, blue: 0.65)).frame(width: 7, height: 7)
                    Text(model.statusText).font(.system(size: 10, weight: .medium)).foregroundStyle(.white.opacity(0.72)).lineLimit(2)
                }
                Text("Profil protégé · Paper \(model.minecraftLine)").font(.system(size: 10)).foregroundStyle(.white.opacity(0.45))
            }.padding(.horizontal, 22).padding(.bottom, 24)
        }
        .frame(width: 220)
        .background(Palette.ink)
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 3) {
                Text(model.section.rawValue).font(.system(size: 22, weight: .bold, design: .rounded)).foregroundStyle(Palette.ink)
                Text(headerSubtitle).font(.system(size: 12)).foregroundStyle(Palette.muted)
            }
            Spacer()
            Label("JAVA  +  BEDROCK", systemImage: "person.2.fill")
                .font(.system(size: 10, weight: .bold)).tracking(0.7).foregroundStyle(Palette.green)
                .padding(.horizontal, 12).padding(.vertical, 8).background(Palette.paleGreen, in: Capsule())
        }
        .padding(.horizontal, 34).padding(.vertical, 19)
        .background(.white.opacity(0.72))
        .overlay(alignment: .bottom) { Rectangle().fill(Palette.line).frame(height: 1) }
    }

    private var headerSubtitle: String {
        switch model.section {
        case .overview: "Un monde commun, avec des changements sous contrôle."
        case .compatibility: "La version du serveur attend que le monde complet soit prêt."
        case .reinstall: "Prépare une installation séparée; l’actuelle reste intacte."
        case .connection: "Aide pour le message Terracotta affiché sur l’iPad."
        }
    }

    private var overview: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Un monde.\nTous vos appareils.")
                        .font(.system(size: 33, weight: .bold, design: .rounded)).tracking(-1.2).foregroundStyle(Palette.ink)
                    Text("Le serveur reste sur la dernière version validée pour les plugins du monde. Java et Bedrock se rejoignent par Geyser.")
                        .font(.system(size: 14)).lineSpacing(4).foregroundStyle(Palette.muted).frame(maxWidth: 510, alignment: .leading)
                    Button { model.section = .reinstall } label: {
                        Label("Préparer une réinstallation", systemImage: "arrow.down.to.line")
                    }.buttonStyle(PrimaryButton())
                }
                Spacer(minLength: 10)
                ZStack {
                    RoundedRectangle(cornerRadius: 24).fill(Color(red: 0.91, green: 0.93, blue: 0.87))
                    Image(systemName: "mountain.2.fill").font(.system(size: 60, weight: .light)).foregroundStyle(Palette.green.opacity(0.82))
                    Image(systemName: "person.2.fill").font(.system(size: 16, weight: .bold)).foregroundStyle(Palette.ink).offset(y: 3)
                }.frame(width: 128, height: 128)
            }
            HStack(spacing: 12) {
                metric("SERVEUR", "Paper \(model.minecraftLine)", "version conservée", "square.stack.3d.up.fill")
                metric("BEDROCK", model.bedrockClient, "iPad détecté", "ipad")
                metric("ADRESSE", "\(model.bedrockAddress):\(model.bedrockPort)", "port Bedrock", "network")
            }
            VStack(alignment: .leading, spacing: 14) {
                sectionHeading("État du profil", detail: "Les composants qui rendent le jeu commun possible")
                ForEach(model.installedProfile) { item in profileRow(item) }
            }.padding(20).background(.white, in: RoundedRectangle(cornerRadius: 16)).overlay(RoundedRectangle(cornerRadius: 16).stroke(Palette.line, lineWidth: 1))
            noteStrip("Règle de sécurité", "Aucune nouvelle version majeure ne sera installée tant que les plugins et le pont Bedrock ne sont pas tous compatibles.", icon: "lock.shield.fill")
        }
    }

    private func metric(_ label: String, _ value: String, _ caption: String, _ symbol: String) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Image(systemName: symbol).font(.system(size: 15)).foregroundStyle(Palette.green)
            Text(label).font(.system(size: 9, weight: .bold)).tracking(1.3).foregroundStyle(Palette.muted)
            Text(value).font(.system(size: 15, weight: .semibold, design: .rounded)).foregroundStyle(Palette.ink).lineLimit(1).minimumScaleFactor(0.75)
            Text(caption).font(.system(size: 10)).foregroundStyle(Palette.muted)
        }.frame(maxWidth: .infinity, alignment: .leading).padding(15).background(.white, in: RoundedRectangle(cornerRadius: 13)).overlay(RoundedRectangle(cornerRadius: 13).stroke(Palette.line, lineWidth: 1))
    }

    private var compatibility: some View {
        VStack(alignment: .leading, spacing: 20) {
            VStack(alignment: .leading, spacing: 8) {
                Text("La compatibilité commande la mise à jour.").font(.system(size: 25, weight: .bold, design: .rounded)).foregroundStyle(Palette.ink)
                Text("Le serveur ne passe pas à une nouvelle version du jeu si un composant du monde ne suit pas.").font(.system(size: 13)).foregroundStyle(Palette.muted)
            }
            VStack(spacing: 0) {
                ForEach(model.installedProfile) { item in profileRow(item).padding(.horizontal, 18).padding(.vertical, 14); if item.id != model.installedProfile.last?.id { Divider().padding(.leading, 58) } }
            }.background(.white, in: RoundedRectangle(cornerRadius: 15)).overlay(RoundedRectangle(cornerRadius: 15).stroke(Palette.line, lineWidth: 1))
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: "checkmark.seal.fill").foregroundStyle(Palette.green).font(.system(size: 17))
                VStack(alignment: .leading, spacing: 6) {
                    Text("Bedrock 1.26.62 est dans la plage supportée par Geyser récent.").font(.system(size: 13, weight: .semibold)).foregroundStyle(Palette.ink)
                    Text("La capture seule ne désigne pas la cause du refus. L’application vérifiera le build Geyser et gardera la ligne Paper 26.2 tant que les plugins ne valident pas plus récent.").font(.system(size: 12)).foregroundStyle(Palette.muted).lineSpacing(3)
                    if let currentStatus = model.compatibility { Text("Référence actuelle : \(currentStatus.source) · vérifiée le \(currentStatus.checkedAt.formatted(date: .abbreviated, time: .shortened))").font(.system(size: 10)).foregroundStyle(Palette.muted) }
                }
            }.padding(16).frame(maxWidth: .infinity, alignment: .leading).background(Palette.paleGreen, in: RoundedRectangle(cornerRadius: 13))
            VStack(alignment: .leading, spacing: 11) {
                sectionHeading("Ce qui fonctionne ensemble", detail: "Compatibilité réelle entre éditions")
                bullet("Java sur PC et Mac, Bedrock sur Windows, iPad, téléphone et tablette.")
                bullet("Geyser traduit les échanges; Floodgate gère l’accès des comptes Bedrock.")
                bullet("Les plugins serveur et datapacks restent communs au monde.")
                bullet("Les packs Java doivent avoir leur équivalent Bedrock; ils ne se convertissent pas seuls.")
                bullet("Les mods qui imposent un mod côté client ne couvrent pas consoles et mobiles.")
            }.padding(18).background(.white, in: RoundedRectangle(cornerRadius: 15)).overlay(RoundedRectangle(cornerRadius: 15).stroke(Palette.line, lineWidth: 1))
        }
    }

    private var reinstall: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 7) {
                Text("On va le faire en trois étapes.").font(.system(size: 24, weight: .bold, design: .rounded)).foregroundStyle(Palette.ink)
                Text("Je prépare une nouvelle copie à côté de l’ancien serveur. Ton monde actuel ne sera pas touché.").font(.system(size: 13)).lineSpacing(3).foregroundStyle(Palette.muted)
            }
            stepHeading("1", "Que veux-tu garder ?", "Pour retrouver tes constructions, choisis une sauvegarde.")
            Picker("Monde", selection: $model.restoreExistingWorld) {
                Text("Garder mon monde").tag(true)
                Text("Créer un monde vide").tag(false)
            }.pickerStyle(.segmented)
            if model.restoreExistingWorld {
                HStack(spacing: 12) {
                    Image(systemName: model.backupURL == nil ? "archivebox" : "checkmark.folder.fill")
                        .font(.system(size: 19)).foregroundStyle(model.backupURL == nil ? Palette.muted : Palette.green)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(model.backupURL?.lastPathComponent ?? (model.target == .remote ? "La dernière sauvegarde du Mac mini sera choisie" : "Aucune sauvegarde sélectionnée"))
                            .font(.system(size: 12, weight: .semibold)).foregroundStyle(Palette.ink).lineLimit(1)
                        Text(model.backupURL == nil ? (model.target == .remote ? "Je vérifierai d’abord que ton monde et ses réglages sont présents." : "Choisis le fichier de sauvegarde du serveur.") : "Ton monde et ses réglages seront repris.")
                            .font(.system(size: 11)).foregroundStyle(Palette.muted)
                    }
                    Spacer(minLength: 4)
                    if model.target == .local {
                        Button(model.backupURL == nil ? "Choisir…" : "Changer…") { showBackupPicker = true }.buttonStyle(QuietButton())
                    }
                }
                if model.backupURL == nil && model.target == .local {
                    Text("Tu ne trouves pas le fichier ? N’installe pas encore un monde vide : l’ancien serveur reste intact pendant qu’on retrouve la sauvegarde.")
                        .font(.system(size: 11)).foregroundStyle(Palette.amber).lineSpacing(2)
                }
                if model.target == .remote && model.backupURL == nil {
                    Text("Si cette sauvegarde ne contient pas les réglages du monde, je m’arrêterai avant toute installation.")
                        .font(.system(size: 10)).foregroundStyle(Palette.muted).lineSpacing(2)
                }
            } else {
                noteStrip("Attention", "Un monde vide ne contient pas tes anciennes constructions. Choisis cette option seulement si tu ne veux pas restaurer ta sauvegarde.", icon: "exclamationmark.triangle.fill")
            }
            stepHeading("2", "Sur quel ordinateur ?", "Le Mac mini M4 est déjà sélectionné.")
            Picker("Ordinateur", selection: $model.target) {
                Text("Mac mini M4").tag(InstallTarget.remote)
                Text("Ce Mac").tag(InstallTarget.local)
            }.pickerStyle(.segmented)
            DisclosureGroup("Réglages avancés") {
                VStack(alignment: .leading, spacing: 11) {
                    if model.target == .remote { labeledField("Adresse du Mac mini", text: $model.remoteHost, placeholder: "danielstreng@MacMiniM4DS.local") }
                    labeledField("Dossier de destination", text: $model.destination, placeholder: "~/Minecraft")
                    labeledField("Port Bedrock de l’iPad", text: $model.bedrockPort, placeholder: "19133")
                    Text("Tu peux laisser ces réglages tels quels.").font(.system(size: 10)).foregroundStyle(Palette.muted)
                }.padding(.top, 10)
            }.font(.system(size: 11, weight: .medium)).foregroundStyle(Palette.muted)
            Toggle(isOn: $model.acceptedEULA) {
                Text("J’accepte les conditions d’utilisation Minecraft.").font(.system(size: 12)).foregroundStyle(Palette.ink)
            }.toggleStyle(.checkbox)
            if !model.progress.isEmpty {
                HStack(spacing: 10) { if model.isBusy { ProgressView().controlSize(.small) }; Text(model.progress).font(.system(size: 11)).foregroundStyle(Palette.muted) }.padding(12).frame(maxWidth: .infinity, alignment: .leading).background(.white, in: RoundedRectangle(cornerRadius: 10))
            }
            if let success = model.successMessage {
                noteStrip("Installation prête", success, icon: "checkmark.circle.fill")
                Text("Le serveur actuel tourne toujours. Avant de jouer sur la nouvelle copie, il faudra arrêter l’ancien serveur pour éviter un conflit.")
                    .font(.system(size: 11)).foregroundStyle(Palette.muted).lineSpacing(2)
                if !model.plan.isEmpty {
                    DisclosureGroup("Voir les versions installées") {
                        Text(model.plan.map { "\($0.name) · \($0.version)" }.joined(separator: "\n")).font(.system(size: 10, design: .monospaced)).foregroundStyle(Palette.muted).padding(.top, 5)
                    }.font(.system(size: 10, weight: .medium)).foregroundStyle(Palette.muted)
                }
            }
            Button { Task { await model.prepareAndInstall() } } label: {
                HStack { if model.isBusy { ProgressView().controlSize(.small).tint(.white) }; Text(model.isBusy ? "Je prépare le serveur…" : "Créer la nouvelle copie"); Spacer(); Image(systemName: "arrow.right") }
            }.buttonStyle(PrimaryButton()).disabled(model.isBusy || !model.acceptedEULA || (model.restoreExistingWorld && model.target == .local && model.backupURL == nil))
            Text("L’ancien dossier reste intact. Le nouveau serveur ne démarrera pas tout seul, pour éviter un conflit avec celui qui tourne déjà.")
                .font(.system(size: 10)).foregroundStyle(Palette.muted).lineSpacing(2)
        }
    }

    private var connection: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 8) {
                Text("J’ai trouvé une cause probable.").font(.system(size: 24, weight: .bold, design: .rounded)).foregroundStyle(Palette.ink)
                Text("Ton iPad est en Bedrock 1.26.62. Le réglage de Geyser sur le Mac mini est encore sur l’ancien mode de connexion « RakNet ». Les versions Bedrock récentes attendent « NetherNet »; cela peut provoquer l’erreur Terracotta.")
                    .font(.system(size: 13)).foregroundStyle(Palette.muted).lineSpacing(4)
            }
            HStack(spacing: 12) {
                metric("APPAREIL", "iPad", "Wi‑Fi", "ipad")
                metric("BEDROCK", "1.26.62", "version vue sur l’image", "cube.transparent")
                metric("ERREUR", "Terracotta", "InitialConnection-149", "exclamationmark.triangle")
            }
            noteStrip("La version de ton iPad convient", "Geyser récent prend en charge Bedrock 1.26.62. La nouvelle installation activera NetherNet et conservera le port 19133.", icon: "checkmark.seal.fill")
            VStack(alignment: .leading, spacing: 12) {
                sectionHeading("Ce qu’il faut retenir", detail: "Tu n’as pas besoin de changer la version de Minecraft sur l’iPad.")
                diagnosticRow("1", "Sur l’iPad", "Adresse : 192.168.1.201 · Port : 19133. Ce n’est pas le port Java.")
                diagnosticRow("2", "Sur le Mac mini", "Le port 19133 doit accepter les connexions réseau TCP et UDP.")
                diagnosticRow("3", "Pour éviter tout risque", "Ces réglages seront appliqués à la nouvelle copie. Le serveur actuel n’a pas été modifié.")
            }.padding(18).background(.white, in: RoundedRectangle(cornerRadius: 15)).overlay(RoundedRectangle(cornerRadius: 15).stroke(Palette.line, lineWidth: 1))
        }
    }

    private func sectionHeading(_ title: String, detail: String) -> some View {
        VStack(alignment: .leading, spacing: 4) { Text(title).font(.system(size: 15, weight: .semibold, design: .rounded)).foregroundStyle(Palette.ink); Text(detail).font(.system(size: 11)).foregroundStyle(Palette.muted) }
    }
    private func stepHeading(_ number: String, _ title: String, _ detail: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Text(number).font(.system(size: 11, weight: .bold, design: .rounded)).foregroundStyle(.white).frame(width: 23, height: 23).background(Palette.green, in: Circle())
            VStack(alignment: .leading, spacing: 3) { Text(title).font(.system(size: 13, weight: .semibold)).foregroundStyle(Palette.ink); Text(detail).font(.system(size: 11)).foregroundStyle(Palette.muted) }
        }
    }
    private func profileRow(_ item: CompatibilityItem) -> some View {
        HStack(spacing: 12) {
            ZStack { RoundedRectangle(cornerRadius: 8).fill(Palette.paleGreen); Image(systemName: item.id == "plugins" ? "puzzlepiece.extension.fill" : "checkmark").font(.system(size: 12, weight: .bold)).foregroundStyle(Palette.green) }.frame(width: 34, height: 34)
            VStack(alignment: .leading, spacing: 3) { Text(item.name).font(.system(size: 12, weight: .semibold)).foregroundStyle(Palette.ink); Text(item.purpose).font(.system(size: 10)).foregroundStyle(Palette.muted) }
            Spacer()
            VStack(alignment: .trailing, spacing: 3) { Text(item.version).font(.system(size: 10, weight: .medium)).foregroundStyle(Palette.ink); Text(item.state).font(.system(size: 9)).foregroundStyle(Palette.muted) }
        }
    }
    private func noteStrip(_ title: String, _ body: String, icon: String) -> some View {
        HStack(alignment: .top, spacing: 11) { Image(systemName: icon).font(.system(size: 16)).foregroundStyle(Palette.green).padding(.top, 1); VStack(alignment: .leading, spacing: 4) { Text(title).font(.system(size: 12, weight: .semibold)).foregroundStyle(Palette.ink); Text(body).font(.system(size: 11)).foregroundStyle(Palette.muted).lineSpacing(2) } }.padding(14).frame(maxWidth: .infinity, alignment: .leading).background(Palette.paleGreen, in: RoundedRectangle(cornerRadius: 12))
    }
    private func fieldTitle(_ title: String) -> some View { Text(title).font(.system(size: 12, weight: .semibold)).foregroundStyle(Palette.ink) }
    private func labeledField(_ title: String, text: Binding<String>, placeholder: String) -> some View {
        VStack(alignment: .leading, spacing: 5) { Text(title).font(.system(size: 10, weight: .medium)).foregroundStyle(Palette.muted); TextField(placeholder, text: text).textFieldStyle(.roundedBorder).font(.system(size: 12, design: .monospaced)) }
    }
    private func bullet(_ text: String) -> some View { HStack(alignment: .top, spacing: 9) { Circle().fill(Palette.green).frame(width: 5, height: 5).padding(.top, 5); Text(text).font(.system(size: 11)).foregroundStyle(Palette.muted).lineSpacing(2) } }
    private func diagnosticRow(_ number: String, _ title: String, _ desc: String) -> some View { HStack(alignment: .top, spacing: 12) { Text(number).font(.system(size: 10, weight: .bold, design: .monospaced)).foregroundStyle(Palette.green).padding(.top, 2); VStack(alignment: .leading, spacing: 4) { Text(title).font(.system(size: 12, weight: .semibold)).foregroundStyle(Palette.ink); Text(desc).font(.system(size: 11)).foregroundStyle(Palette.muted).lineSpacing(2) } } }
}

private struct PrimaryButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View { configuration.label.font(.system(size: 12, weight: .semibold)).foregroundStyle(.white).padding(.horizontal, 16).padding(.vertical, 11).background(Palette.green.opacity(configuration.isPressed ? 0.78 : 1), in: RoundedRectangle(cornerRadius: 9)) }
}
private struct QuietButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View { configuration.label.font(.system(size: 11, weight: .medium)).foregroundStyle(Palette.ink).padding(.horizontal, 11).padding(.vertical, 8).background(Palette.paper.opacity(configuration.isPressed ? 0.6 : 1), in: RoundedRectangle(cornerRadius: 7)).overlay(RoundedRectangle(cornerRadius: 7).stroke(Palette.line, lineWidth: 1)) }
}
