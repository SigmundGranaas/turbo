import Foundation
import CoreModel

/// Real avalanche danger from NVE Varsom's free API (no key). Returns `nil` when
/// the point is outside a forecast region or no warning is issued. Mirrors the
/// avalanche side of Android's `HttpConditionsRepository`.
public struct VarsomAvalancheProvider: AvalancheProvider {
    private let session: URLSession

    public init(session: URLSession = .shared) { self.session = session }

    public func danger(at position: LatLng) async -> AvalancheInfo? {
        let day = Self.today()
        let path = "https://api01.nve.no/hydrology/forecast/avalanche/v6.2.1/api/"
            + "AvalancheWarningByCoordinates/Detail/\(position.lat)/\(position.lng)/1/\(day)/\(day)"
        guard let url = URL(string: path) else { return nil }
        do {
            let (data, _) = try await session.data(from: url)
            return Self.parse(data)
        } catch {
            return nil
        }
    }

    // MARK: - Parsing (network-free, testable)

    static func parse(_ data: Data) -> AvalancheInfo? {
        guard let warnings = try? JSONDecoder().decode([Warning].self, from: data) else { return nil }
        // First warning with a real danger level (> 0).
        guard let w = warnings.first(where: { (Int($0.dangerLevel ?? "0") ?? 0) > 0 }),
              let level = Int(w.dangerLevel ?? "") else { return nil }
        return AvalancheInfo(
            region: w.regionName ?? "Region",
            level: level,
            headline: w.mainText ?? ""
        )
    }

    static func today(_ now: Date = Date()) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        return formatter.string(from: now)
    }

    private struct Warning: Decodable {
        let dangerLevel: String?
        let mainText: String?
        let regionName: String?
        enum CodingKeys: String, CodingKey {
            case dangerLevel = "DangerLevel"
            case mainText = "MainText"
            case regionName = "RegionName"
        }
    }
}
