import Foundation
import CoreModel

/// Downloads and manages offline map regions. This is the *seam* — the only
/// place allowed to touch the map SDK (MapKit / MapLibre) once a real backend is
/// wired in. The rest of the app sees this protocol + the domain
/// ``OfflineRegionInfo``.
///
/// Mirrors the Kotlin `interface OfflineTileManager` in `:core:map`. The reactive
/// ``regions`` stream is the Swift equivalent of Android's
/// `StateFlow<List<OfflineRegionInfo>>`.
public protocol OfflineTileManager: Sendable {
    /// A snapshot of the current regions (the stream's latest value).
    func currentRegions() async -> [OfflineRegionInfo]
    /// Emits the region list on every change.
    func regionsStream() async -> AsyncStream<[OfflineRegionInfo]>
    /// Re-read the on-disk regions.
    func refresh() async
    /// Download `bounds` at base map `base`, spanning `minZoom...maxZoom`.
    func download(name: String, base: BaseLayer, bounds: GeoBounds, minZoom: Double, maxZoom: Double) async
    /// Delete a downloaded region and free its tiles.
    func delete(id: Int64) async
}
