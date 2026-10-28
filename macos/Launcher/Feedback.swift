import AppKit
import UserNotifications

/// A progress bar over the Dock icon, for a download left running in the background.
@MainActor
enum DockProgress {
    private static var bar: NSProgressIndicator?

    /// `nil` puts the plain icon back.
    static func show(_ fraction: Double?) {
        let tile = NSApp.dockTile
        guard let fraction else {
            if bar != nil {
                bar = nil
                tile.contentView = nil
                tile.display()
            }
            return
        }
        if bar == nil {
            let icon = NSImageView(frame: NSRect(origin: .zero, size: tile.size))
            icon.image = NSApp.applicationIconImage
            let b = NSProgressIndicator(frame: NSRect(x: tile.size.width * 0.1, y: tile.size.height * 0.06,
                                                      width: tile.size.width * 0.8, height: 16))
            b.style = .bar
            b.isIndeterminate = false
            b.minValue = 0
            b.maxValue = 1
            icon.addSubview(b)
            tile.contentView = icon
            bar = b
        }
        bar?.doubleValue = fraction
        tile.display()
    }
}

/// System notifications for the end of a long download, shown only when the launcher isn't in front.
@MainActor
enum Notifier {
    /// Asked when a download starts, while the player is looking at the launcher.
    static func requestPermission() {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    static func post(_ title: String, detail: String) {
        guard !NSApp.isActive else { return }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = detail
        content.sound = .default
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }
}
