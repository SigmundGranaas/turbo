import Foundation
import SwiftData
import CoreModel
import CoreCommon
import CoreData
import CoreAuth
import CoreSync
import CoreMap
import FeatureMap
import FeatureSearch
import FeatureSettings
import FeatureRecording
import FeatureAuth
import FeatureCollections
import FeatureOffline

/// The composition root — constructs the singletons (repositories, seams) and
/// vends view models with their dependencies injected. The hand-rolled
/// equivalent of Android's Hilt graph: one place that owns wiring, so feature
/// modules stay free of construction logic.
@MainActor
public final class AppContainer {

    // MARK: Singletons (seams / repositories)

    public let markerRepository: MarkerRepository
    public let settingsRepository: SettingsRepository
    public let searchRepository: SearchRepository
    public let pathRepository: PathRepository
    public let collectionRepository: CollectionRepository
    public let authRepository: AuthRepository
    public let offlineManager: OfflineTileManager
    public let locationProvider: LocationProvider
    public let weatherProvider: WeatherProvider
    public let avalancheProvider: AvalancheProvider
    public let reverseGeocode: ReverseGeocodeRepository
    public let sunProvider: SunProvider
    public let marineProvider: MarineProvider
    public let routeRepository: RouteRepository
    public let photoRepository: PhotoRepository
    public let sharingRepository: SharingRepository
    public let syncController: SyncController
    /// The app-lifetime recording session — owned here, not by any screen, so a
    /// track survives the recording sheet being dismissed (ambient recording).
    public let recordingController: RecordingController
    /// Whether the live API (auth + cloud sync) is configured for this build.
    public let isOnline: Bool

    /// Production wiring — SwiftData persistence + UserDefaults settings. With a
    /// configured `TurboConfig.apiBaseURL` it uses the live Google auth + HTTP
    /// sync; otherwise it stays fully local. Falls back to in-memory persistence
    /// if the store can't be opened (e.g. previews).
    public init(config: TurboConfig = .fromBundle()) {
        let container = try? TurboPersistence.container()
        if let container {
            markerRepository = SwiftDataMarkerRepository(container: container)
            pathRepository = SwiftDataPathRepository(container: container)
            collectionRepository = SwiftDataCollectionRepository(container: container)
        } else {
            markerRepository = InMemoryMarkerRepository(seed: [])
            pathRepository = InMemoryPathRepository(seed: [])
            collectionRepository = InMemoryCollectionRepository(seed: [])
        }
        settingsRepository = UserDefaultsSettingsRepository()
        searchRepository = CompositeSearchRepository(
            place: KartverketSearchRepository(),
            trails: GeonorgeTrailSearchRepository()
        )
        offlineManager = DiskOfflineTileManager()
        locationProvider = CoreLocationProvider()
        weatherProvider = MetNoWeatherProvider()
        avalancheProvider = VarsomAvalancheProvider()
        reverseGeocode = KartverketReverseGeocodeRepository()   // public stedsnavn API, no auth
        sunProvider = MetNoSunProvider()   // met.no Sunrise 3.0, no auth
        marineProvider = MetNoMarineProvider()   // met.no oceanforecast 2.0, no auth
        routeRepository = HttpRouteRepository()   // public routing API, no auth
        photoRepository = FilePhotoRepository()
        recordingController = RecordingController(
            location: locationProvider, pathRepository: pathRepository, activity: Self.makeActivityPresenter()
        )
        isOnline = config.isOnline

        let cursor = UserDefaultsCursorStore()
        if let base = config.apiBaseURL {
            // Live: Google sign-in + HTTP sync against the API.
            let auth = GoogleAuthRepository(apiBaseURL: base)
            authRepository = auth
            let bearer: @Sendable () async -> String? = { await auth.token() }
            sharingRepository = HttpSharingRepository(
                apiBaseURL: base, webBaseURL: URL(string: "https://kart.sandring.no")!, token: bearer
            )
            syncController = SyncController(
                units: [
                    Syncers.marker(repository: markerRepository,
                                   transport: HttpSyncTransport<MarkerPayload>(endpoint: base.appendingPathComponent("markers"), token: bearer),
                                   cursor: cursor),
                    Syncers.path(repository: pathRepository,
                                 transport: HttpSyncTransport<PathPayload>(endpoint: base.appendingPathComponent("paths"), token: bearer),
                                 cursor: cursor),
                    Syncers.collection(repository: collectionRepository,
                                       transport: HttpSyncTransport<CollectionPayload>(endpoint: base.appendingPathComponent("collections"), token: bearer),
                                       cursor: cursor),
                ],
                auth: authRepository, settings: settingsRepository
            )
        } else {
            // Offline: no auth backend (always signed out, no sign-in), no sync,
            // no sharing (never reached — sharing is signed-in only).
            authRepository = UnauthenticatedAuthRepository()
            sharingRepository = InMemorySharingRepository()
            syncController = SyncController(units: [], auth: authRepository, settings: settingsRepository)
        }
    }

    /// Explicit injection — for previews and tests.
    public init(
        markerRepository: MarkerRepository,
        settingsRepository: SettingsRepository = InMemorySettingsRepository(),
        searchRepository: SearchRepository = InMemorySearchRepository(),
        pathRepository: PathRepository = InMemoryPathRepository(),
        collectionRepository: CollectionRepository = InMemoryCollectionRepository(),
        authRepository: AuthRepository = InMemoryAuthRepository(),
        offlineManager: OfflineTileManager = InMemoryOfflineTileManager(),
        locationProvider: LocationProvider = SimulatedLocationProvider(fixes: []),
        weatherProvider: WeatherProvider = InMemoryWeatherProvider(),
        avalancheProvider: AvalancheProvider = InMemoryAvalancheProvider(),
        reverseGeocode: ReverseGeocodeRepository = InMemoryReverseGeocodeRepository(result: nil),
        sunProvider: SunProvider = InMemorySunProvider(),
        marineProvider: MarineProvider = InMemoryMarineProvider(),
        routeRepository: RouteRepository = InMemoryRouteRepository(),
        photoRepository: PhotoRepository = FilePhotoRepository(),
        sharingRepository: SharingRepository = InMemorySharingRepository(),
        isOnline: Bool = false
    ) {
        self.markerRepository = markerRepository
        self.settingsRepository = settingsRepository
        self.searchRepository = searchRepository
        self.pathRepository = pathRepository
        self.collectionRepository = collectionRepository
        self.authRepository = authRepository
        self.offlineManager = offlineManager
        self.locationProvider = locationProvider
        self.weatherProvider = weatherProvider
        self.avalancheProvider = avalancheProvider
        self.reverseGeocode = reverseGeocode
        self.sunProvider = sunProvider
        self.marineProvider = marineProvider
        self.routeRepository = routeRepository
        self.photoRepository = photoRepository
        self.sharingRepository = sharingRepository
        self.recordingController = RecordingController(
            location: locationProvider, pathRepository: pathRepository, activity: Self.makeActivityPresenter()
        )
        self.isOnline = isOnline
        self.syncController = AppContainer.makeSyncController(
            markers: markerRepository, paths: pathRepository, collections: collectionRepository,
            auth: authRepository, settings: settingsRepository, cursor: InMemoryCursorStore()
        )
    }

    /// Build the sync controller with one engine per entity, all on in-memory
    /// transports by default (real HTTP transports land once the API is configured).
    private static func makeSyncController(
        markers: MarkerRepository, paths: PathRepository, collections: CollectionRepository,
        auth: AuthRepository, settings: SettingsRepository, cursor: SyncCursorStore
    ) -> SyncController {
        SyncController(
            units: [
                Syncers.marker(repository: markers, transport: InMemorySyncTransport<MarkerPayload>(), cursor: cursor),
                Syncers.path(repository: paths, transport: InMemorySyncTransport<PathPayload>(), cursor: cursor),
                Syncers.collection(repository: collections, transport: InMemorySyncTransport<CollectionPayload>(), cursor: cursor),
            ],
            auth: auth, settings: settings
        )
    }

    /// Choose the container for this launch. With `-uitest` the app runs on
    /// deterministic, seeded **in-memory** backends (no network, no OAuth) so the
    /// end-to-end UI suite is hermetic; otherwise it uses the live backends.
    public static func resolve(arguments: [String] = ProcessInfo.processInfo.arguments) -> AppContainer {
        if arguments.contains("-uitest") {
            // Scripted location so recording captures a track deterministically.
            let fixes = (0..<6).map { i in
                LocationFix(position: LatLng(lat: 69.60 + Double(i) * 0.003, lng: 19.90 + Double(i) * 0.004),
                            headingDegrees: 45, altitude: 10 + Double(i) * 5)
            }
            return AppContainer(
                markerRepository: InMemoryMarkerRepository(),
                searchRepository: InMemorySearchRepository(),
                pathRepository: InMemoryPathRepository(),
                collectionRepository: InMemoryCollectionRepository(),
                authRepository: InMemoryAuthRepository(),
                offlineManager: InMemoryOfflineTileManager(),
                locationProvider: SimulatedLocationProvider(fixes: fixes, interval: .milliseconds(80)),
                isOnline: true   // exercise the sign-in flow with the in-memory double
            )
        }
        return AppContainer()
    }

    // MARK: View-model factories

    public func makeMapViewModel() -> MapViewModel {
        MapViewModel(markerRepository: markerRepository, location: locationProvider)
    }
    public func makeMarkersViewModel() -> MarkersViewModel { MarkersViewModel(repository: markerRepository) }
    public func makeRouteViewModel() -> RouteViewModel {
        RouteViewModel(routeRepository: routeRepository, pathRepository: pathRepository)
    }
    public func makePhotosViewModel(marker: Marker) -> MarkerPhotosViewModel {
        MarkerPhotosViewModel(repository: photoRepository, marker: marker)
    }
    public func makeSearchViewModel() -> SearchViewModel { SearchViewModel(repository: searchRepository) }
    public func makeSettingsViewModel() -> SettingsViewModel { SettingsViewModel(repository: settingsRepository) }
    public func makePathsViewModel() -> PathsViewModel { PathsViewModel(repository: pathRepository) }
    /// The real ActivityKit presenter on device; a no-op elsewhere (host build,
    /// `-uitest`, or where Live Activities are unsupported).
    private static func makeActivityPresenter() -> RecordingActivityPresenter {
        #if canImport(ActivityKit) && os(iOS)
        if ProcessInfo.processInfo.arguments.contains("-uitest") { return NoRecordingActivityPresenter() }
        return LiveActivityPresenter()
        #else
        return NoRecordingActivityPresenter()
        #endif
    }
    public func makeWeatherViewModel(position: LatLng, placeName: String) -> WeatherViewModel {
        WeatherViewModel(provider: weatherProvider, position: position, placeName: placeName,
                         reverseGeocode: reverseGeocode, sunProvider: sunProvider, marineProvider: marineProvider)
    }
    public func makeAvalancheViewModel(position: LatLng) -> AvalancheViewModel {
        AvalancheViewModel(provider: avalancheProvider, position: position)
    }
    public func makeCollectionsViewModel() -> CollectionsViewModel { CollectionsViewModel(repository: collectionRepository) }
    public func makeAuthViewModel() -> AuthViewModel { AuthViewModel(repository: authRepository) }
    public func makeOfflineViewModel() -> OfflineViewModel { OfflineViewModel(manager: offlineManager) }

    /// Mint a share link for a resource via the sharing service. Returns nil on
    /// failure (e.g. not authenticated). Callers only surface this when online +
    /// signed in.
    public func shareLink(resourceId: String) async -> URL? {
        await sharingRepository.createLink(resourceId: resourceId).getOrNil()
    }
}
