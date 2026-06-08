import Foundation
import CoreModel

/// Turbo pathfinder client. ``planStream(points:preset:profile:)`` streams the
/// solver's best-path snapshots (progress) then the final result (or failure)
/// from the **public** SSE endpoint, so the UI can animate the route as it solves.
/// Mirrors Android's `core.data.RouteRepository`.
public protocol RouteRepository: Sendable {
    func planStream(points: [LatLng], preset: RoutePreset, profile: String) -> AsyncStream<RouteStreamEvent>
}

/// Real client against the public routing API (no auth — `/api/route/*` is open).
public struct HttpRouteRepository: RouteRepository {
    private let baseURL: URL
    private let session: URLSession

    public init(baseURL: URL = URL(string: "https://kart-api.sandring.no/api/route")!,
                session: URLSession = .shared) {
        self.baseURL = baseURL
        self.session = session
    }

    public func planStream(points: [LatLng], preset: RoutePreset, profile: String) -> AsyncStream<RouteStreamEvent> {
        AsyncStream { continuation in
            let task = Task {
                var request = URLRequest(url: baseURL.appendingPathComponent("plan/stream"))
                request.httpMethod = "POST"
                request.setValue("application/json", forHTTPHeaderField: "Content-Type")
                request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
                request.httpBody = RouteSse.encodeRequest(points: points, preset: preset, profile: profile)
                do {
                    let (bytes, _) = try await session.bytes(for: request)
                    var event: String?
                    for try await line in bytes.lines {
                        if line.isEmpty {
                            event = nil
                        } else if line.hasPrefix("event:") {
                            event = String(line.dropFirst(6)).trimmingCharacters(in: .whitespaces)
                        } else if line.hasPrefix("data:") {
                            let data = String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)
                            if let parsed = RouteSse.parse(event: event, data: data) {
                                continuation.yield(parsed)
                            }
                        }
                    }
                } catch {
                    continuation.yield(.failure(RouteSse.defaultError))
                }
                continuation.finish()
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}

/// In-memory route "solver" — connects the waypoints with straight legs. Used as
/// a test double and an offline stand-in (no real pathfinding).
public struct InMemoryRouteRepository: RouteRepository {
    public init() {}

    public func planStream(points: [LatLng], preset: RoutePreset, profile: String) -> AsyncStream<RouteStreamEvent> {
        AsyncStream { continuation in
            Task {
                continuation.yield(.progress(points))
                let distance = GeoMetrics.pathLengthMeters(points)
                continuation.yield(.result(RoutePlan(
                    distanceM: distance, durationS: distance / 1.3, ascentM: 0,
                    onTrailPct: 1, surfaces: [:], geometry: points
                )))
                continuation.finish()
            }
        }
    }
}
