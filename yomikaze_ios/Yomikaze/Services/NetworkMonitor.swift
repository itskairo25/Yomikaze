import Foundation
import Network
import Observation

// the network may be used when there is a connection and offline mode is off
@MainActor
@Observable
final class NetworkMonitor {
    static let shared = NetworkMonitor()

    private(set) var hasConnection = true

    // stored in userdefaults so OfflineMode can read it from any thread
    var forcedOffline: Bool {
        didSet {
            UserDefaults.standard.set(forcedOffline, forKey: OfflineMode.key)
        }
    }

    var isOnline: Bool {
        hasConnection && !forcedOffline
    }

    private let monitor = NWPathMonitor()

    private init() {
        forcedOffline = UserDefaults.standard.bool(forKey: OfflineMode.key)
        monitor.pathUpdateHandler = { [weak self] path in
            let connected = path.status == .satisfied
            Task { @MainActor in
                self?.hasConnection = connected
            }
        }
        monitor.start(queue: DispatchQueue(label: "dev.kairo.yomikaze.network"))
    }
}
