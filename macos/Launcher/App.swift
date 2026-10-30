import AppKit
import SwiftUI

@main
struct LauncherApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var data = GameData()
    @StateObject private var game = GameProcess()
    @StateObject private var config = GameConfig()
    @StateObject private var updater = Updater()
    @ObservedObject private var server = LocalServer.shared
    @ObservedObject private var home = HomeServer.shared

    var body: some Scene {
        Window(Paths.appName, id: "launcher") {
            LauncherView()
                .environmentObject(data)
                .environmentObject(game)
                .environmentObject(config)
                .environmentObject(updater)
                .environmentObject(server)
                .environmentObject(home)
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .commands {
            LauncherCommands(data: data, game: game, config: config, updater: updater, server: server, home: home)
        }

        Window("Logs", id: "logs") {
            LogsWindow()
                .environmentObject(server)
                .environmentObject(home)
        }
        .defaultSize(width: 1100, height: 700)

        Window("Help", id: "help") {
            HelpWindow()
        }
        .defaultSize(width: 900, height: 640)

        Settings {
            SettingsView()
                .environmentObject(data)
                .environmentObject(game)
                .environmentObject(config)
                .environmentObject(updater)
                .environmentObject(server)
                .environmentObject(home)
        }
        .windowResizability(.contentMinSize)
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Closing the launcher's window quits it, except while it is hidden behind a game session.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        !Launcher.inSession
    }

    /// The local server stops with the launcher, unless Settings keeps it running.
    func applicationWillTerminate(_ notification: Notification) {
        if !UserDefaults.standard.bool(forKey: Prefs.keepServerKey) { LocalServer.shared.stopDetached() }
    }
}

/// Window and Dock handling around a game session: one Dock icon at a time, the game's.
@MainActor
enum Launcher {
    /// From Play until the game quits; the launcher's window is hidden meanwhile.
    static var inSession = false
    /// Reopens the launcher's window should SwiftUI have released it (set by `LauncherView`).
    static var openLauncher: (() -> Void)?

    /// Play, as the launcher is set: Survivors; Classic at an address; or Classic on this Mac or
    /// the home server, starting it first when it is not up yet.
    static func play(game: GameProcess, config: GameConfig) {
        guard !game.running else { game.activate(); return }
        guard GameMode.current == .classic else { start(game); return }
        Addons.sync()
        switch ClassicServer.current {
        case .remote:
            let address = (UserDefaults.standard.string(forKey: Prefs.remoteServerKey) ?? "")
                .trimmingCharacters(in: .whitespaces)
            config.set(Realm.cvar, address.isEmpty ? Realm.fallback : address, default: Realm.fallback)
            start(game)
        case .home:
            let home = HomeServer.shared
            Task { @MainActor in
                await home.refresh()
                if home.phase == .stopped { await home.start() }
                guard home.phase == .running, !game.running else { return }
                config.set(Realm.cvar, home.address, default: Realm.fallback)
                start(game)
            }
        case .local:
            let server = LocalServer.shared
            Task { @MainActor in
                await server.start()
                guard server.phase == .running, !game.running else { return }
                config.set(Realm.cvar, LocalServer.address, default: Realm.fallback)
                start(game)
            }
        }
    }

    /// Classic on this Mac or the home server plays once that server is set up and holds an account.
    static func canPlay(server: LocalServer, home: HomeServer) -> Bool {
        guard GameMode.current == .classic else { return true }
        switch ClassicServer.current {
        case .remote: return true
        case .home: return home.canPlay
        case .local: return server.isSetUp && !server.isBusy && !server.accounts.isEmpty
        }
    }

    private static func start(_ game: GameProcess) {
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
