import SwiftUI
import AppKit

@main
struct MondeEnsembleManagerApp: App {
    @StateObject private var model = ManagerModel()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(model)
                .frame(minWidth: 980, minHeight: 700)
                .onAppear {
                    NSApp.setActivationPolicy(.regular)
                    NSApp.activate(ignoringOtherApps: true)
                }
                .task { await model.refreshCompatibility() }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1120, height: 780)
    }
}
