import Foundation

/// Current marine conditions at a coastal coordinate, from MET Norway's
/// oceanforecast. Mirrors the current-point fields of Flutter `MarinePoint`.
/// MET only serves Nordic seas, so this is nil for inland points.
public struct MarineConditions: Equatable, Sendable {
    public let waveHeightM: Double?
    public let seaTemperatureC: Double?
    /// Sea-water current speed, metres per second.
    public let seaCurrentMs: Double?

    public init(waveHeightM: Double?, seaTemperatureC: Double?, seaCurrentMs: Double?) {
        self.waveHeightM = waveHeightM
        self.seaTemperatureC = seaTemperatureC
        self.seaCurrentMs = seaCurrentMs
    }

    /// True when at least one field carries data worth showing.
    public var hasData: Bool {
        waveHeightM != nil || seaTemperatureC != nil || seaCurrentMs != nil
    }
}
