import Foundation
import CoreModel
import CoreCommon

/// Reads and writes the user's markers. Mirrors `core.data.MarkerRepository`.
/// Offline-first: callers observe ``stream()`` and the map updates on every change.
public protocol MarkerRepository: Sendable {
    func current() async -> [Marker]
    func stream() async -> AsyncStream<[Marker]>
    func upsert(_ marker: Marker) async
    func delete(id: String) async
}

/// In-memory implementation. A SwiftData-backed store swaps in behind this
/// protocol later, exactly as the offline tile seam will gain a real backend.
public final class InMemoryMarkerRepository: MarkerRepository {
    private let store: ReactiveStore<[Marker]>

    public init(seed: [Marker] = InMemoryMarkerRepository.sample) {
        store = ReactiveStore(seed)
    }

    public func current() async -> [Marker] { await store.current() }
    public func stream() async -> AsyncStream<[Marker]> { await store.stream() }

    public func upsert(_ marker: Marker) async {
        await store.update { markers in
            var next = markers.filter { $0.id != marker.id }
            next.append(marker)
            return next
        }
    }

    public func delete(id: String) async {
        await store.update { $0.filter { $0.id != id } }
    }

    public static let sample: [Marker] = [
        Marker(id: "s1", name: "Storvika camp", kind: .camping, position: LatLng(lat: 69.62, lng: 20.05)),
        Marker(id: "s2", name: "Heggmotinden", kind: .mountain, position: LatLng(lat: 69.55, lng: 19.88)),
        Marker(id: "s3", name: "Storvikelva", kind: .fishing, position: LatLng(lat: 69.60, lng: 20.12)),
    ]
}
