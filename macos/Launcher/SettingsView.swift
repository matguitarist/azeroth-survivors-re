import AppKit
import SwiftUI

struct SettingsView: View {
    var body: some View {
        TabView {
            GeneralSettings().tabItem { Label("General", systemImage: "gearshape") }
            GraphicsSettings().tabItem { Label("Graphics", systemImage: "display") }
            SoundSettings().tabItem { Label("Sound", systemImage: "speaker.wave.2") }
            AdvancedSettings().tabItem { Label("Advanced", systemImage: "wrench.and.screwdriver") }
        }
        .frame(width: 520)
    }
}

// MARK: - General

private struct GeneralSettings: View {
    @EnvironmentObject private var data: GameData
    @EnvironmentObject private var game: GameProcess
    @EnvironmentObject private var updater: Updater
    @AppStorage(Prefs.dataPathKey) private var dataPath = ""
    @AppStorage(Prefs.afterQuitKey) private var afterQuit = AfterQuit.showLauncher.rawValue
    @AppStorage(Prefs.skipLauncherKey) private var skipLauncher = false

    var body: some View {
        Form {
            Section("Game Data") {
                LabeledContent("Folder") {
                    Text(Paths.data.path(percentEncoded: false))
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .textSelection(.enabled)
                }
                HStack {
                    Button("Change…", action: choose)
                    if !dataPath.isEmpty {
                        Button("Use Default") {
                            dataPath = ""
                            data.quickCheck()
                        }
                    }
                    Button("Show in Finder") { reveal(Paths.data) }
                    Spacer()
                    Button("Verify Game Files") { data.verify() }
                        .disabled(data.isBusy || game.running)
                }
            }

            Section("Launching") {
                Picker("When the game quits", selection: $afterQuit) {
                    ForEach(AfterQuit.allCases) { Text($0.label).tag($0.rawValue) }
                }
                Toggle(isOn: $skipLauncher) {
                    Text("Start the game directly")
                    Text("Hold ⌥ Option while opening the app to see the launcher.")
                }
            }

            Section("Updates") {
                if updater.enabled {
                    Toggle("Check for updates automatically", isOn: Binding(
                        get: { updater.automaticallyChecks }, set: { updater.automaticallyChecks = $0 }))
                    Toggle("Download updates in the background", isOn: Binding(
                        get: { updater.automaticallyDownloads }, set: { updater.automaticallyDownloads = $0 }))
                        .disabled(!updater.automaticallyChecks)
                    HStack {
                        if let last = updater.lastCheck {
                            Text("Last checked \(last.formatted(.relative(presentation: .named)))")
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("Check Now") { updater.checkForUpdates() }
                            .disabled(!updater.canCheck)
                    }
                } else {
                    Text("This is a local build, so updates are off. Releases update themselves.")
                        .foregroundStyle(.secondary)
                }
            }
        }
        .formStyle(.grouped)
        .fixedSize(horizontal: false, vertical: true)
    }

    private func choose() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = "Use This Folder"
        panel.message = "Choose where the game data lives. An existing WoW 1.12.1 Data folder works too."
        guard panel.runModal() == .OK, let url = panel.url else { return }
        dataPath = url.path
        data.quickCheck()
    }
}

// MARK: - Game settings (config.toml)

/// A cvar as a SwiftUI binding; numbers are written the way the game reads them.
@MainActor
private struct Cvar {
    let config: GameConfig
    let name: String
    let def: String

    var string: Binding<String> {
        Binding(get: { config.value(name, default: def) },
                set: { config.set(name, $0, default: def) })
    }

    var bool: Binding<Bool> {
        Binding(get: { config.value(name, default: def) != "0" },
                set: { config.set(name, $0 ? "1" : "0", default: def) })
    }

    /// `format` must reproduce `def` at the default, or the default would be saved as a change.
    func number(format: @escaping (Double) -> String = Cvar.short) -> Binding<Double> {
        Binding(get: { Double(config.value(name, default: def)) ?? Double(def) ?? 0 },
                set: { config.set(name, format($0), default: def) })
    }

    /// Up to two decimals, no trailing zeros: 1 → "1", 0.4 → "0.4".
    nonisolated static func short(_ v: Double) -> String {
        var s = String(format: "%.2f", v)
        while s.hasSuffix("0") { s.removeLast() }
        if s.hasSuffix(".") { s.removeLast() }
        return s
    }
}

/// Greys the game's settings out while it runs: it saves its own copy at exit, over the launcher's.
private struct GameSettingsForm<Content: View>: View {
    @EnvironmentObject private var game: GameProcess
    @ViewBuilder var content: Content

    var body: some View {
        Form {
            if game.running {
                Label("Close the game to change these.", systemImage: "lock.fill")
                    .foregroundStyle(.secondary)
            }
            content.disabled(game.running)
        }
        .formStyle(.grouped)
        .fixedSize(horizontal: false, vertical: true)
    }
}

/// One-click graphics levels: each sets the same six cvars. Balanced is the game's defaults.
private enum QualityPreset: String, CaseIterable, Identifiable {
    case performance = "Performance", balanced = "Balanced", quality = "Quality"
    var id: String { rawValue }

    /// The cvars a preset owns, with each one's default: render scale, MSAA, anisotropy, view
    /// distance, ground clutter, weather.
    static let keys: [(name: String, def: String)] = [
        ("renderScale", "1"), ("gxMultisample", "1"), ("anisotropic", "1"),
        ("farclip", "350"), ("WorldDetail", "1"), ("useWeatherShaders", "1"),
    ]

    var values: [String] {
        switch self {
        case .performance: ["0.75", "1", "1", "250", "0", "0"]
        case .balanced: ["1", "1", "1", "350", "1", "1"]
        case .quality: ["1", "1", "16", "550", "2", "1"]
        }
    }

    var summary: String {
        switch self {
        case .performance: "75% render scale, shorter view distance, no clutter or weather. For older Macs or long battery play."
        case .balanced: "Native resolution and the game's standard view distance."
        case .quality: "16× texture filtering, long view distance, dense ground clutter. Anti-aliasing stays separate: at Retina resolutions it costs about as much as everything else together."
        }
    }

    /// The preset the current settings match, if any.
    @MainActor static func current(in config: GameConfig) -> QualityPreset? {
        allCases.first { p in
            zip(keys, p.values).allSatisfy { key, v in
                Double(config.value(key.name, default: key.def)) == Double(v)
            }
        }
    }

    @MainActor func apply(to config: GameConfig) {
        config.set(zip(Self.keys, values).map { ($0.name, $1, $0.def) })
    }
}

private struct GraphicsSettings: View {
    @EnvironmentObject private var config: GameConfig

    private static let sizes = ["1280x720", "1440x900", "1600x900", "1920x1080", "2560x1440"]

    var body: some View {
        let c = { (name: String, def: String) in Cvar(config: config, name: name, def: def) }
        let windowed = c("gxWindow", "0")
        let scale = c("renderScale", "1").number()
        let farclip = c("farclip", "350").number { String(Int($0.rounded())) }

        let preset = QualityPreset.current(in: config)

        GameSettingsForm {
            Section("Preset") {
                Picker("Graphics", selection: Binding(
                    get: { preset?.rawValue ?? "custom" },
                    set: { QualityPreset(rawValue: $0)?.apply(to: config) })) {
                    ForEach(QualityPreset.allCases) { Text($0.rawValue).tag($0.rawValue) }
                    if preset == nil { Text("Custom").tag("custom") }
                }
                .pickerStyle(.segmented)
                Text(preset?.summary ?? "Your own mix of the settings below.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Display") {
                Picker("Mode", selection: windowed.string) {
                    Text("Fullscreen").tag("0")
                    Text("Window").tag("1")
                }
                .pickerStyle(.segmented)
                if windowed.bool.wrappedValue {
                    Picker("Window size", selection: c("gxResolution", "1600x900").string) {
                        ForEach(Self.sizes, id: \.self) { Text($0.replacingOccurrences(of: "x", with: " × ")).tag($0) }
                    }
                }
                Toggle("Vertical sync", isOn: c("gxVSync", "1").bool)
                LabeledContent("Render scale") {
                    HStack {
                        Slider(value: scale, in: 0.5...2, step: 0.05)
                        Text("\(Int((scale.wrappedValue * 100).rounded()))%")
                            .monospacedDigit()
                            .frame(width: 44, alignment: .trailing)
                    }
                }
                Text("Below 100% renders the 3D world at a lower resolution for speed; the interface stays sharp.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Quality") {
                Picker("Anti-aliasing", selection: c("gxMultisample", "1").string) {
                    Text("Off").tag("1")
                    Text("2×").tag("2")
                    Text("4×").tag("4")
                    Text("8×").tag("8")
                }
                Picker("Texture filtering", selection: c("anisotropic", "1").string) {
                    Text("Bilinear").tag("1")
                    Text("2× anisotropic").tag("2")
                    Text("4× anisotropic").tag("4")
                    Text("8× anisotropic").tag("8")
                    Text("16× anisotropic").tag("16")
                }
                LabeledContent("View distance") {
                    HStack {
                        Slider(value: farclip, in: 177...777, step: 25)
                        Text("\(Int(farclip.wrappedValue)) yd")
                            .monospacedDigit()
                            .frame(width: 54, alignment: .trailing)
                    }
                }
                Picker("Ground clutter", selection: c("WorldDetail", "1").string) {
                    Text("Low").tag("0")
                    Text("Medium").tag("1")
                    Text("High").tag("2")
                }
                Toggle("Weather effects", isOn: c("useWeatherShaders", "1").bool)
                LabeledContent("Gamma") {
                    Slider(value: c("gamma", "1.000000").number { String(format: "%.6f", $0) }, in: 0.5...1.5)
                }
            }
        }
    }
}

private struct SoundSettings: View {
    @EnvironmentObject private var config: GameConfig

    var body: some View {
        let c = { (name: String, def: String) in Cvar(config: config, name: name, def: def) }

        GameSettingsForm {
            Section("Volume") {
                volume("Master", c("MasterVolume", "1"))
                volume("Effects", c("SoundVolume", "1"))
                volume("Music", c("MusicVolume", "0.4"))
                volume("Ambience", c("AmbienceVolume", "0.6"))
            }
            Section {
                Toggle("Sound effects", isOn: c("MasterSoundEffects", "1").bool)
                Toggle("Music", isOn: c("EnableMusic", "1").bool)
                Toggle("Ambient sounds", isOn: c("EnableAmbience", "1").bool)
                Toggle("Play sound when the game is in the background", isOn: c("Sound_EnableSoundWhenGameIsInBG", "0").bool)
            }
        }
    }

    private func volume(_ label: String, _ cvar: Cvar) -> some View {
        let value = cvar.number()
        return LabeledContent(label) {
            HStack {
                Slider(value: value, in: 0...1, step: 0.05)
                Text("\(Int((value.wrappedValue * 100).rounded()))%")
                    .monospacedDigit()
                    .frame(width: 44, alignment: .trailing)
            }
        }
    }
}

// MARK: - Advanced

private struct AdvancedSettings: View {
    @EnvironmentObject private var config: GameConfig
    @EnvironmentObject private var game: GameProcess
    @AppStorage(Prefs.dataURLKey) private var dataURL = ""
    @State private var confirmReset = false

    var body: some View {
        Form {
            Section {
                TextField("Download from", text: $dataURL, prompt: Text(Prefs.defaultDataURL))
                    .textFieldStyle(.roundedBorder)
                Text("A direct link to a zip of a WoW 1.12.1 client holding Data/*.MPQ. Its server must support resumable (Range) downloads. Files are checked against the known checksums either way.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } header: {
                Text("Data Source")
            }

            Section("Troubleshooting") {
                HStack {
                    Button("Show Logs") { reveal(Paths.gameLog) }
                    Button("Show Game Settings Folder") { reveal(Paths.gameConfig) }
                }
                Button("Reset Game Settings…") { confirmReset = true }
                    .disabled(game.running)
                    .confirmationDialog("Reset every graphics and sound setting to its default?",
                                        isPresented: $confirmReset) {
                        Button("Reset", role: .destructive) { config.reset() }
                    }
            }
        }
        .formStyle(.grouped)
        .fixedSize(horizontal: false, vertical: true)
    }
}
