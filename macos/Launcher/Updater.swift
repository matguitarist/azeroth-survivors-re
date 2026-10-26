import Combine
import Sparkle
import SwiftUI

/// Sparkle, behind what the menus and Settings need. The feed (`SUFeedURL`) and the EdDSA key
/// that every update must be signed with (`SUPublicEDKey`) are in Info.plist.
@MainActor
final class Updater: ObservableObject {
    private let controller: SPUStandardUpdaterController
    @Published private(set) var canCheck = false
    private var watch: AnyCancellable?

    /// An unsigned or ad-hoc build has no feed to trust; Sparkle stays off rather than erroring.
    let enabled: Bool

    init() {
        let info = Bundle.main.infoDictionary ?? [:]
        enabled = !((info["SUFeedURL"] as? String) ?? "").isEmpty && !((info["SUPublicEDKey"] as? String) ?? "").isEmpty
        controller = SPUStandardUpdaterController(startingUpdater: enabled, updaterDelegate: nil, userDriverDelegate: nil)
        watch = controller.updater.publisher(for: \.canCheckForUpdates)
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.canCheck = $0 && (self?.enabled ?? false) }
    }

    func checkForUpdates() { controller.checkForUpdates(nil) }

    var automaticallyChecks: Bool {
        get { controller.updater.automaticallyChecksForUpdates }
        set {
            controller.updater.automaticallyChecksForUpdates = newValue
            objectWillChange.send()
        }
    }

    var automaticallyDownloads: Bool {
        get { controller.updater.automaticallyDownloadsUpdates }
        set {
            controller.updater.automaticallyDownloadsUpdates = newValue
            objectWillChange.send()
        }
    }

    var lastCheck: Date? { controller.updater.lastUpdateCheckDate }
}
