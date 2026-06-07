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

    public init(container: AppContainer) {
        self.container = container
        _root = State(initialValue: RootViewModel(
            settingsRepository: container.settingsRepository,
            authRepository: container.authRepository
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
                    makeAvalancheViewModel: { container.makeAvalancheViewModel(position: $0) }
                )
                .navigationDestination(for: Route.self, destination: destination)
            }
            .sheet(isPresented: $showMenu) {
                AccountMenuSheet(
                    accountName: root.account?.displayName ?? "Guest",
                    accountEmail: root.account?.email,
                    onSelect: { path.append($0) },
                    onAccount: { showAuth = true }
                )
                .presentationDetents([.large])
            }
            .sheet(isPresented: $showAuth) {
                AuthScreen(viewModel: container.makeAuthViewModel())
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
            await container.seedIfEmpty()
            root.start()
        }
        .onChange(of: scenePhase) { _, phase in
            // Pull/push whenever the app comes to the foreground (mirrors MainActivity.onResume).
            if phase == .active {
                Task { await container.syncController.syncNow() }
            }
        }
    }

    @ViewBuilder
    private func destination(_ route: Route) -> some View {
        switch route {
        case .markers:
            MarkersScreen(viewModel: container.makeMarkersViewModel())
        case .paths:
            PathsScreen(viewModel: container.makePathsViewModel(),
                        makeRecordingViewModel: container.makeRecordingViewModel)
        case .collections:
            CollectionsScreen(viewModel: container.makeCollectionsViewModel())
        case .settings:
            SettingsScreen(
                viewModel: container.makeSettingsViewModel(),
                accountName: root.account?.displayName ?? "Sigmund Granaas",
                onOpenOffline: { path.append(Route.offline) }
            )
        case .offline:
            OfflineMapsScreen(viewModel: container.makeOfflineViewModel())
        }
    }
}
