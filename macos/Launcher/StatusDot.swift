import AppKit
import SwiftUI

/// A server's state at a glance: green running, orange on its way, red down, grey not set up.
enum Health {
    case up, working, down, off

    var color: Color {
        switch self {
        case .up: .green
        case .working: .orange
        case .down: .red
        case .off: .secondary
        }
    }
}

struct StatusDot: View {
    let health: Health

    var body: some View {
        Circle()
            .fill(health.color)
            .frame(width: 8, height: 8)
            .accessibilityHidden(true)
    }
}

extension HomeServer {
    var health: Health {
        switch phase {
        case .running: .up
        case .checking, .busy, .settingUp, .updating: .working
        case .stopped, .unreachable, .noDocker, .failed: .down
        case .notConfigured, .notSetUp: .off
        }
    }
}

extension LocalServer {
    var health: Health {
        switch phase {
        case .running: .up
        case .checking, .busy: .working
        case .stopped, .failed: .down
        case .noDocker, .notSetUp: .off
        }
    }
}

/// Makes the window around a view resizable: the Settings scene's window is not, by default.
struct ResizableWindow: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async { view.window?.styleMask.insert(.resizable) }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        view.window?.styleMask.insert(.resizable)
    }
}
