import AppKit
import Foundation

/// Runs the game binary with its data and settings folders, and tracks it until it quits.
@MainActor
final class GameProcess: ObservableObject {
    @Published private(set) var running = false
    /// Set when the game exits abnormally: a short message and the end of its log.
    @Published var crash: Crash?

    struct Crash: Identifiable {
        let id = UUID()
        let message: String
        let logTail: String
    }

    private var process: Process?
    /// Called on the main actor when the game quits; the flag is true for a crash.
    var onExit: ((Bool) -> Void)?

    func launch() {
        guard !running else { return }
        let fm = FileManager.default
        do {
            try fm.createDirectory(at: Paths.support, withIntermediateDirectories: true)
            try fm.createDirectory(at: Paths.gameConfig, withIntermediateDirectories: true)
            try fm.createDirectory(at: Paths.logs, withIntermediateDirectories: true)
            // Keep the previous run's log beside the new one.
            let previous = Paths.logs.appendingPathComponent("game.previous.log")
            try? fm.removeItem(at: previous)
            try? fm.moveItem(at: Paths.gameLog, to: previous)
            fm.createFile(atPath: Paths.gameLog.path, contents: nil)
            let log = try FileHandle(forWritingTo: Paths.gameLog)

            let p = Process()
            p.executableURL = Paths.gameBinary
            p.currentDirectoryURL = Paths.support
            var env = ProcessInfo.processInfo.environment
            env["WOW_DATA"] = Paths.data.path
            env["BENILLA_HOME"] = Paths.gameConfig.path
            env["NO_COLOR"] = "1" // a readable log file, not ANSI escapes
            p.environment = env
            p.standardOutput = log
            p.standardError = log
            let started = Date()
            p.terminationHandler = { proc in
                let status = proc.terminationStatus
                let reason = proc.terminationReason
                Task { @MainActor in
                    try? log.close()
                    self.finished(status: status, reason: reason, after: Date().timeIntervalSince(started))
                }
            }
            try p.run()
            process = p
            running = true
        } catch {
            crash = Crash(message: "The game could not start: \(error.localizedDescription)", logTail: "")
        }
    }

    private func finished(status: Int32, reason: Process.TerminationReason, after seconds: TimeInterval) {
        running = false
        process = nil
        if reason == .exit && status == 0 {
            onExit?(false)
            return
        }
        let how = reason == .uncaughtSignal ? "was stopped by signal \(status)" : "quit with code \(status)"
        crash = Crash(
            message: "The game \(how) after \(duration(seconds)).",
            logTail: Self.tail(of: Paths.gameLog, lines: 40))
        onExit?(true)
    }

    /// Bring the running game forward (the launcher's Play button while it runs).
    func activate() {
        guard let pid = process?.processIdentifier,
              let app = NSRunningApplication(processIdentifier: pid) else { return }
        app.activate()
    }

    static func tail(of url: URL, lines: Int) -> String {
        guard var text = try? String(contentsOf: url, encoding: .utf8) else { return "" }
        text = text.replacingOccurrences(of: "\u{1B}\\[[0-9;]*m", with: "", options: .regularExpression)
        return text.split(separator: "\n", omittingEmptySubsequences: false).suffix(lines).joined(separator: "\n")
    }
}
