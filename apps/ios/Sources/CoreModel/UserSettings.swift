import Foundation

/// How the app picks light vs dark colours. Mirrors `domain.ThemeMode`.
public enum ThemeMode: String, CaseIterable, Sendable, Codable {
    case system
    case light
    case dark

    public var label: String {
        switch self {
        case .system: "System"
        case .light: "Light"
        case .dark: "Dark"
        }
    }
}

/// Persisted user preferences. Mirrors `domain.UserSettings`.
public struct UserSettings: Equatable, Sendable, Codable {
    public var compassOrientation: Bool
    public var followLocation: Bool
    public var metricUnits: Bool
    public var themeMode: ThemeMode
    /// When off, the cloud sync engine is paused even while signed in.
    public var cloudSyncEnabled: Bool
    public var shareLocation: Bool
    public var avalancheAlerts: Bool

    public init(
        compassOrientation: Bool = true,
        followLocation: Bool = false,
        metricUnits: Bool = true,
        themeMode: ThemeMode = .system,
        cloudSyncEnabled: Bool = true,
        shareLocation: Bool = true,
        avalancheAlerts: Bool = true
    ) {
        self.compassOrientation = compassOrientation
        self.followLocation = followLocation
        self.metricUnits = metricUnits
        self.themeMode = themeMode
        self.cloudSyncEnabled = cloudSyncEnabled
        self.shareLocation = shareLocation
        self.avalancheAlerts = avalancheAlerts
    }
}
