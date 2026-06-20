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
    public var overlays: Set<OverlayId> = []
    /// Nasjonal Turbase (ut.no / DNT) "Cabins & trips" overlay on/off. Toggled
    /// from the Layers sheet; drives the in-view POI fetch + on-map pins.
    public var showCabins: Bool = false
    /// In-view POIs while the overlay is on (rendered as pins). Empty when off.
    public private(set) var ntbPois: [NtbPoi] = []
    /// The POI whose info sheet is open (`nil` = none).
    public private(set) var ntbSelected: NtbPoi?
    /// The selected trip's loaded route, revealed on the map (`nil` until loaded).
    public private(set) var ntbRoute: NtbRoute?
    public var following: Bool
    /// The user's live position + heading (from ``LocationProvider``).
    public private(set) var userLocation: LatLng?
    public private(set) var heading: Double?
    /// The map's current visible bounds — lets "download offline maps" grab the
    /// area you're actually looking at instead of a fixed demo region.
    public private(set) var visibleBounds: GeoBounds?
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
    private let nasjonalTurbase: NasjonalTurbaseRepository?
    private var observation: Task<Void, Never>?
    private var locationObservation: Task<Void, Never>?
    private var ntbTask: Task<Void, Never>?
    private var ntbRouteTask: Task<Void, Never>?

    /// Above this viewport span (degrees) the cabins/trips overlay stays empty —
    /// at country zoom the markers would be meaningless (and the proxy rejects it).
    private static let maxCabinSpanDeg = 1.5

    public init(
        markerRepository: MarkerRepository,
        location: LocationProvider? = nil,
        nasjonalTurbase: NasjonalTurbaseRepository? = nil,
        baseLayer: BaseLayer = .norgeskart,
        following: Bool = false
    ) {
        self.markerRepository = markerRepository
        self.location = location
        self.nasjonalTurbase = nasjonalTurbase
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
        ntbTask?.cancel()
        ntbTask = nil
        ntbRouteTask?.cancel()
        ntbRouteTask = nil
    }

    // MARK: - Nasjonal Turbase (ut.no / DNT) "Cabins & trips" overlay

    /// Reload the in-view POIs for the overlay. Called on each region change while
    /// the overlay is on; skips very large viewports and coalesces rapid pans.
    public func refreshCabins(in bounds: GeoBounds) {
        guard showCabins, let nasjonalTurbase else { return }
        if bounds.north - bounds.south > Self.maxCabinSpanDeg || bounds.east - bounds.west > Self.maxCabinSpanDeg {
            if !ntbPois.isEmpty { ntbPois = [] }
            return
        }
        ntbTask?.cancel()
        ntbTask = Task { [weak self] in
            let pois = await nasjonalTurbase.pois(in: bounds)
            guard !Task.isCancelled else { return }
            self?.ntbPois = pois
        }
    }

    /// Toggle the overlay off: drop the pins, selection and revealed route.
    public func clearCabins() {
        ntbTask?.cancel()
        ntbRouteTask?.cancel()
        ntbPois = []
        ntbSelected = nil
        ntbRoute = nil
    }

    /// Open the info sheet for `poi`; for a trip, fetch + reveal its route.
    public func selectNtb(_ poi: NtbPoi) {
        ntbRouteTask?.cancel()
        ntbSelected = poi
        ntbRoute = nil
        guard poi.hasRoute, let nasjonalTurbase else { return }
        ntbRouteTask = Task { [weak self] in
            let route = await nasjonalTurbase.route(id: poi.id)
            guard !Task.isCancelled else { return }
            // Ignore a late result if the user already moved on to another POI.
            if self?.ntbSelected?.id == poi.id { self?.ntbRoute = route }
        }
    }

    /// Dismiss the info sheet and hide the revealed route.
    public func dismissNtb() {
        ntbRouteTask?.cancel()
        ntbSelected = nil
        ntbRoute = nil
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
        setFollowing(!following)
    }

    /// Set camera-follow explicitly — true on app open and when a record/follow
    /// session starts; false the moment the user pans the map (US-6).
    public func setFollowing(_ value: Bool) {
        guard following != value else { return }
        following = value
        if value { enableLocation() }
    }

    /// Record the map's current visible rectangle (from the map view's region).
    public func updateVisibleBounds(_ bounds: GeoBounds) { visibleBounds = bounds }

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
