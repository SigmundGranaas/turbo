import Foundation
import SwiftData
import CoreModel

// SwiftData persistence entities — the iOS analogue of Android's Room tables.
// Kept internal; the repositories map them to/from the public `CoreModel` structs
// so the rest of the app never sees a SwiftData type.

@Model
final class MarkerEntity {
    @Attribute(.unique) var id: String
    var name: String
    var kindKey: String
    var lat: Double
    var lng: Double
    var colorArgb: Int?
    var notes: String?

    init(id: String, name: String, kindKey: String, lat: Double, lng: Double, colorArgb: Int?, notes: String?) {
        self.id = id
        self.name = name
        self.kindKey = kindKey
        self.lat = lat
        self.lng = lng
        self.colorArgb = colorArgb
        self.notes = notes
    }

    convenience init(_ m: Marker) {
        self.init(id: m.id, name: m.name, kindKey: m.kind.key, lat: m.position.lat, lng: m.position.lng,
                  colorArgb: m.colorArgb.map(Int.init), notes: m.notes)
    }

    func apply(_ m: Marker) {
        name = m.name; kindKey = m.kind.key; lat = m.position.lat; lng = m.position.lng
        colorArgb = m.colorArgb.map(Int.init); notes = m.notes
    }

    var domain: Marker {
        Marker(
            id: id,
            name: name,
            kind: ActivityKindId.fromKey(kindKey) ?? .mountain,
            position: LatLng(lat: lat, lng: lng),
            colorArgb: colorArgb.map(Int64.init),
            notes: notes
        )
    }
}

@Model
final class PathEntity {
    @Attribute(.unique) var id: String
    var name: String
    var activityKey: String?
    var source: String
    var distanceM: Double
    var ascentM: Double?
    var recordedAtEpochMs: Int?
    var movingTimeSeconds: Int?
    /// Encoded `[LatLng]` + `[Double]?` so the geometry rides along.
    var pointsJSON: Data
    var elevationsJSON: Data?
    /// Follow-mode metadata (D1): the planned guide `[LatLng]` and the checkpoint splits,
    /// JSON-encoded; nil for plain recordings. Optional → SwiftData lightweight-migrates.
    var plannedRouteJSON: Data?
    var phaseSplitsJSON: Data?

    init(id: String, name: String, activityKey: String?, source: String, distanceM: Double,
         ascentM: Double?, recordedAtEpochMs: Int?, movingTimeSeconds: Int?, pointsJSON: Data, elevationsJSON: Data?,
         plannedRouteJSON: Data? = nil, phaseSplitsJSON: Data? = nil) {
        self.id = id
        self.name = name
        self.activityKey = activityKey
        self.source = source
        self.distanceM = distanceM
        self.ascentM = ascentM
        self.recordedAtEpochMs = recordedAtEpochMs
        self.movingTimeSeconds = movingTimeSeconds
        self.pointsJSON = pointsJSON
        self.elevationsJSON = elevationsJSON
        self.plannedRouteJSON = plannedRouteJSON
        self.phaseSplitsJSON = phaseSplitsJSON
    }

    convenience init(_ p: SavedPath) {
        self.init(
            id: p.id, name: p.name, activityKey: p.activityKind?.key,
            source: p.path.source.rawValue, distanceM: p.path.distanceM, ascentM: p.path.ascentM,
            recordedAtEpochMs: p.path.recordedAtEpochMs.map(Int.init),
            movingTimeSeconds: p.path.movingTimeSeconds,
            pointsJSON: (try? JSONEncoder().encode(p.path.points)) ?? Data(),
            elevationsJSON: p.path.elevations.flatMap { try? JSONEncoder().encode($0) },
            plannedRouteJSON: p.plannedRoute.flatMap { try? JSONEncoder().encode($0) },
            phaseSplitsJSON: p.phaseSplits.isEmpty ? nil : try? JSONEncoder().encode(p.phaseSplits.map(PhaseSplitDTO.init))
        )
    }

    func apply(_ p: SavedPath) {
        name = p.name; activityKey = p.activityKind?.key; source = p.path.source.rawValue
        distanceM = p.path.distanceM; ascentM = p.path.ascentM
        recordedAtEpochMs = p.path.recordedAtEpochMs.map(Int.init)
        movingTimeSeconds = p.path.movingTimeSeconds
        pointsJSON = (try? JSONEncoder().encode(p.path.points)) ?? Data()
        elevationsJSON = p.path.elevations.flatMap { try? JSONEncoder().encode($0) }
        plannedRouteJSON = p.plannedRoute.flatMap { try? JSONEncoder().encode($0) }
        phaseSplitsJSON = p.phaseSplits.isEmpty ? nil : try? JSONEncoder().encode(p.phaseSplits.map(PhaseSplitDTO.init))
    }

    var domain: SavedPath {
        let points = (try? JSONDecoder().decode([LatLng].self, from: pointsJSON)) ?? []
        let elevations = elevationsJSON.flatMap { try? JSONDecoder().decode([Double].self, from: $0) }
        let plannedRoute = plannedRouteJSON.flatMap { try? JSONDecoder().decode([LatLng].self, from: $0) }
        let splits = phaseSplitsJSON
            .flatMap { try? JSONDecoder().decode([PhaseSplitDTO].self, from: $0) }?
            .map(\.model) ?? []
        return SavedPath(
            id: id, name: name,
            path: GeoPath(
                points: points,
                source: GeoPathSource(rawValue: source) ?? .saved,
                elevations: elevations,
                distanceM: distanceM,
                ascentM: ascentM,
                recordedAtEpochMs: recordedAtEpochMs.map(Int64.init),
                movingTimeSeconds: movingTimeSeconds
            ),
            activityKind: activityKey.flatMap(ActivityKindId.fromKey),
            plannedRoute: plannedRoute,
            phaseSplits: splits
        )
    }
}

/// Codable wire shape for a persisted checkpoint split (D1). Mirrors ``PhaseSplit``.
private struct PhaseSplitDTO: Codable {
    let index: Int
    let name: String
    let crossedAtEpochMs: Int64
    let splitDistanceM: Double
    let splitSeconds: Int

    init(_ s: PhaseSplit) {
        index = s.index; name = s.name; crossedAtEpochMs = s.crossedAtEpochMs
        splitDistanceM = s.splitDistanceM; splitSeconds = s.splitSeconds
    }

    var model: PhaseSplit {
        PhaseSplit(index: index, name: name, crossedAtEpochMs: crossedAtEpochMs,
                   splitDistanceM: splitDistanceM, splitSeconds: splitSeconds)
    }
}

@Model
final class CollectionEntity {
    @Attribute(.unique) var id: String
    var name: String
    var colorArgb: Int?
    var icon: String?
    var itemCount: Int

    init(id: String, name: String, colorArgb: Int?, icon: String?, itemCount: Int) {
        self.id = id
        self.name = name
        self.colorArgb = colorArgb
        self.icon = icon
        self.itemCount = itemCount
    }

    convenience init(_ c: MapCollection) {
        self.init(id: c.id, name: c.name, colorArgb: c.colorArgb.map(Int.init), icon: c.icon, itemCount: c.itemCount)
    }

    func apply(_ c: MapCollection) {
        name = c.name; colorArgb = c.colorArgb.map(Int.init); icon = c.icon; itemCount = c.itemCount
    }

    var domain: MapCollection {
        MapCollection(id: id, name: name, colorArgb: colorArgb.map(Int64.init), icon: icon, itemCount: itemCount)
    }
}

/// Builds the app's SwiftData `ModelContainer`. `inMemoryContainer()` backs tests.
public enum TurboPersistence {
    public static let schema = Schema([MarkerEntity.self, PathEntity.self, CollectionEntity.self])

    public static func container() throws -> ModelContainer {
        try ModelContainer(for: schema, configurations: ModelConfiguration())
    }

    public static func inMemoryContainer() throws -> ModelContainer {
        try ModelContainer(for: schema, configurations: ModelConfiguration(isStoredInMemoryOnly: true))
    }
}
