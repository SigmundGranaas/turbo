import Foundation
import Observation
import CoreModel
import CoreMap

/// Drives the offline-maps screen and the "download this area" action. All tile
/// work lives behind the ``OfflineTileManager`` seam in `CoreMap`; this just
/// exposes the region list and translates a camera box into a download with a
/// sensible zoom span.
///
/// Mirrors `feature.offline.OfflineViewModel` (Android). `@Observable` +
/// `@MainActor` is the Swift equivalent of a Hilt `ViewModel` exposing a
/// `StateFlow`.
@MainActor
@Observable
public final class OfflineViewModel {
    public private(set) var regions: [OfflineRegionInfo] = []

    private let manager: OfflineTileManager
    private var observation: Task<Void, Never>?

    public init(manager: OfflineTileManager) {
        self.manager = manager
    }

    /// Begin observing the region stream (call from `.task`). Idempotent.
    public func start() {
        guard observation == nil else { return }
        observation = Task { [weak self, manager] in
            for await list in await manager.regionsStream() {
                self?.regions = list
            }
        }
        Task { await manager.refresh() }
    }

    public func stop() {
        observation?.cancel()
        observation = nil
    }

    /// Download the currently-visible `bounds` at base map `base`, spanning a few
    /// zoom levels around the current camera `fromZoom` so the area is usable
    /// offline. Zoom math mirrors the Android view model exactly.
    public func download(name: String, base: BaseLayer, bounds: GeoBounds, fromZoom: Double) {
        let minZoom = fromZoom.rounded(.down).clamped(to: Self.minZoom...Self.maxZoom)
        let maxZoom = min(minZoom + Self.zoomSpan, Self.maxZoom)
        Task { await manager.download(name: name, base: base, bounds: bounds, minZoom: minZoom, maxZoom: maxZoom) }
    }

    public func delete(id: Int64) {
        Task { await manager.delete(id: id) }
    }

    private static let minZoom: Double = 8
    private static let maxZoom: Double = 16
    private static let zoomSpan: Double = 4
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
