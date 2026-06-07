import Foundation

/// Avalanche danger for a region — the Varsom/NVE 1–5 European scale.
/// Mirrors the avalanche bits of `domain.Conditions`.
public struct AvalancheInfo: Hashable, Sendable {
    public let region: String
    /// 1 (Low) … 5 (Extreme).
    public let level: Int
    public let headline: String

    public init(region: String, level: Int, headline: String) {
        self.region = region
        self.level = max(1, min(5, level))
        self.headline = headline
    }

    public var label: String {
        switch level {
        case 1: "Low"
        case 2: "Moderate"
        case 3: "Considerable"
        case 4: "High"
        default: "Extreme"
        }
    }
}
