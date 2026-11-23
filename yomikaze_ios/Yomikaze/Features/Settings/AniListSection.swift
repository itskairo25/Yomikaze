import AuthenticationServices
import SwiftUI

// the anilist part of settings: connect, sync now, disconnect
struct AniListSection: View {
    @Environment(\.webAuthenticationSession) private var webAuthenticationSession
    @State private var isConnecting = false
    @State private var loginError: String?

    private let sync = AniListSync.shared

    private var status: String {
        if sync.isBusy {
            return sync.progressText ?? tr("Syncing...")
        }
        if let error = sync.errorMessage {
            return error
        }
        if let last = sync.lastSync {
            return tr("Last synced {when}", ["when": last.formatted(.relative(presentation: .named))])
        }
        return tr("Not synced yet")
    }

    var body: some View {
        Section {
            if !AniListSync.isConfigured {
                Text(tr("AniList isn't set up in this build."))
                    .foregroundStyle(.secondary)
            } else if let user = sync.user, sync.isConnected {
                LabeledContent(tr("Account"), value: user.name)
                LabeledContent(tr("Status"), value: status)
                Button {
                    Task { await sync.sync(full: true) }
                } label: {
                    Label(tr("Sync now"), systemImage: "arrow.triangle.2.circlepath")
                }
                .disabled(sync.isBusy)
                Button(role: .destructive) {
                    sync.disconnect()
                } label: {
                    Label(tr("Disconnect"), systemImage: "rectangle.portrait.and.arrow.right")
                }
            } else {
                Button {
                    connect()
                } label: {
                    Label(tr("Connect AniList"), systemImage: "link")
                }
                .disabled(isConnecting)
                if let loginError {
                    Text(loginError)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
        } header: {
            Text("AniList")
        } footer: {
            Text(tr("Syncs your manga list, favourites and reading progress with AniList. Nothing is ever deleted from your AniList."))
        }
    }

    private func connect() {
        isConnecting = true
        loginError = nil
        Task {
            defer { isConnecting = false }
            do {
                let callback = try await webAuthenticationSession.authenticate(
                    using: AniListSync.loginURL,
                    callbackURLScheme: AniListSync.callbackScheme,
                    preferredBrowserSession: .shared
                )
                try await sync.finishLogin(callback: callback)
                await sync.sync()
            } catch let error as ASWebAuthenticationSessionError where error.code == .canceledLogin {
                return
            } catch {
                loginError = tr("AniList didn't accept the login.")
            }
        }
    }
}
