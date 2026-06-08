import SwiftUI
import CoreModel
import CoreMap
import CoreDesignSystem
#if canImport(UIKit)
import UIKit
#endif

/// The map home — a full-bleed Norgeskart map with floating Liquid Glass chrome:
/// weather chip, account avatar, the control rail (layers · follow · compass),
/// the bottom search pill + FAB, and a scale bar.
///
/// Mirrors `feature.map.MapScreen` (Android). Long-press the map to drop a marker.
public struct MapScreen: View {
    @Environment(\.turbo) private var t
    private var viewModel: MapViewModel
    private let onOpenSearch: () -> Void
    private let onOpenMenu: () -> Void
    private let onOpenLayers: () -> Void
    private let makeWeatherViewModel: ((LatLng) -> WeatherViewModel)?
    private let makeAvalancheViewModel: ((LatLng) -> AvalancheViewModel)?
    private let accountInitials: String?
    @State private var showWeather = false
    @State private var editorTarget: EditorTarget?
    @State private var mapCenter: LatLng?
    @State private var mapMetersPerPoint: Double = 0
    @State private var conditions: WeatherSummary?
    @State private var selectedMarker: Marker?
    @State private var compassResetToken = 0
    @State private var longPressCoord: LatLng?

    /// What the marker-editor sheet is editing — a new drop point (optionally
    /// prefilled from a searched place) or an existing marker.
    private enum EditorTarget: Identifiable {
        case new(LatLng, name: String)
        case edit(Marker)
        var id: String {
            switch self {
            case let .new(p, name): "new-\(name)-\(p.lat),\(p.lng)"
            case .edit(let m): "edit-\(m.id)"
            }
        }
    }

    public init(
        viewModel: MapViewModel,
        onOpenSearch: @escaping () -> Void = {},
        onOpenMenu: @escaping () -> Void = {},
        onOpenLayers: @escaping () -> Void = {},
        makeWeatherViewModel: ((LatLng) -> WeatherViewModel)? = nil,
        makeAvalancheViewModel: ((LatLng) -> AvalancheViewModel)? = nil,
        accountInitials: String? = nil
    ) {
        self.viewModel = viewModel
        self.onOpenSearch = onOpenSearch
        self.onOpenMenu = onOpenMenu
        self.onOpenLayers = onOpenLayers
        self.makeWeatherViewModel = makeWeatherViewModel
        self.makeAvalancheViewModel = makeAvalancheViewModel
        self.accountInitials = accountInitials
    }

    private var currentCenter: LatLng { mapCenter ?? LatLng(lat: 69.58, lng: 19.95) }

    public var body: some View {
        TurboMapView(
            baseLayer: viewModel.baseLayer,
            overlays: viewModel.overlays,
            pins: pins,
            following: viewModel.following,
            focus: viewModel.focusedPlace?.position,
            resetBearingToken: compassResetToken,
            onLongPress: { longPressCoord = $0 },
            onRegionChange: { mapCenter = $0; mapMetersPerPoint = $1 },
            onSelectPin: { id in selectedMarker = viewModel.markers.first { $0.id == id } }
        )
        .ignoresSafeArea()
        .animation(.easeOut(duration: 0.25), value: viewModel.focusedPlace)
        .animation(.easeOut(duration: 0.2), value: viewModel.following)
        .sensoryFeedback(.impact(weight: .medium), trigger: longPressCoord) { _, new in new != nil }
        .sensoryFeedback(.selection, trigger: viewModel.following)
        .overlay(alignment: .topLeading) { mapCenterProbe }
        .overlay(alignment: .top) { topChrome.mapChrome() }
        .overlay(alignment: .bottomTrailing) { controlRail.mapChrome() }
        .overlay(alignment: .bottomLeading) { scaleBar }
        .overlay(alignment: .bottom) { bottomBar.mapChrome() }
        .hideNavigationBar()
        .task {
            viewModel.start()
            if let vm = makeWeatherViewModel?(currentCenter) {
                await vm.load()
                conditions = vm.summary
            }
        }
        .sheet(item: $editorTarget) { target in
            switch target {
            case let .new(position, name):
                MarkerEditorSheet(viewModel: viewModel.makeEditor(at: position, name: name))
            case .edit(let marker):
                MarkerEditorSheet(viewModel: viewModel.makeEditor(for: marker))
            }
        }
        .sheet(item: $selectedMarker) { marker in
            MarkerDetailScreen(
                marker: marker,
                onEdit: { editorTarget = .edit(marker) },
                onDelete: { viewModel.deleteMarker(id: marker.id) }
            )
            .presentationDetents([.medium, .large])
        }
        .sheet(isPresented: $showWeather) {
            if let makeWeatherViewModel, let makeAvalancheViewModel {
                WeatherDetailScreen(
                    weather: makeWeatherViewModel(currentCenter),
                    avalanche: makeAvalancheViewModel(currentCenter)
                )
            }
        }
        .confirmationDialog(
            "Drop a point here?",
            isPresented: Binding(get: { longPressCoord != nil }, set: { if !$0 { longPressCoord = nil } }),
            presenting: longPressCoord
        ) { coord in
            Button("New Marker") { editorTarget = .new(coord, name: ""); longPressCoord = nil }
            openInMapsButton(coord)
            Button("Cancel", role: .cancel) { longPressCoord = nil }
        }
    }

    @ViewBuilder
    private func openInMapsButton(_ coord: LatLng) -> some View {
        #if os(iOS)
        Button("Open in Maps") {
            if let url = URL(string: "maps://?ll=\(coord.lat),\(coord.lng)") {
                UIApplication.shared.open(url)
            }
            longPressCoord = nil
        }
        #else
        EmptyView()
        #endif
    }

    private var pins: [MapPin] {
        var result = viewModel.markers.map { marker in
            MapPin(
                id: marker.id,
                coordinate: marker.position,
                title: marker.name,
                symbolName: marker.kind.symbolName,
                tint: marker.displayColor(t)
            )
        }
        if let place = viewModel.focusedPlace {
            result.append(MapPin(id: "focus-\(place.id)", coordinate: place.position,
                                 title: place.name, symbolName: "mappin", tint: t.red))
        }
        return result
    }

    private var topChrome: some View {
        VStack(spacing: 10) {
            HStack(alignment: .top) {
                Button { showWeather = true } label: {
                    WeatherChip(
                        symbol: conditions?.symbol.sfSymbol ?? "cloud.sun.fill",
                        temperature: conditions.map { WeatherSummary.formatTemperature($0.temperatureC) } ?? "—"
                    )
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("map.weather")
                .accessibilityLabel("Weather")
                Spacer()
                MapAvatar(initials: accountInitials, action: onOpenMenu)
                    .accessibilityIdentifier("map.avatar")
                    .accessibilityLabel("Account")
            }
            if let place = viewModel.focusedPlace {
                focusBanner(place)
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 8)
    }

    /// Shown after a search pick: names the centered place and offers to save it.
    private func focusBanner(_ place: MapViewModel.FocusedPlace) -> some View {
        HStack(spacing: 10) {
            Image(systemName: "mappin.circle.fill").foregroundStyle(t.red)
            Text(place.name).font(.turboHeadline).foregroundStyle(t.label).lineLimit(1)
            Spacer(minLength: 8)
            Button("Save") { editorTarget = .new(place.position, name: place.name) }
                .font(.turboHeadline)
                .foregroundStyle(t.blue)
                .accessibilityIdentifier("focus.save")
            Button { viewModel.clearFocus() } label: {
                Image(systemName: "xmark.circle.fill").foregroundStyle(t.label3)
            }
            .accessibilityIdentifier("focus.dismiss")
        }
        .padding(.horizontal, 14)
        .frame(height: 48)
        .liquidGlass(Capsule())
    }

    private var controlRail: some View {
        MapControlRail {
            MapRailButton(symbol: "square.2.stack.3d", action: onOpenLayers)
                .accessibilityIdentifier("map.layers")
                .accessibilityLabel("Map layers")
            MapRailDivider()
            MapRailButton(
                symbol: "location.north.fill",
                active: viewModel.following,
                action: viewModel.toggleFollowing
            )
            .accessibilityLabel("Follow my location")
            .accessibilityAddTraits(viewModel.following ? [.isSelected] : [])
            MapRailDivider()
            // Compass — reflects the live device heading; tap resets bearing north.
            Button(action: { compassResetToken += 1 }) {
                CompassDial(heading: viewModel.heading ?? 0)
                    .frame(width: 48, height: 48)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Compass; resets bearing to north")
        }
        .padding(.trailing, 16)
        .padding(.bottom, 86)
    }

    private var bottomBar: some View {
        HStack(spacing: 10) {
            SearchPill(action: onOpenSearch)
                .accessibilityIdentifier("map.search")
            MapFAB(symbol: "plus") {
                editorTarget = .new(currentCenter, name: "")
            }
            .accessibilityIdentifier("map.fab")
            .accessibilityLabel("New marker")
        }
        .padding(.horizontal, 14)
        .padding(.bottom, 12)
    }

    /// An invisible element that publishes the live map center so tests (and
    /// VoiceOver) can observe the camera. Not shown to sighted users.
    private var mapCenterProbe: some View {
        Text(mapCenter.map { String(format: "%.3f,%.3f", $0.lat, $0.lng) } ?? "")
            .opacity(0)
            .accessibilityIdentifier("map.center")
    }

    private var scaleBar: some View {
        let bar = MapScale.bar(metersPerPoint: mapMetersPerPoint, maxWidthPoints: 64)
        return ScaleBar(label: bar.label, width: bar.widthPoints)
            .padding(.leading, 18)
            .padding(.bottom, 96)
            .opacity(mapMetersPerPoint > 0 ? 1 : 0)   // hide until the first region settles
    }
}

private extension View {
    /// Floating chrome sits over the always-bright map raster, so it should read
    /// in light styling regardless of the app's light/dark setting — otherwise
    /// white text/glass becomes illegible over the bright map in dark mode.
    func mapChrome() -> some View {
        environment(\.colorScheme, .light)
            .environment(\.turbo, TurboColors(dark: false))
    }

    /// Hide the navigation bar so the map runs full-bleed. iOS-only API.
    @ViewBuilder func hideNavigationBar() -> some View {
        #if os(iOS)
        toolbar(.hidden, for: .navigationBar)
        #else
        self
        #endif
    }
}
