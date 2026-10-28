import Foundation

/// The addons the app ships (`addons/` in the repo, `Resources/AddOns` in the bundle), installed into
/// the game's own AddOns folder (`benilla-config/AddOns`) before each Classic launch, so an update
/// of the app updates them. The player's other addons there are left alone; the in-game AddOns
/// list at character select turns each on or off per character.
enum Addons {
    static let shipped = ["AzerothPlus", "AzerothAdmin"]
    static var folder: URL { Paths.gameConfig.appendingPathComponent("AddOns", isDirectory: true) }

    /// Install (or with the setting off, remove) the shipped addons.
    static func sync() {
        let fm = FileManager.default
        let install = UserDefaults.standard.object(forKey: Prefs.installAddonsKey) as? Bool ?? true
        guard let bundled = Bundle.main.url(forResource: "AddOns", withExtension: nil) else { return }
        try? fm.createDirectory(at: folder, withIntermediateDirectories: true)
        for name in shipped {
            let dest = folder.appendingPathComponent(name, isDirectory: true)
            try? fm.removeItem(at: dest)
            if install { try? fm.copyItem(at: bundled.appendingPathComponent(name), to: dest) }
        }
    }
}
