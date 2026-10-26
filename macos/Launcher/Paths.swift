import Foundation

/// Where the launcher and the game keep things. Nothing is written inside the app bundle: an
/// update replaces the bundle, and writing into a signed bundle breaks its signature.
enum Paths {
    static let appName = "Azeroth Survivors"

    /// ~/Library/Application Support/Azeroth Survivors: the game's working directory.
    static var support: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(appName, isDirectory: true)
    }

    /// The game's settings folder (`$BENILLA_HOME`): `config.toml`, the last pick, diagnostics.
    static var gameConfig: URL { support.appendingPathComponent("benilla-config", isDirectory: true) }

    static var defaultData: URL { support.appendingPathComponent("Data", isDirectory: true) }

    /// The MPQ folder in use: the player's choice in Settings, else the default.
    static var data: URL {
        let custom = Prefs.dataPath
        return custom.isEmpty ? defaultData : URL(fileURLWithPath: custom, isDirectory: true)
    }

    static var logs: URL {
        FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Logs/\(appName)", isDirectory: true)
    }

    static var gameLog: URL { logs.appendingPathComponent("game.log") }

    /// The game binary, beside the launcher in Contents/MacOS.
    static var gameBinary: URL {
        Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/warcraft-survivors")
    }
}

/// The launcher's own preferences (UserDefaults). The game's settings live in its config.toml.
enum Prefs {
    static let dataPathKey = "dataPath"
    static let dataURLKey = "dataURL"
    static let afterQuitKey = "afterQuit"
    static let skipLauncherKey = "skipLauncher"

    static let defaultDataURL = "https://archive.org/download/WoW_1.12.1-5875/1.12.1-5875_enUS.zip"

    static var dataPath: String { UserDefaults.standard.string(forKey: dataPathKey) ?? "" }

    static var dataURL: URL {
        let s = UserDefaults.standard.string(forKey: dataURLKey) ?? ""
        return URL(string: s.isEmpty ? defaultDataURL : s) ?? URL(string: defaultDataURL)!
    }
}

/// What the launcher does when the game quits.
enum AfterQuit: String, CaseIterable, Identifiable {
    case showLauncher, quitLauncher
    var id: String { rawValue }
    var label: String {
        switch self {
        case .showLauncher: "Show the launcher"
        case .quitLauncher: "Quit"
        }
    }
}
