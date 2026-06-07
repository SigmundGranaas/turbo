import Foundation
import Observation
import CoreModel
import CoreData
import CoreDesignSystem

/// Holds the map home's UI state — markers (from ``MarkerRepository``), the active
/// base layer, and whether the camera follows the user. Mirrors
/// `feature.map.MapViewModel` + `MapUiState` (Android).
@MainActor
@Observable
public final class MapViewModel {
    public private(set) var markers: [Marker] = []
    public var baseLayer: BaseLayer
    public var overlays: Set<OverlayId> = [.avalanche, .trails]
    public var following: Bool
    /// The user's live position + heading (from ``LocationProvider``).
    public private(set) var userLocation: LatLng?
    public private(set) var heading: Double?
    /// The place the map is centered on after a search pick (drives the camera +
    /// a "save this place" banner). `nil` when no search result is active.
    public private(set) var focusedPlace: FocusedPlace?

    /// A transient, non-persisted place the map is focused on.
    public struct FocusedPlace: Identifiable, Equatable, Sendable {
        public let id = UUID()
        public let name: String
        public let position: LatLng
    }

    private let markerRepository: MarkerRepository
    private let location: LocationProvider?
    private var observation: Task<Void, Never>?
    private var locationObservation: Task<Void, Never>?

    public init(
        markerRepository: MarkerRepository,
        location: LocationProvider? = nil,
        baseLayer: BaseLayer = .norgeskart,
        following: Bool = false
    ) {
        self.markerRepository = markerRepository
        self.location = location
        self.baseLayer = baseLayer
        self.following = following
    }

    /// Begin observing markers (call from `.task`). Idempotent.
    public func start() {
        guard observation == nil else { return }
        observation = Task { [weak self, markerRepository] in
            for await list in await markerRepository.stream() {
                self?.markers = list
            }
        }
    }

    public func stop() {
        observation?.cancel()
        observation = nil
        locationObservation?.cancel()
        locationObservation = nil
    }

    /// Begin streaming the user's location + heading into ``userLocation`` /
    /// ``heading``. Idempotent. Requests permission on first call. Mirrors
    /// `MapViewModel.enableLocation()` on Android.
    public func enableLocation() {
        guard locationObservation == nil, let location else { return }
        location.requestAuthorization()
        locationObservation = Task { [weak self, location] in
            for await fix in location.fixes() {
                self?.userLocation = fix.position
                if let heading = fix.headingDegrees { self?.heading = heading }
            }
        }
    }

    /// Cycle the base map (the rail's layers button) — Norgeskart → OSM →
    /// Satellite → … . The full layer sheet lives in `FeatureLayers`.
    public func cycleBaseLayer() {
        let all = BaseLayer.allCases
        let next = (all.firstIndex(of: baseLayer).map { $0 + 1 } ?? 0) % all.count
        baseLayer = all[next]
    }

    public func toggleFollowing() {
        following.toggle()
        if following { enableLocation() }
    }

    /// Drop a new marker (long-press the map, or the FAB). Offline-first; the map
    /// updates from the repository stream.
    public func addMarker(at position: LatLng, kind: ActivityKindId = .mountain) {
        let marker = Marker(id: "m-\(UUID().uuidString)", name: kind.label, kind: kind, position: position)
        Task { await markerRepository.upsert(marker) }
    }

    /// Center the map on a searched place and offer to save it.
    public func focus(on position: LatLng, name: String) {
        focusedPlace = FocusedPlace(name: name, position: position)
    }

    public func clearFocus() { focusedPlace = nil }

    /// An editor for a new marker at `position`, optionally prefilled (search → save).
    public func makeEditor(at position: LatLng, name: String = "") -> MarkerEditorViewModel {
        MarkerEditorViewModel(repository: markerRepository, position: position, name: name)
    }

    /// An editor for an existing marker.
    public func makeEditor(for marker: Marker) -> MarkerEditorViewModel {
        MarkerEditorViewModel(repository: markerRepository, marker: marker)
    }

    public func deleteMarker(id: String) {
        Task { await markerRepository.delete(id: id) }
    }
}
