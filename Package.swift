// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "MondeEnsembleManager",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "MondeEnsembleManager", targets: ["MondeEnsembleManager"])],
    targets: [.executableTarget(name: "MondeEnsembleManager")]
)
