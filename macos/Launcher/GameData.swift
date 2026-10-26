import Foundation

/// One MPQ the game needs, from data-manifest.tsv.
struct DataFile: Hashable {
    let name: String
    let size: Int64
    let sha256: String
}

/// The game data folder: whether it is complete, and fetching what is missing.
@MainActor
final class GameData: ObservableObject {
    enum State: Equatable {
        case checking
        case ready
        /// Files absent, the wrong size, or failing their hash.
        case missing([DataFile])
        case working(Progress)
        case failed(String)
    }

    struct Progress: Equatable {
        var title: String
        var detail: String
        var fraction: Double
    }

    @Published private(set) var state: State = .checking
    let manifest: [DataFile]
    private var job: Task<Void, Never>?
    /// The running verify or download; progress from any other (a finished one's last queued
    /// update) is dropped.
    private var jobID: UUID?

    init() {
        manifest = GameData.loadManifest()
    }

    var totalSize: Int64 { manifest.reduce(0) { $0 + $1.size } }

    var isBusy: Bool { jobID != nil }

    private func begin() -> UUID {
        let id = UUID()
        jobID = id
        return id
    }

    /// End the job, then show where things stand.
    private func end(_ final: State? = nil) {
        jobID = nil
        if let final { state = final } else { quickCheck() }
    }

    private func report(_ id: UUID, _ p: Progress) {
        if jobID == id { state = .working(p) }
    }

    static func loadManifest() -> [DataFile] {
        guard let url = Bundle.main.url(forResource: "data-manifest", withExtension: "tsv"),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return [] }
        return text.split(separator: "\n").compactMap { line in
            if line.hasPrefix("#") { return nil }
            let f = line.split(separator: "\t")
            guard f.count == 3, let size = Int64(f[1]) else { return nil }
            return DataFile(name: String(f[0]), size: size, sha256: String(f[2]))
        }
    }

    /// The fast check, run at every launch: each file present at its exact size. Hashes are left to
    /// a download (always verified) and to "Verify Game Files".
    func quickCheck() {
        guard !isBusy else { return }
        let dir = Paths.data
        let missing = manifest.filter { FileOps.size(dir.appendingPathComponent($0.name)) != $0.size }
        state = missing.isEmpty ? .ready : .missing(missing)
    }

    /// Hash every file; the ones that fail are dropped and reported missing.
    func verify() {
        guard !isBusy else { return }
        let files = manifest
        let dir = Paths.data
        let total = Double(totalSize)
        let id = begin()
        job = Task {
            var bad: [DataFile] = []
            var done: Int64 = 0
            do {
                for f in files {
                    let url = dir.appendingPathComponent(f.name)
                    guard FileOps.size(url) == f.size else { bad.append(f); done += f.size; continue }
                    let base = done
                    let tick = Throttle()
                    let sum = try await Task.detached {
                        try FileOps.sha256(url) { read in
                            guard tick.ready() else { return }
                            Task { @MainActor in
                                self.report(id, Progress(
                                    title: "Verifying game files…",
                                    detail: f.name,
                                    fraction: Double(base + read) / total))
                            }
                        }
                    }.value
                    if sum != f.sha256 { bad.append(f) }
                    done += f.size
                }
                end(bad.isEmpty ? .ready : .missing(bad))
            } catch {
                end()
            }
        }
    }

    func cancel() {
        job?.cancel()
    }

    /// Download, unpack and verify each missing file, one at a time. Partial downloads survive a
    /// cancel or a quit and resume next time.
    func download(_ files: [DataFile]) {
        guard !isBusy else { return }
        let dir = Paths.data
        let work = dir.appendingPathComponent(".download", isDirectory: true)
        let source = Prefs.dataURL
        let id = begin()
        state = .working(Progress(title: "Connecting…", detail: source.host ?? "", fraction: 0))

        job = Task {
            do {
                try FileManager.default.createDirectory(at: work, withIntermediateDirectories: true)
                try checkSpace(for: files, in: dir)
                let zip = try await RemoteZip.open(source)
                let total = Double(files.reduce(0) { $0 + $1.size })
                var done: Int64 = 0
                let meter = SpeedMeter()

                for (i, f) in files.enumerated() {
                    guard let entry = zip.entry(forFile: f.name) else {
                        throw RemoteZip.Failure.notAZip("\(f.name) isn't in the download")
                    }
                    let start = try await zip.dataStart(of: entry)
                    let packed = work.appendingPathComponent(f.name + ".packed")
                    let label = "\(f.name) (\(i + 1) of \(files.count))"
                    // Progress counts unpacked bytes; the packed size is within a percent or two.
                    let scale = Double(f.size) / Double(max(entry.compressedSize, 1))
                    let base = done
                    let tick = Throttle()

                    try await RangeDownloader(url: zip.url, start: start, length: entry.compressedSize, to: packed) { got in
                        guard tick.ready() else { return }
                        Task { @MainActor in
                            let rate = meter.sample(got)
                            let shown = base + Int64(Double(got) * scale)
                            var detail = "\(label) — \(bytes(shown)) of \(bytes(Int64(total)))"
                            if let rate { detail += " · \(bytes(Int64(rate)))/s" }
                            if let rate, rate > 0 {
                                detail += " · \(duration((total - Double(shown)) / rate)) left"
                            }
                            self.report(id, Progress(title: "Downloading game data…", detail: detail,
                                                     fraction: Double(shown) / total))
                        }
                    }.run()

                    meter.reset()
                    let target = dir.appendingPathComponent(f.name)
                    let unpacking = work.appendingPathComponent(f.name + ".part")
                    let sum = try await Task.detached {
                        try FileOps.inflate(packed, method: entry.method, to: unpacking) { used in
                            guard tick.ready() else { return }
                            Task { @MainActor in
                                self.report(id, Progress(
                                    title: "Unpacking…", detail: label,
                                    fraction: Double(base + Int64(Double(used) * scale)) / total))
                            }
                        }
                    }.value
                    try? FileManager.default.removeItem(at: packed)
                    guard sum == f.sha256 else {
                        try? FileManager.default.removeItem(at: unpacking)
                        throw RemoteZip.Failure.notAZip("\(f.name) failed its checksum; try again")
                    }
                    try? FileManager.default.removeItem(at: target)
                    try FileManager.default.moveItem(at: unpacking, to: target)
                    done += f.size
                }
                try? FileManager.default.removeItem(at: work)
                end()
            } catch is CancellationError {
                end()
            } catch let e as URLError where e.code == .cancelled {
                end()
            } catch {
                end(.failed(error.localizedDescription))
            }
        }
    }

    /// Refuse to start when the disk can't hold the unpacked files plus the largest packed one.
    private func checkSpace(for files: [DataFile], in dir: URL) throws {
        let need = files.reduce(0) { $0 + $1.size } + (files.map(\.size).max() ?? 0)
        let values = try dir.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
        if let free = values.volumeAvailableCapacityForImportantUsage, free < need {
            throw SpaceError(need: need, free: free)
        }
    }

    struct SpaceError: LocalizedError {
        let need: Int64, free: Int64
        var errorDescription: String? {
            "Not enough disk space: the game data needs \(bytes(need)) and \(bytes(free)) is free."
        }
    }
}

/// Lets one progress update through per tenth of a second, from any thread.
final class Throttle: @unchecked Sendable {
    private let lock = NSLock()
    private var last = Date.distantPast

    func ready() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        let now = Date()
        guard now.timeIntervalSince(last) >= 0.1 else { return false }
        last = now
        return true
    }
}

/// Download speed over the last few seconds.
@MainActor
final class SpeedMeter {
    private var samples: [(Date, Int64)] = []

    func sample(_ total: Int64) -> Double? {
        let now = Date()
        samples.append((now, total))
        samples.removeAll { now.timeIntervalSince($0.0) > 5 }
        guard let first = samples.first, now.timeIntervalSince(first.0) > 0.5 else { return nil }
        return Double(total - first.1) / now.timeIntervalSince(first.0)
    }

    func reset() { samples.removeAll() }
}

func bytes(_ n: Int64) -> String {
    ByteCountFormatter.string(fromByteCount: n, countStyle: .file)
}

func duration(_ seconds: Double) -> String {
    let f = DateComponentsFormatter()
    f.allowedUnits = seconds >= 3600 ? [.hour, .minute] : [.minute, .second]
    f.unitsStyle = .abbreviated
    f.maximumUnitCount = 2
    return f.string(from: max(seconds, 1)) ?? ""
}
