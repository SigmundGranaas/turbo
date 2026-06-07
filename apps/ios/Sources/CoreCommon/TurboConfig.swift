import Foundation

/// Runtime configuration read from the app's Info.plist. Drives whether the app
/// runs against the live API (Google sign-in + cloud sync) or stays fully local.
/// Absent/blank `TurboAPIBaseURL` → offline (the safe default).
public struct TurboConfig: Sendable {
    public let apiBaseURL: URL?

    public init(apiBaseURL: URL? = nil) {
        self.apiBaseURL = apiBaseURL
    }

    /// True when a real API base URL is configured.
    public var isOnline: Bool { apiBaseURL != nil }

    /// Parse from an Info-dictionary (`TurboAPIBaseURL` string key).
    public static func from(_ info: [String: Any]?) -> TurboConfig {
        let raw = (info?["TurboAPIBaseURL"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines)
        let url = (raw?.isEmpty == false) ? URL(string: raw!) : nil
        return TurboConfig(apiBaseURL: url)
    }

    public static func fromBundle(_ bundle: Bundle = .main) -> TurboConfig {
        from(bundle.infoDictionary)
    }
}
