import Foundation

/// One day's sun timeline at a coordinate, from MET Norway's Sunrise 3 API.
/// Mirrors the relevant parts of Flutter `SunEvent`.
public struct SunTimes: Equatable, Sendable {
    public let sunrise: Date?
    public let sunset: Date?
    /// Sun never sets on this date (high-latitude summer).
    public let polarDay: Bool
    /// Sun never rises on this date (high-latitude winter).
    public let polarNight: Bool

    public init(sunrise: Date?, sunset: Date?, polarDay: Bool = false, polarNight: Bool = false) {
        self.sunrise = sunrise
        self.sunset = sunset
        self.polarDay = polarDay
        self.polarNight = polarNight
    }

    /// Daylight span, or nil when unknown. 24 h for polar day, 0 for polar night.
    public var daylight: TimeInterval? {
        if polarDay { return 24 * 3600 }
        if polarNight { return 0 }
        guard let r = sunrise, let s = sunset, s > r else { return nil }
        return s.timeIntervalSince(r)
    }
}
