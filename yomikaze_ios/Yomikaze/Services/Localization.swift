import Foundation
import Observation

// the chosen language. views that call tr() read it, so they redraw by themselves when it changes.
@Observable
final class AppLanguage {
    static let shared = AppLanguage()

    var setting: String {
        didSet {
            UserDefaults.standard.set(setting, forKey: L10n.settingKey)
        }
    }

    private init() {
        setting = UserDefaults.standard.string(forKey: L10n.settingKey) ?? "system"
    }
}

// translations. the english text is the key, anything missing falls back to english.
// the same json files are used by the windows app.
enum L10n {
    static let settingKey = "appLanguage"

    struct Language: Identifiable {
        let code: String
        let name: String
        var id: String { code }
    }

    // "system" follows the iphone's language
    static let languages: [Language] = [
        Language(code: "system", name: "System"),
        Language(code: "en", name: "English"),
        Language(code: "hu", name: "Magyar"),
        Language(code: "es", name: "Español"),
        Language(code: "ja", name: "日本語"),
        Language(code: "zh", name: "中文"),
        Language(code: "pirate", name: "Pirate"),
    ]

    private static let lock = NSLock()
    private static var loaded: (code: String, table: [String: String])?

    static var code: String {
        let setting = AppLanguage.shared.setting
        if setting != "system" {
            return setting
        }
        let preferred = String((Locale.preferredLanguages.first ?? "en").prefix(2))
        return ["hu", "es", "ja", "zh"].contains(preferred) ? preferred : "en"
    }

    static func lookup(_ key: String) -> String {
        let code = code
        if code == "en" {
            return key
        }
        lock.lock()
        defer { lock.unlock() }
        if loaded?.code != code {
            var table: [String: String] = [:]
            if let url = Bundle.main.url(forResource: code, withExtension: "json"),
               let data = try? Data(contentsOf: url),
               let decoded = try? JSONDecoder().decode([String: String].self, from: data) {
                table = decoded
            }
            loaded = (code, table)
        }
        return loaded?.table[key] ?? key
    }
}

func tr(_ key: String) -> String {
    L10n.lookup(key)
}

// fills {name} placeholders after translating
func tr(_ key: String, _ args: [String: Any]) -> String {
    var text = L10n.lookup(key)
    for (name, value) in args {
        text = text.replacingOccurrences(of: "{\(name)}", with: "\(value)")
    }
    return text
}
