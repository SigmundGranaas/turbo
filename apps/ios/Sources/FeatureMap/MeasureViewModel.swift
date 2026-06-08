import Foundation
import Observation
import CoreModel

/// The measuring tool — tap points on the map, get the running total distance.
/// Mirrors `feature.measuring` (Flutter/Android).
@MainActor
@Observable
public final class MeasureViewModel {
    public private(set) var points: [LatLng] = []

    public init() {}

    public var distanceMeters: Double { GeoMetrics.pathLengthMeters(points) }

    public func addPoint(_ point: LatLng) { points.append(point) }
    public func removeLast() { if !points.isEmpty { points.removeLast() } }
    public func clear() { points = [] }
}
