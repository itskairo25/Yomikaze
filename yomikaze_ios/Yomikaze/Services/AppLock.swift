import LocalAuthentication
import SwiftUI
import UIKit

enum AppLock {
    // face id first, the passcode when that fails or isn't set up
    static func authenticate(reason: String) async -> Bool {
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            // no passcode on the device, nothing to lock with
            return true
        }
        do {
            return try await context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason)
        } catch {
            return false
        }
    }
}

// the lock screen lives in its own window so it also covers the reader and open sheets
@MainActor
final class AppLockController {
    static let shared = AppLockController()

    private var window: UIWindow?
    private var isAuthenticating = false

    var isEnabled: Bool {
        UserDefaults.standard.bool(forKey: SettingsKey.appLock)
    }

    var isLocked: Bool {
        window != nil
    }

    func lock() {
        guard isEnabled, window == nil else { return }
        let scene = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first
        guard let scene else { return }

        let defaults = UserDefaults.standard
        let accent = AppTheme.accent(
            raw: defaults.string(forKey: SettingsKey.accent) ?? AccentChoice.yoru.rawValue,
            customHex: defaults.string(forKey: SettingsKey.customAccent) ?? AppTheme.defaultCustomAccent
        )
        let screen = LockScreen { [weak self] in
            self?.unlock()
        }
        .tint(accent)

        let lockWindow = UIWindow(windowScene: scene)
        lockWindow.windowLevel = .alert + 1
        lockWindow.rootViewController = UIHostingController(rootView: screen)
        lockWindow.isHidden = false
        window = lockWindow
    }

    func unlock() {
        guard window != nil, !isAuthenticating else { return }
        isAuthenticating = true
        Task {
            let success = await AppLock.authenticate(reason: tr("Unlock Yomikaze"))
            isAuthenticating = false
            if success {
                window?.isHidden = true
                window = nil
            }
        }
    }
}

struct LockScreen: View {
    let unlock: () -> Void

    var body: some View {
        ZStack {
            AmbientBackground()

            VStack(spacing: 22) {
                Text("読")
                    .font(.system(size: 80, weight: .bold))
                    .foregroundStyle(.tint)

                VStack(spacing: 6) {
                    Text(tr("Yomikaze is locked"))
                        .font(.title3.weight(.bold))
                    Text(tr("Unlock with Face ID or your passcode."))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }

                Button(action: unlock) {
                    Label(tr("Unlock"), systemImage: "faceid")
                        .font(.headline)
                        .padding(.horizontal, 18)
                        .padding(.vertical, 6)
                }
                .buttonStyle(.glassProminent)
            }
            .padding(32)
        }
    }
}
