import Foundation
import CoreModel
import CoreCommon

/// Reads and writes recorded/saved tracks. Mirrors `core.data.PathRepository`.
public protocol PathRepository: Sendable {
    func current() async -> [SavedPath]
    func stream() async -> AsyncStream<[SavedPath]>
    func upsert(_ path: SavedPath) async
    func delete(id: String) async
}

public final class InMemoryPathRepository: PathRepository {
    private let store: ReactiveStore<[SavedPath]>

    public init(seed: [SavedPath] = InMemoryPathRepository.sample) {
        store = ReactiveStore(seed)
    }

    public func current() async -> [SavedPath] { await store.current() }
    public func stream() async -> AsyncStream<[SavedPath]> { await store.stream() }

    public func upsert(_ path: SavedPath) async {
        await store.update { paths in
            var next = paths.filter { $0.id != path.id }
            next.append(path)
            return next
        }
    }

    public func delete(id: String) async {
        await store.update { $0.filter { $0.id != id } }
    }

    /// Sample recorded tracks (with elevation profiles for the list sparklines),
    /// mirroring the `PathsList` design.
    public static let sample: [SavedPath] = [
        path("p1", "Storheia Loop", .hiking, km: 12.4, day: 24, elev: [8, 10, 14, 20, 16, 12, 18, 22, 14, 9, 7]),
        path("p2", "Tromsdalstinden", .mountain, km: 9.1, day: 18, elev: [6, 8, 12, 18, 24, 20, 14, 10, 12, 8, 6]),
        path("p3", "Fløya ridge", .hiking, km: 6.8, day: 11, elev: [10, 14, 12, 16, 14, 18, 16, 12, 14, 10, 8]),
        path("p4", "Kvaløya coast", .biking, km: 15.2, day: 3, elev: [12, 10, 11, 9, 12, 10, 13, 11, 9, 10, 8]),
        path("p5", "Lyngen traverse", .skiing, km: 21.6, day: 27, elev: [6, 10, 16, 22, 18, 24, 20, 16, 12, 9, 6]),
    ]

    private static func path(_ id: String, _ name: String, _ kind: ActivityKindId, km: Double, day: Int, elev: [Double]) -> SavedPath {
        // Synthesise a short track near Tromsø so exports carry real geometry.
        let points = elev.indices.map { i in
            LatLng(lat: 69.60 + Double(i) * 0.004, lng: 19.90 + Double(i) * 0.006)
        }
        return SavedPath(
            id: id, name: name,
            path: GeoPath(
                points: points,
                source: .recording,
                elevations: elev,
                distanceM: km * 1000,
                recordedAtEpochMs: nil
            ),
            activityKind: kind
        )
    }
}
