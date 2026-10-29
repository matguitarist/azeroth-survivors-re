import AppKit
import SwiftUI

/// The launcher's view of the home server: connect, set up, its state, the first account.
struct HomeServerPanel: View {
    @EnvironmentObject private var home: HomeServer
    @EnvironmentObject private var game: GameProcess

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            switch home.phase {
            case .notConfigured:
                note("Runs CMaNGOS with bots on an always-on machine at home with Docker (an Unraid box, say), managed from here over SSH. Every Mac at home plays on it.")
                HomeConnection(compact: true)

            case .checking:
                busy("Looking at the home server…")

            case .unreachable(let why):
                Label("Can't sign in to \(home.host): \(why)", systemImage: "exclamationmark.triangle.fill")
                    .fixedSize(horizontal: false, vertical: true)
                HomeConnection(compact: true)

            case .noDocker:
                note("\(home.address) has no Docker. Install it there (on Unraid: Settings › Docker), then check again.")
                Button("Check Again") { Task { await home.refresh() } }

            case .notSetUp:
                note("Sets up CMaNGOS with bots on \(home.address): about 4 GB of game files are copied there, then it builds the server and the maps its bots walk on, an hour or two on a small CPU. You can quit the launcher meanwhile.")
                Button("Set Up Home Server") { Task { await home.setUp() } }
                    .buttonStyle(.borderedProminent)

            case .settingUp(let step):
                busy(step)
                note("The setup runs on the server: you can quit the launcher and come back.")

            case .busy(let what):
                busy(what)

            case .stopped, .running:
                status
                if home.accounts.isEmpty {
                    if home.phase == .running {
                        AccountForm(title: "Create your account", action: "Create Account", home: true)
                    } else {
                        Button("Start Server") { Task { await home.start() } }
                    }
                }

            case .failed(let why):
                Label(why, systemImage: "exclamationmark.triangle.fill")
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 10) {
                    Button("Try Again") { Task { await home.refresh() } }
                    Button("Show Log") { Task { await home.showLog() } }
                }
            }
        }
        .disabled(game.running)
        .homeServerPolling()
    }

    @ViewBuilder private var status: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(home.phase == .running ? Color.green : Color.secondary)
                .frame(width: 8, height: 8)
            Text(home.phase == .running ? "Running at \(home.address)" : "Stopped · starts with Play")
            if let bots = home.bots { Text("· \(bots) bots").foregroundStyle(.secondary) }
            if !home.accounts.isEmpty {
                Text("· \(home.accounts.count == 1 ? "account \(home.accounts[0])" : "\(home.accounts.count) accounts")")
                    .foregroundStyle(.secondary)
            }
        }
        .font(.callout)
    }

    private func busy(_ text: String) -> some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(text).font(.callout)
        }
    }

    private func note(_ text: String) -> some View {
        Text(text)
            .font(.callout)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// The server's login and address and the SSH key the launcher signs in with.
struct HomeConnection: View {
    @EnvironmentObject private var home: HomeServer
    @AppStorage(Prefs.homeHostKey) private var host = ""
    @AppStorage(Prefs.homeKeyKey) private var keyPath = ""
    var compact = false
    @State private var copied = false
    @State private var error: String?
    @State private var hasKey = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                TextField("root@192.168.1.10", text: $host)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 200)
                    .onSubmit(connect)
                if !compact {
                    TextField(HomeServer.defaultKey, text: $keyPath)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 200)
                }
                Button("Connect", action: connect)
                    .disabled(host.trimmingCharacters(in: .whitespaces).isEmpty || home.isBusy)
            }
            HStack(spacing: 8) {
                if hasKey {
                    Button(copied ? "Copied" : "Copy Public Key") {
                        guard let key = home.publicKey else { return }
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(key, forType: .string)
                        copied = true
                    }
                } else {
                    Button("Create Key") {
                        Task {
                            error = await home.createKey()
                            check()
                        }
                    }
                }
                Text("The server signs the launcher in with the key \(home.keyPath). Add its public half to the login's authorized keys; on Unraid, Users › root › SSH authorized keys.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if let error { Text(error).font(.caption).foregroundStyle(.red) }
        }
        .onAppear(perform: check)
        .onChange(of: host) { _, _ in home.settingsChanged() }
        .onChange(of: keyPath) { _, _ in home.settingsChanged(); check() }
    }

    private func check() {
        hasKey = FileManager.default.fileExists(atPath: home.key.path)
        copied = false
    }

    private func connect() {
        Task { await home.refresh() }
    }
}

/// Settings › Home Server: the connection, the server's state, its bots and accounts, removal.
struct HomeServerSettings: View {
    @EnvironmentObject private var home: HomeServer
    @EnvironmentObject private var game: GameProcess
    @State private var newAccount = false
    @State private var passwordFor: String?
    @State private var confirmRemove = false
    @State private var bots = 100

    var body: some View {
        Form {
            Section("Connection") {
                HomeConnection()
                LabeledContent("Status") { Text(statusText).multilineTextAlignment(.trailing) }
                if home.isSetUp {
                    HStack {
                        if home.phase == .running {
                            Button("Stop Server") { Task { await home.stop() } }
                                .disabled(game.running)
                            Button("Restart") { Task { await home.restart() } }
                                .disabled(game.running)
                        } else {
                            Button("Start Server") { Task { await home.start() } }
                        }
                        Spacer()
                        Button("Show Log") { Task { await home.showLog() } }
                    }
                    .disabled(home.isBusy)
                } else if home.phase == .notSetUp {
                    Button("Set Up Home Server") { Task { await home.setUp() } }
                }
            }

            if home.isSetUp {
                Section("Bots") {
                    HStack {
                        Stepper("\(bots) random bots", value: $bots, in: 0...1000, step: 50)
                        Spacer()
                        Button("Apply") { Task { await home.setBots(bots) } }
                            .disabled(home.isBusy || game.running || bots == home.bots)
                    }
                    Text("They level, quest and trade across the world. Applying restarts the game server: everyone is signed out for a minute, longer while new bots are made. Each 100 bots take about 1 GB of memory.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Section("Accounts") {
                    ForEach(home.accounts, id: \.self) { name in
                        HStack {
                            Text(name)
                            Spacer()
                            Toggle("Administrator", isOn: Binding(
                                get: { home.admins.contains(name) },
                                set: { on in Task { _ = await home.setAdmin(name, on) } }))
                                .toggleStyle(.checkbox)
                            Button("Change Password…") { passwordFor = name }
                        }
                        .disabled(home.phase != .running)
                    }
                    Button("New Account…") { newAccount = true }
                        .disabled(home.phase != .running)
                    Text(home.phase == .running
                         ? "An administrator uses AzerothAdmin (/aa in the game) for levels, gold, teleports, items and bots. A change takes effect at the account's next login."
                         : "Start the server to manage accounts.")
                        .font(.caption).foregroundStyle(.secondary)
                }

                Section {
                    Button("Remove Home Server…", role: .destructive) { confirmRemove = true }
                        .disabled(home.isBusy || game.running)
                }
            }
        }
        .formStyle(.grouped)
        .fixedSize(horizontal: false, vertical: true)
        .task { await home.refresh() }
        .homeServerPolling()
        .onChange(of: home.bots) { _, now in if let now { bots = now } }
        .sheet(isPresented: $newAccount) {
            sheet { AccountForm(title: "New account", action: "Create", home: true) { newAccount = false } }
        }
        .sheet(item: Binding(get: { passwordFor.map(Named.init) }, set: { passwordFor = $0?.name })) { item in
            sheet { AccountForm(title: "New password", action: "Change", name: item.name, home: true) { passwordFor = nil } }
        }
        .confirmationDialog("Remove the home server, its accounts and every character on it?", isPresented: $confirmRemove) {
            Button("Remove", role: .destructive) { Task { await home.remove() } }
        } message: {
            Text("Its containers, databases, image and folder on \(home.address) are deleted. This cannot be undone.")
        }
    }

    private var statusText: String {
        switch home.phase {
        case .notConfigured: "Not connected"
        case .checking: "Checking…"
        case .unreachable(let why): "Can't sign in: \(why)"
        case .noDocker: "No Docker on the server"
        case .notSetUp: "Not set up"
        case .settingUp(let step): "Setting up: \(step)"
        case .busy(let what): what
        case .stopped: "Stopped"
        case .running: "Running at \(home.address)"
        case .failed(let why): why
        }
    }

    private func sheet<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .trailing, spacing: 12) {
            content()
            Button("Close") { newAccount = false; passwordFor = nil }
                .keyboardShortcut(.cancelAction)
        }
        .padding(20)
    }

    private struct Named: Identifiable {
        let name: String
        var id: String { name }
    }
}

private struct HomeServerPolling: ViewModifier {
    @EnvironmentObject private var home: HomeServer

    private var settingUp: Bool { if case .settingUp = home.phase { true } else { false } }

    func body(content: Content) -> some View {
        content.task(id: settingUp) {
            // Follow a setup running on the server while this view is open.
            while settingUp {
                try? await Task.sleep(nanoseconds: 20_000_000_000)
                await home.refresh()
            }
        }
    }
}

extension View {
    func homeServerPolling() -> some View { modifier(HomeServerPolling()) }
}
