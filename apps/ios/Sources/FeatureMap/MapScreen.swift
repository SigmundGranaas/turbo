import SwiftUI
import CoreModel
import CoreMap
import CoreDesignSystem

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
    @State private var editorTarget: EditorTarget?
    @State private var mapCenter: LatLng?
    @State private var selectedMarker: Marker?

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
        onOpenLayers: @escaping () -> Void = {}
    ) {
        self.viewModel = viewModel
        self.onOpenSearch = onOpenSearch
        self.onOpenMenu = onOpenMenu
        self.onOpenLayers = onOpenLayers
    }

    public var body: some View {
        TurboMapView(
            baseLayer: viewModel.baseLayer,
            pins: pins,
            following: viewModel.following,
            focus: viewModel.focusedPlace?.position,
            onLongPress: { editorTarget = .new($0, name: "") },
            onRegionChange: { mapCenter = $0 },
            onSelectPin: { id in selectedMarker = viewModel.markers.first { $0.id == id } }
        )
        .ignoresSafeArea()
        .overlay(alignment: .topLeading) { mapCenterProbe }
        .overlay(alignment: .top) { topChrome }
        .overlay(alignment: .bottomTrailing) { controlRail }
        .overlay(alignment: .bottomLeading) { scaleBar }
        .overlay(alignment: .bottom) { bottomBar }
        .hideNavigationBar()
        .task { viewModel.start() }
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
    }

    private var pins: [MapPin] {
        var result = viewModel.markers.map { marker in
            MapPin(
                id: marker.id,
                coordinate: marker.position,
                title: marker.name,
                symbolName: marker.kind.symbolName,
                tint: marker.kind.tint(t)
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
                WeatherChip(temperature: "−3°")
                Spacer()
                MapAvatar(initials: "SG", action: onOpenMenu)
                    .accessibilityIdentifier("map.avatar")
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
            MapRailDivider()
            MapRailButton(
                symbol: "location.north.fill",
                active: viewModel.following,
                action: viewModel.toggleFollowing
            )
            MapRailDivider()
            // Compass — reflects the live device heading.
            Button(action: {}) {
                CompassDial(heading: viewModel.heading ?? 0)
                    .frame(width: 48, height: 48)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
        .padding(.trailing, 16)
        .padding(.bottom, 86)
    }

    private var bottomBar: some View {
        HStack(spacing: 10) {
            SearchPill(action: onOpenSearch)
                .accessibilityIdentifier("map.search")
            MapFAB(symbol: "plus") {
                editorTarget = .new(LatLng(lat: 69.58, lng: 19.95), name: "")
            }
            .accessibilityIdentifier("map.fab")
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
        ScaleBar()
            .padding(.leading, 18)
            .padding(.bottom, 96)
    }
}

private extension View {
    /// Hide the navigation bar so the map runs full-bleed. iOS-only API.
    @ViewBuilder func hideNavigationBar() -> some View {
        #if os(iOS)
        toolbar(.hidden, for: .navigationBar)
        #else
        self
        #endif
    }
}
