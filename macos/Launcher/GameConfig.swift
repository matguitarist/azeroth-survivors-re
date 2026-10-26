import Foundation

/// The game's own settings file, `benilla-config/config.toml`: a `[cvars]` table of
/// `Name = "value"` strings. The game loads it at boot and writes only values off their default,
/// so the launcher does the same and edits it only while the game is not running.
@MainActor
final class GameConfig: ObservableObject {
    @Published private(set) var cvars: [String: String] = [:]

    private var url: URL { Paths.gameConfig.appendingPathComponent("config.toml") }

    init() { reload() }

    func reload() {
        cvars = (try? String(contentsOf: url, encoding: .utf8)).map(Self.parse) ?? [:]
    }

    /// A cvar's value, or `default` when the file doesn't set it. Lookups ignore case, as the game's do.
    func value(_ name: String, default def: String) -> String {
        cvars.first { $0.key.caseInsensitiveCompare(name) == .orderedSame }?.value ?? def
    }

    /// Set a cvar; the default removes the line, as the game's own save does.
    func set(_ name: String, _ value: String, default def: String) {
        for key in cvars.keys where key.caseInsensitiveCompare(name) == .orderedSame {
            cvars.removeValue(forKey: key)
        }
        if value != def { cvars[name] = value }
        save()
    }

    /// Back to every default: the file goes; the game recreates it.
    func reset() {
        try? FileManager.default.removeItem(at: url)
        reload()
    }

    private func save() {
        var out = "[cvars]\n"
        for key in cvars.keys.sorted() {
            out += "\(key) = \"\(Self.escape(cvars[key]!))\"\n"
        }
        try? FileManager.default.createDirectory(at: Paths.gameConfig, withIntermediateDirectories: true)
        try? out.write(to: url, atomically: true, encoding: .utf8)
    }

    /// The flat subset of TOML the game writes: a `[cvars]` header and basic-string values.
    static func parse(_ text: String) -> [String: String] {
        var out: [String: String] = [:]
        var inCvars = false
        for raw in text.split(whereSeparator: \.isNewline) {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }
            if line.hasPrefix("[") {
                inCvars = line == "[cvars]"
                continue
            }
            guard inCvars, let eq = line.firstIndex(of: "=") else { continue }
            var key = line[..<eq].trimmingCharacters(in: .whitespaces)
            if key.hasPrefix("\""), key.hasSuffix("\""), key.count >= 2 { key = String(key.dropFirst().dropLast()) }
            let rest = line[line.index(after: eq)...].trimmingCharacters(in: .whitespaces)
            guard rest.hasPrefix("\"") else { continue }
            var value = ""
            var escaped = false
            for c in rest.dropFirst() {
                if escaped {
                    value.append(c == "n" ? "\n" : c == "t" ? "\t" : c)
                    escaped = false
                } else if c == "\\" {
                    escaped = true
                } else if c == "\"" {
                    break
                } else {
                    value.append(c)
                }
            }
            out[key] = value
        }
        return out
    }

    static func escape(_ s: String) -> String {
        s.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"")
    }
}
