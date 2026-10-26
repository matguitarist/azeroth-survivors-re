import AppKit
import SwiftUI

/// The launcher window: key art, the data status, and Play.
struct LauncherView: View {
    @EnvironmentObject private var data: GameData
    @EnvironmentObject private var game: GameProcess
    @EnvironmentObject private var config: GameConfig
    @Environment(\.openWindow) private var openWindow
    @AppStorage(Prefs.skipLauncherKey) private var skipLauncher = false
    @State private var autoPlayed = false

    private var version: String {
        let v = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "dev"
        return "Version \(v)"
    }

    var body: some View {
        ZStack(alignment: .bottomLeading) {
            background
            LinearGradient(colors: [.clear, .black.opacity(0.55), .black.opacity(0.92)],
                           startPoint: .top, endPoint: .bottom)
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(Paths.appName)
                        .font(.system(size: 40, weight: .heavy, design: .serif))
                        .shadow(radius: 6)
                    Text(version)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                status
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(32)
            .foregroundStyle(.white)
        }
        .frame(width: 820, height: 480)
        .environment(\.colorScheme, .dark)
        .onAppear(perform: appeared)
        .onChange(of: game.running) { _, running in
            if !running {
                config.reload()
                data.quickCheck()
            }
        }
        .onChange(of: game.stuck) { _, stuck in
            if stuck { Launcher.surface() } else if game.running { Launcher.resubmerge() }
        }
        .background {
            Color.clear.alert("The game isn't starting", isPresented: .constant(game.stuck)) {
                Button("Keep Waiting", role: .cancel) { game.keepWaiting() }
                Button("Force Quit", role: .destructive) { game.forceQuit() }
            } message: {
                Text("It has been running for \(Int(GameProcess.stuckAfter)) seconds without opening its window. This usually means the sound output device isn't answering, for example a remote-desktop audio driver. Switching the sound output in System Settings, then trying again, usually fixes it.")
            }
        }
        .alert(item: $game.crash) { crash in
            Alert(title: Text("The game stopped unexpectedly"),
                  message: Text(crash.message + (crash.logTail.isEmpty ? "" : "\n\nThe log is in ~/Library/Logs/\(Paths.appName).")),
                  primaryButton: .default(Text("Show Log")) { reveal(Paths.gameLog) },
                  secondaryButton: .cancel(Text("OK")))
        }
    }

    @ViewBuilder private var background: some View {
        if let url = Bundle.main.url(forResource: "background", withExtension: "jpg"),
           let image = NSImage(contentsOf: url) {
            Image(nsImage: image)
                .resizable()
                .aspectRatio(contentMode: .fill)
                .frame(width: 820, height: 480)
                .clipped()
        } else {
            Color(red: 0.08, green: 0.07, blue: 0.1)
        }
    }

    @ViewBuilder private var status: some View {
        switch data.state {
        case .checking:
            HStack(spacing: 10) {
                ProgressView().controlSize(.small)
                Text("Checking game files…")
            }

        case .ready:
            HStack(alignment: .center, spacing: 16) {
                Button(action: play) {
                    Text(game.running ? "Playing" : "Play")
                        .font(.title2.weight(.bold))
                        .frame(width: 180, height: 34)
                }
                .buttonStyle(.borderedProminent)
                .tint(Color(red: 0.75, green: 0.45, blue: 0.1))
                .keyboardShortcut(.defaultAction)
                .disabled(game.running)
                SettingsLink()
            }

        case .missing(let files):
            let size = files.reduce(Int64(0)) { $0 + $1.size }
            VStack(alignment: .leading, spacing: 10) {
                Text(files.count == data.manifest.count
                     ? "The game needs the World of Warcraft 1.12.1 client's data (\(bytes(size)))."
                     : "\(files.count) game file\(files.count == 1 ? " is" : "s are") missing or damaged (\(bytes(size))).")
                    .font(.headline)
                Text("It's downloaded once from archive.org into \(Paths.data.path(percentEncoded: false).replacingOccurrences(of: NSHomeDirectory(), with: "~")). No Blizzard files ship with this app.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 12) {
                    Button(files.count == data.manifest.count ? "Download" : "Download Missing Files") {
                        data.download(files)
                    }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
                    Button("Use an Existing Folder…", action: chooseFolder)
                }
            }

        case .working(let p):
            VStack(alignment: .leading, spacing: 8) {
                Text(p.title).font(.headline)
                ProgressView(value: p.fraction)
                    .frame(maxWidth: 520)
                HStack {
                    Text(p.detail)
                        .font(.callout.monospacedDigit())
                        .foregroundStyle(.secondary)
                    Spacer().frame(width: 16)
                    Button("Pause") { data.cancel() }
                        .controlSize(.small)
                }
                .frame(maxWidth: 520, alignment: .leading)
            }

        case .failed(let message):
            VStack(alignment: .leading, spacing: 10) {
                Label(message, systemImage: "exclamationmark.triangle.fill")
                    .font(.headline)
                    .fixedSize(horizontal: false, vertical: true)
                HStack {
                    Button("Try Again") { data.quickCheck() }
                        .buttonStyle(.borderedProminent)
                    Text("Finished files are kept.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private func appeared() {
        game.onExit = { Launcher.gameEnded(crashed: $0) }
        Launcher.openLauncher = { [openWindow] in openWindow(id: "launcher") }
        data.quickCheck()
        // "Start the game directly", unless ⌥ is held to reach the launcher.
        if skipLauncher, !autoPlayed, data.state == .ready, !NSEvent.modifierFlags.contains(.option) {
            autoPlayed = true
            play()
        }
    }

    private func play() {
        if game.running { game.activate(); return }
        Launcher.play(game: game)
    }

    private func chooseFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.prompt = "Use This Folder"
        panel.message = "Choose a World of Warcraft 1.12.1 Data folder (the one holding patch.MPQ)."
        guard panel.runModal() == .OK, let url = panel.url else { return }
        UserDefaults.standard.set(url.path, forKey: Prefs.dataPathKey)
        data.quickCheck()
    }
}

extension GameProcess.Crash: Equatable {
    static func == (a: Self, b: Self) -> Bool { a.id == b.id }
}
