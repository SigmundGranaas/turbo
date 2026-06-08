import Foundation
import CoreModel

/// Pure (network-free) encoding/decoding for the Turbo routing API: builds the
/// request body and turns one SSE frame `(event, data)` into a domain
/// ``RouteStreamEvent``. Kept separate from the HTTP client so it's unit-testable.
/// Wire coordinates are GeoJSON `[lon, lat]`; domain ``LatLng`` is `(lat, lng)`.
/// Mirrors Android's `core.data.RouteSse`.
public enum RouteSse {

    public static func encodeRequest(points: [LatLng], preset: RoutePreset, profile: String) -> Data {
        let body: [String: Any] = [
            "points": points.map { [$0.lng, $0.lat] },
            "preset": preset.key,
            "profile": profile,
        ]
        return (try? JSONSerialization.data(withJSONObject: body)) ?? Data()
    }

    /// Parse one SSE frame; returns `nil` for keep-alives / unknown events.
    public static func parse(event: String?, data: String) -> RouteStreamEvent? {
        guard let payload = data.data(using: .utf8) else { return nil }
        switch event {
        case "progress":
            guard let dto = try? JSONDecoder().decode(CoordsDTO.self, from: payload) else { return nil }
            return .progress(dto.coordinates.toLatLngs())
        case "result":
            guard let dto = try? JSONDecoder().decode(RoutePlanDTO.self, from: payload) else { return nil }
            return .result(dto.domain)
        case "error":
            let dto = try? JSONDecoder().decode(ErrorDTO.self, from: payload)
            return .failure(dto?.error ?? defaultError)
        default:
            return nil
        }
    }

    public static let defaultError = "The route could not be solved."

    // MARK: - Wire types

    private struct CoordsDTO: Decodable { let coordinates: [[Double]] }
    private struct ErrorDTO: Decodable { let error: String? }
    private struct GeometryDTO: Decodable { let coordinates: [[Double]] }
    private struct RoutePlanDTO: Decodable {
        let distanceM: Double
        let durationS: Double
        let ascentM: Double
        let onTrailPct: Double
        let surfaces: [String: Double]
        let geometry: GeometryDTO
        enum CodingKeys: String, CodingKey {
            case distanceM = "distance_m"
            case durationS = "duration_s"
            case ascentM = "ascent_m"
            case onTrailPct = "on_trail_pct"
            case surfaces, geometry
        }
        var domain: RoutePlan {
            RoutePlan(distanceM: distanceM, durationS: durationS, ascentM: ascentM,
                      onTrailPct: onTrailPct, surfaces: surfaces, geometry: geometry.coordinates.toLatLngs())
        }
    }
}

private extension Array where Element == [Double] {
    func toLatLngs() -> [LatLng] {
        compactMap { $0.count >= 2 ? LatLng(lat: $0[1], lng: $0[0]) : nil }
    }
}
