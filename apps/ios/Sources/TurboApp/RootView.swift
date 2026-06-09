import SwiftUI
import CoreModel
import CoreDesignSystem
import FeatureMap
import FeatureSearch
import FeatureSettings
import FeatureRecording
import FeatureCollections
import FeatureLayers
import FeatureAuth
import FeatureOffline

/// Root navigation scaffold. Mirrors `TurboNavGraph` (Android) — a single
/// `NavigationStack` with the map home as the start destination, the account menu
/// as a sheet, search + layers as sheets, and the rest pushed onto the stack.
public struct RootView: View {
    @Environment(\.scenePhase) private var scenePhase
    private let container: AppContainer
    @State private var root: RootViewModel
    @State private var mapViewModel: MapViewModel
    @State private var path = NavigationPath()
    @State private var showMenu = false
    @State private var showSearch = false
    @State private var showLayers = false
    @State private var showAuth = false
    @State private var showRecording = false

    public init(container: AppContainer) {
        self.container = container
        _root = State(initialValue: RootViewModel(
            settingsRepository: container.settingsRepository,
            authRepository: container.authRepository,
            sharingRepository: container.sharingRepository
        ))
        _mapViewModel = State(initialValue: container.makeMapViewModel())
    }

    /// Push destinations off the map home. Mirrors the Android route set.
    public enum Route: Hashable {
        case markers, paths, collections, settings, offline
    }

    public var body: some View {
        TurboTheme(scheme: root.colorScheme) {
            NavigationStack(path: $path) {
                MapScreen(
                    viewModel: mapViewModel,
                    onOpenSearch: { showSearch = true },
                    onOpenMenu: { showMenu = true },
                    onOpenLayers: { showLayers = true },
                    makeWeatherViewModel: { container.makeWeatherViewModel(position: $0, placeName: "Conditions") },
                    makeAvalancheViewModel: { container.makeAvalancheViewModel(position: $0) },
                    accountInitials: root.account.map { AccountMenuSheet.initials($0.displayName) },
                    makeRouteViewModel: container.makeRouteViewModel,
                    makePhotosViewModel: container.makePhotosViewModel,
                    shareResource: shareResource,
                    recording: recordingStatus,
                    onOpenRecording: { showRecording = true },
                    onStartRecording: { container.recordingController.start(); showRecording = true }
                )
                .navigationDestination(for: Route.self, destination: destination)
            }
            .sheet(isPresented: $showMenu) {
                AccountMenuSheet(
                    accountName: root.account?.displayName,
                    accountEmail: root.account?.email,
                    friendCode: root.friendCode,
                    canSignIn: container.isOnline && root.account == nil,
                    syncStatus: root.account != nil ? container.syncStatus : nil,
                    onSelect: { path.append($0) },
                    onAccount: { showAuth = true },
                    onSync: { Task { await container.syncController.syncNow() } }
                )
                .presentationDetents([.large])
            }
            .sheet(isPresented: $showAuth) {
                AuthScreen(viewModel: container.makeAuthViewModel())
            }
            .sheet(isPresented: $showRecording) {
                RecordingScreen(controller: container.recordingController)
            }
            .sheet(isPresented: $showSearch) {
                SearchScreen(viewModel: container.makeSearchViewModel()) { name, position in
                    mapViewModel.focus(on: position, name: name)
                }
            }
            .sheet(isPresented: $showLayers) {
                let bindable = Bindable(mapViewModel)
                MapLayersSheet(baseLayer: bindable.baseLayer, overlays: bindable.overlays)
                    .presentationDetents([.medium, .large])
            }
        }
        .task {
            root.start()
        }
        .onChange(of: scenePhase) { _, phase in
            // Pull/push whenever the app comes to the foreground (mirrors MainActivity.onResume).
            if phase == .active {
                Task { await container.syncController.syncNow() }
            }
        }
    }

    /// A per-resource share-link minter, offered only when signed in (the
    /// sharing service is token-authed). Nil otherwise, so the UI hides sharing.
    private var shareResource: ((String) async -> URL?)? {
        guard root.account != nil else { return nil }
        return { await container.shareLink(resourceId: $0) }
    }

    /// A snapshot of the active recording session for the map's ambient pill;
    /// nil when no session is running. Reading the controller here registers the
    /// observation so the map updates live.
    private var recordingStatus: RecordingStatus? {
        let c = container.recordingController
        guard c.isSessionActive else { return nil }
        return RecordingStatus(isRecording: c.isRecording, distanceMeters: c.distanceMeters, elapsedSeconds: c.elapsedSeconds)
    }

    @ViewBuilder
    private func destination(_ route: Route) -> some View {
        switch route {
        case .markers:
            MarkersScreen(viewModel: container.makeMarkersViewModel(),
                          makePhotosViewModel: container.makePhotosViewModel,
                          shareResource: shareResource)
        case .paths:
            PathsScreen(viewModel: container.makePathsViewModel(),
                        onStartRecording: { container.recordingController.start(); showRecording = true },
                        shareResource: shareResource)
        case .collections:
            CollectionsScreen(viewModel: container.makeCollectionsViewModel())
        case .settings:
            SettingsScreen(
                viewModel: container.makeSettingsViewModel(),
                accountName: root.account?.displayName,
                onOpenOffline: { path.append(Route.offline) }
            )
        case .offline:
            OfflineMapsScreen(
                viewModel: container.makeOfflineViewModel(),
                currentBounds: mapViewModel.visibleBounds,
                currentBase: mapViewModel.baseLayer,
                resolveName: { bounds in
                    let center = LatLng(lat: (bounds.south + bounds.north) / 2, lng: (bounds.west + bounds.east) / 2)
                    return await container.reverseGeocode.describe(center)?.title
                }
            )
        }
    }
}
