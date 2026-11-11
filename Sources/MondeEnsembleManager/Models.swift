import Foundation

enum AppSection: String, CaseIterable, Identifiable {
    case overview = "Vue d’ensemble"
    case compatibility = "Compatibilité"
    case reinstall = "Réinstaller"
    case connection = "Erreur de l’iPad"
    var id: String { rawValue }
    var symbol: String {
        switch self {
        case .overview: "square.grid.2x2"
        case .compatibility: "arrow.triangle.2.circlepath"
        case .reinstall: "shippingbox"
        case .connection: "dot.radiowaves.left.and.right"
        }
    }
}

enum InstallTarget: String, CaseIterable, Identifiable {
    case local = "Ce Mac"
    case remote = "Mac distant / SSH"
    var id: String { rawValue }
}

struct CompatibilityItem: Identifiable {
    let id: String
    let name: String
    let version: String
    let purpose: String
    let state: String
}

struct CompatibilityStatus: Codable {
    let bedrock: String
    let java: String
    let checkedAt: Date
    let source: String
}

struct PaperBuilds: Decodable {
    let builds: [PaperBuild]
}

struct PaperBuild: Decodable {
    let id: Int
    let channel: String
    let downloads: [String: PaperDownload]
}

struct PaperDownload: Decodable {
    let name: String
    let url: URL
    let checksums: [String: String]
}

struct GitHubRelease: Decodable {
    let tag_name: String
    let prerelease: Bool
    let assets: [Asset]
    struct Asset: Decodable { let name: String; let browser_download_url: URL }
}

struct DownloadedArtifact: Identifiable {
    let id = UUID()
    let name: String
    let version: String
    let url: URL
    var localURL: URL?
    var expectedSHA256: String? = nil
}
