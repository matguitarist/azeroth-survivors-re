import AppKit
import Foundation

/// Classic on a home server: CMaNGOS with bots in Docker on an always-on machine on the network
/// (an Unraid box, say), set up and run from here over SSH.
///
/// The launcher copies the CMaNGOS recipe and the game files the map tools read into a folder
/// there (`/mnt/user/appdata/azeroth-survivors-cmangos` on Unraid, else in the login's home), with
/// an `.env` that publishes the realm at the server's address. `home-setup` then runs there on its
/// own, an hour or two on a small CPU, so the launcher can quit meanwhile; everything after goes
/// through `home-compose` and `docker exec`. The server restarts with the machine and keeps
/// running when the game quits. Nothing here opens it to the internet.
@MainActor
final class HomeServer: ObservableObject {
    enum Phase: Equatable {
        /// No host or key yet.
        case notConfigured
        case checking
        /// SSH failed: the server is off, the address is wrong, or it does not hold the key.
        case unreachable(String)
        case noDocker
        case notSetUp
        /// The setup runs on the server, at this step.
        case settingUp(String)
        /// A step the launcher waits for: copying, start, stop.
        case busy(String)
        case stopped
        case running
        case failed(String)
    }

    @Published private(set) var phase: Phase = .notConfigured
    /// Kept across launches, so Play knows of them while the server is stopped.
    @Published private(set) var accounts: [String] = UserDefaults.standard.stringArray(forKey: "homeAccounts") ?? [] {
        didSet { UserDefaults.standard.set(accounts, forKey: "homeAccounts") }
    }
    @Published private(set) var admins: Set<String> = []
    /// The random bots the server keeps (`BOTS` in its `.env`).
    @Published private(set) var bots: Int?

    static let shared = HomeServer()
    static let defaultKey = "~/.ssh/id_ed25519"
    static let log = Paths.logs.appendingPathComponent("home-server.log")
    /// The CMaNGOS level of an administrator (`SEC_ADMINISTRATOR`).
    let adminLevel = 3
    private static let project = "azeroth-survivors-cmangos"
    private static let realmDB = "classicrealmd"
    /// The game files the map tools read, of the client's Data folder.
    private static let mpqs = ["dbc.MPQ", "terrain.MPQ", "model.MPQ", "texture.MPQ", "wmo.MPQ", "patch.MPQ", "patch-2.MPQ"]

    private init() {}

    // MARK: - Settings

    /// `user@address`, as for `ssh`.
    var host: String { UserDefaults.standard.string(forKey: Prefs.homeHostKey)?.trimmingCharacters(in: .whitespaces) ?? "" }
    var keyPath: String {
        let k = UserDefaults.standard.string(forKey: Prefs.homeKeyKey)?.trimmingCharacters(in: .whitespaces) ?? ""
        return k.isEmpty ? Self.defaultKey : k
    }
    var key: URL { URL(fileURLWithPath: (keyPath as NSString).expandingTildeInPath) }
    /// The server's folder; found at the first connection when not set.
    private var folder: String { UserDefaults.standard.string(forKey: Prefs.homeDirKey) ?? "" }

    /// Another server or login: its folder is looked for again, its accounts are its own.
    func settingsChanged() {
        UserDefaults.standard.removeObject(forKey: Prefs.homeDirKey)
        accounts = []
        admins = []
        bots = nil
        setUpDone = false
        if !isBusy { phase = .notConfigured }
    }

    /// The address the game dials and the ports bind to.
    var address: String { host.split(separator: "@").last.map(String.init) ?? host }

    var isSetUp: Bool {
        switch phase {
        case .stopped, .running: true
        case .busy, .failed: setUpDone
        default: false
        }
    }
    var isBusy: Bool {
        switch phase {
        case .busy, .checking: true
        default: false
        }
    }
    var canPlay: Bool { isSetUp && !isBusy && !accounts.isEmpty }
    private var setUpDone = false

    /// What is wrong with the settings, if anything. They end up in shell commands: only the
    /// characters an address, a login and a path need.
    private var problem: String? {
        if host.isEmpty { return "Enter the server's login and address." }
        guard host.range(of: #"^([A-Za-z0-9._-]+@)?[0-9]{1,3}(\.[0-9]{1,3}){3}$"#, options: .regularExpression) != nil else {
            return "Enter the login and the server's IP address, as root@192.168.1.10."
        }
        guard key.path.range(of: #"^[A-Za-z0-9/._-]+$"#, options: .regularExpression) != nil else {
            return "The key's path can hold letters, digits, dots, dashes and slashes only."
        }
        guard FileManager.default.fileExists(atPath: key.path) else { return "There is no SSH key at \(keyPath)." }
        return nil
    }

    // MARK: - State

    /// Look again: the connection, the setup, whether the server answers, its accounts.
    func refresh() async {
        guard !isBusy else { return }
        await look()
    }

    private func look() async {
        if let problem {
            phase = host.isEmpty ? .notConfigured : .failed(problem)
            return
        }
        if case .settingUp = phase {} else { phase = .checking }
        let status: [String: String]
        do {
            try await findFolder()
            status = try await probe()
        } catch {
            phase = .unreachable(LocalServer.message(error))
            return
        }
        bots = status["bots"].flatMap(Int.init)
        setUpDone = status["state"] == "done"
        switch status["state"] {
        case "nodocker":
            phase = .noDocker
        case "running":
            phase = .settingUp(status["step"].map { $0.isEmpty ? "Starting…" : $0 } ?? "Starting…")
        case "failed":
            phase = .failed("The setup stopped at: \(status["step"] ?? "?") Show Log has the end of its output.")
        case "done":
            let up = status["realmd"] == "healthy" && status["mangosd"] == "healthy"
            phase = up ? .running : .stopped
            if up { await loadAccounts() }
        default:
            phase = .notSetUp
        }
    }

    /// One look at everything, as `key=value` lines.
    private func probe() async throws -> [String: String] {
        let script = """
            command -v docker >/dev/null || { echo state=nodocker; exit 0; }
            cd \(folder) 2>/dev/null || { echo state=none; exit 0; }
            if [ -f .setup-done ]; then echo state=done
            elif [ -f .setup-pid ] && kill -0 "$(cat .setup-pid)" 2>/dev/null; then echo state=running
            elif [ -f .setup-failed ]; then echo state=failed
            else echo state=none; fi
            echo "step=$(cat .setup-step 2>/dev/null)"
            echo "bots=$(sed -n 's/^BOTS=//p' .env 2>/dev/null)"
            for s in realmd mangosd; do
              echo "$s=$(docker inspect -f '{{.State.Health.Status}}' \(Self.project)-$s-1 2>/dev/null)"
            done
            """
        var out: [String: String] = [:]
        for line in try await ssh(script, quiet: true).split(whereSeparator: \.isNewline) {
            let kv = line.split(separator: "=", maxSplits: 1).map(String.init)
            if kv.count == 2 { out[kv[0]] = kv[1] } else if kv.count == 1 { out[kv[0]] = "" }
        }
        return out
    }

    /// The folder on the server, once: Unraid's appdata share if there is one, else the login's home.
    private func findFolder() async throws {
        guard folder.isEmpty else { return }
        let found = try await ssh(#"if [ -d /mnt/user/appdata ]; then echo /mnt/user/appdata/azeroth-survivors-cmangos; else echo "$HOME/azeroth-survivors-cmangos"; fi"#,
                                  quiet: true)
        guard found.range(of: #"^/[A-Za-z0-9/._-]+/azeroth-survivors-cmangos$"#, options: .regularExpression) != nil else {
            throw ServerError("The server's home folder has an unusual path: \(found)")
        }
        UserDefaults.standard.set(found, forKey: Prefs.homeDirKey)
    }

    // MARK: - Setup

    /// Copy the recipe and the game files, then start the setup on the server and return; it
    /// carries on there.
    func setUp() async {
        guard !isBusy, problem == nil else { return }
        do {
            try await findFolder()
            let data = Paths.data
            let files = Self.mpqs.map { data.appendingPathComponent($0) }
            if let missing = files.first(where: { !FileManager.default.fileExists(atPath: $0.path) }) {
                throw ServerError("The game files are missing \(missing.lastPathComponent). Verify them in the Game menu.")
            }
            guard let recipe = Bundle.main.url(forResource: "Server", withExtension: nil)?.appendingPathComponent("CMaNGOS"),
                  FileManager.default.fileExists(atPath: recipe.path) else {
                throw ServerError("The app is missing its server files. Reinstall it.")
            }
            phase = .busy("Copying the server's recipe…")
            try await ssh("mkdir -p \(folder)/storage/client-data/Data")
            try await rsync([recipe.path + "/"], to: folder + "/")
            let bots = self.bots ?? 100
            try await ssh("cd \(folder) && printf 'LAN_IP=%s\\nBOTS=%s\\nBOT_ACCOUNTS=%s\\n' \(address) \(bots) \(Self.accounts(for: bots)) > .env")
            phase = .busy("Copying the game files the maps are read from (about 4 GB)…")
            try await rsync(files.map(\.path), to: folder + "/storage/client-data/Data/")
            try await ssh("cd \(folder) && rm -f .setup-done .setup-failed .setup-step && (nohup ./home-setup > setup.log 2>&1 < /dev/null &)")
            phase = .settingUp("Starting…")
        } catch {
            phase = .failed(LocalServer.message(error))
        }
    }

    // MARK: - Start and stop

    func start() async {
        guard isSetUp, !isBusy, phase != .running else { return }
        await run("Starting the server…") {
            try await self.compose("up -d database realmd mangosd")
            try await self.waitHealthy(minutes: 15)
        }
    }

    func stop() async {
        guard isSetUp, !isBusy else { return }
        await run("Stopping the server…") { try await self.compose("stop") }
    }

    func restart() async {
        guard isSetUp, !isBusy else { return }
        await run("Restarting the server…") {
            try await self.compose("up -d database realmd mangosd")
            try await self.compose("restart realmd mangosd")
            try await self.waitHealthy(minutes: 15)
        }
    }

    /// Keep this many random bots: the game server restarts with them (new ones are made on the way).
    func setBots(_ count: Int) async {
        guard isSetUp, !isBusy, (0...2000).contains(count) else { return }
        await run("Restarting the server with \(count) bots…") {
            let env = "\(self.folder)/.env"
            try await self.ssh("""
                sed -i -e '/^BOTS=/d' -e '/^BOT_ACCOUNTS=/d' \(env)
                printf 'BOTS=%s\\nBOT_ACCOUNTS=%s\\n' \(count) \(Self.accounts(for: count)) >> \(env)
                """)
            try await self.compose("up -d database realmd mangosd")
            try await self.waitHealthy(minutes: 30)
        }
    }

    /// Each bot account holds nine characters, with room to spare.
    private static func accounts(for bots: Int) -> Int { max(20, bots / 9 + 10) }

    /// A step that ends on a fresh look at the server, or on why it failed.
    private func run(_ what: String, _ body: @escaping () async throws -> Void) async {
        phase = .busy(what)
        do {
            try await body()
            await look()
        } catch {
            phase = .failed(LocalServer.message(error))
        }
    }

    private func waitHealthy(minutes: Int) async throws {
        let deadline = Date().addingTimeInterval(TimeInterval(minutes * 60))
        while Date() < deadline {
            try await Task.sleep(nanoseconds: 5_000_000_000)
            let s = try await probe()
            if s["realmd"] == "healthy", s["mangosd"] == "healthy" { return }
            for name in ["realmd", "mangosd"] where s[name] == "unhealthy" || s[name] == "" {
                // Gone or stopped: the end of its log says why.
                let state = try? await ssh("docker inspect -f '{{.State.Status}}' \(Self.project)-\(name)-1", quiet: true)
                if state == "exited" || state == "dead" {
                    throw ServerError("The server's \(name) stopped while starting. Show Log has the end of its output.")
                }
            }
        }
        throw ServerError("The server did not start in \(minutes) minutes.")
    }

    // MARK: - Accounts

    func account(_ name: String, password: String, changePassword: Bool = false, admin: Bool = false) async -> String? {
        guard phase == .running else { return "The server is not running." }
        do {
            let sql = try await LocalServer.accountSQL(name, password: password, changePassword: changePassword)
            try await mariadb(stdin: sql)
        } catch {
            let why = LocalServer.message(error)
            return why.contains("Duplicate entry") ? "An account named \(name) already exists." : why
        }
        if !changePassword, admin, let why = await setAdmin(name, true) { return why }
        await loadAccounts()
        return nil
    }

    func setAdmin(_ name: String, _ on: Bool) async -> String? {
        guard let sql = LocalServer.adminSQL(name, on, vmangos: false) else { return "That account name is not allowed." }
        do {
            try await mariadb(stdin: sql)
        } catch {
            return LocalServer.message(error)
        }
        await loadAccounts()
        return nil
    }

    private func loadAccounts() async {
        guard let out = try? await mariadb(["-N", "-B", "-e", "\"\(LocalServer.accountsQuery(vmangos: false))\""], quiet: true)
        else { return }
        (accounts, admins) = LocalServer.parseAccounts(out, adminLevel: adminLevel)
    }

    @discardableResult
    private func mariadb(_ args: [String] = [], stdin: String? = nil, quiet: Bool = false) async throws -> String {
        try await ssh((["docker exec -i \(Self.project)-database-1 mariadb -umangos -pmangos"] + args + [Self.realmDB])
                        .joined(separator: " "), stdin: stdin, quiet: quiet)
    }

    // MARK: - Log and removal

    /// The end of the setup's output and of the game server's, into a file here, shown in Finder.
    func showLog() async {
        let text = (try? await ssh("""
            cd \(folder) 2>/dev/null && { echo '== setup.log'; tail -n 200 setup.log 2>/dev/null; }
            echo '== mangosd'; docker logs --tail 300 \(Self.project)-mangosd-1 2>&1
            echo '== realmd'; docker logs --tail 50 \(Self.project)-realmd-1 2>&1
            """, quiet: true)) ?? "The server could not be reached."
        try? FileManager.default.createDirectory(at: Paths.logs, withIntermediateDirectories: true)
        try? text.write(to: Self.log, atomically: true, encoding: .utf8)
        NSWorkspace.shared.activateFileViewerSelecting([Self.log])
    }

    /// The server's containers, its databases (every character on it), its image and its folder go.
    func remove() async {
        guard !isBusy, !folder.isEmpty, folder.hasSuffix("/azeroth-survivors-cmangos") else { return }
        await run("Removing the server…") {
            try await self.ssh("""
                cd \(self.folder) && { [ ! -f .setup-pid ] || kill "$(cat .setup-pid)" 2>/dev/null; ./home-compose --profile tools down -v; }
                docker rmi azeroth-survivors-cmangos:1 2>/dev/null
                rm -rf \(self.folder)
                """)
            self.accounts = []
            self.admins = []
        }
    }

    // MARK: - SSH

    /// The private key's public half, to add to the server's authorized keys.
    var publicKey: String? {
        try? String(contentsOf: key.appendingPathExtension("pub"), encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// A new key without a passphrase at the key's path: the launcher signs in unattended.
    func createKey() async -> String? {
        guard !FileManager.default.fileExists(atPath: key.path) else { return nil }
        do {
            try FileManager.default.createDirectory(at: key.deletingLastPathComponent(), withIntermediateDirectories: true)
            try await Docker.run(URL(fileURLWithPath: "/usr/bin/ssh-keygen"),
                                 ["-q", "-t", "ed25519", "-N", "", "-C", "azeroth-survivors", "-f", key.path], quiet: true)
            return nil
        } catch {
            return LocalServer.message(error)
        }
    }

    private var sshOptions: [String] {
        ["-i", key.path, "-o", "IdentitiesOnly=yes", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5",
         "-o", "StrictHostKeyChecking=accept-new", "-o", "ServerAliveInterval=15"]
    }

    @discardableResult
    private func ssh(_ script: String, stdin: String? = nil, quiet: Bool = false) async throws -> String {
        try await Docker.run(URL(fileURLWithPath: "/usr/bin/ssh"), sshOptions + [host, script], stdin: stdin, quiet: quiet)
    }

    private func compose(_ args: String) async throws {
        try await ssh("\(folder)/home-compose \(args)")
    }

    private func rsync(_ sources: [String], to dest: String) async throws {
        try await Docker.run(URL(fileURLWithPath: "/usr/bin/rsync"),
                             ["-a", "--partial", "-e", (["ssh"] + sshOptions).joined(separator: " ")] + sources + ["\(host):\(dest)"])
    }
}
