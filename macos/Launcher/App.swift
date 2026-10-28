import AppKit
import SwiftUI

@main
struct LauncherApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var data = GameData()
    @StateObject private var game = GameProcess()
    @StateObject private var config = GameConfig()
    @StateObject private var updater = Updater()

    var body: some Scene {
        Window(Paths.appName, id: "launcher") {
            LauncherView()
                .environmentObject(data)
                .environmentObject(game)
                .environmentObject(config)
                .environmentObject(updater)
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .commands {
            CommandGroup(after: .appInfo) {
                Button("Check for Updates…") { updater.checkForUpdates() }
                    .disabled(!updater.canCheck)
            }
            CommandGroup(replacing: .newItem) {}
            CommandMenu("Game") {
                Button("Play") { play() }
                    .keyboardShortcut("p")
                    .disabled(data.state != .ready || game.running)
                Divider()
                Button("Verify Game Files") { data.verify() }
                    .disabled(data.isBusy || game.running)
                Button("Show Game Data in Finder") { reveal(Paths.data) }
                Button("Show Logs in Finder") { reveal(Paths.logs) }
            }
        }

        Settings {
            SettingsView()
                .environmentObject(data)
                .environmentObject(game)
                .environmentObject(config)
                .environmentObject(updater)
        }
    }

    private func play() {
        Launcher.play(game: game)
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Closing the launcher's window quits it, except while it is hidden behind a game session.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        !Launcher.inSession
    }
}

/// Window and Dock handling around a game session: one Dock icon at a time, the game's.
@MainActor
enum Launcher {
    /// From Play until the game quits; the launcher's window is hidden meanwhile.
    static var inSession = false
    /// Reopens the launcher's window should SwiftUI have released it (set by `LauncherView`).
    static var openLauncher: (() -> Void)?

    static func play(game: GameProcess) {
        game.launch()
        guard game.running else { return }
        inSession = true
        // Hide, not close or order out: SwiftUI releases a `Window` scene's window that leaves
        // the screen, and the launcher would come back with nothing to show.
        NSApp.hide(nil)
        NSApp.setActivationPolicy(.accessory)
    }

    /// Show the launcher over a game that is still starting, to ask about it.
    static func surface() {
        NSApp.setActivationPolicy(.regular)
        NSApp.unhide(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    /// Back behind the game, once it is going again.
    static func resubmerge() {
        guard inSession else { return }
        NSApp.hide(nil)
        NSApp.setActivationPolicy(.accessory)
    }

    /// The game quit: come back, or quit with it, as Settings says.
    static func gameEnded(crashed: Bool) {
        inSession = false
        let after = AfterQuit(rawValue: UserDefaults.standard.string(forKey: Prefs.afterQuitKey) ?? "") ?? .showLauncher
        if !crashed && after == .quitLauncher {
            NSApp.terminate(nil)
            return
        }
        NSApp.setActivationPolicy(.regular)
        NSApp.unhide(nil)
        NSApp.activate(ignoringOtherApps: true)
        if !NSApp.windows.contains(where: { $0.canBecomeMain && $0.isVisible }) {
            openLauncher?()
        }
    }
}

func reveal(_ url: URL) {
    try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    NSWorkspace.shared.activateFileViewerSelecting([url])
}
