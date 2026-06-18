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
    private let makeRouteViewModel: (() -> RouteViewModel)?
    private let makePhotosViewModel: ((Marker) -> MarkerPhotosViewModel)?
    private let shareResource: ((String) async -> URL?)?
    private let recording: RecordingStatus?
    private let onOpenRecording: (() -> Void)?
    private let onStartRecording: (() -> Void)?
    private let follow: FollowController?
    private let solveRoute: (@Sendable ([LatLng]) async -> RoutePlan?)?
    @State private var routing: RouteViewModel?
    @State private var drawStarted = false
    /// The waypoint index picked for moving (tap-to-select → tap-to-place).
    @State private var selectedWaypoint: Int?
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
        accountInitials: String? = nil,
        makeRouteViewModel: (() -> RouteViewModel)? = nil,
        makePhotosViewModel: ((Marker) -> MarkerPhotosViewModel)? = nil,
        shareResource: ((String) async -> URL?)? = nil,
        recording: RecordingStatus? = nil,
        onOpenRecording: (() -> Void)? = nil,
        onStartRecording: (() -> Void)? = nil,
        follow: FollowController? = nil,
        solveRoute: (@Sendable ([LatLng]) async -> RoutePlan?)? = nil
    ) {
        self.viewModel = viewModel
        self.onOpenSearch = onOpenSearch
        self.onOpenMenu = onOpenMenu
        self.onOpenLayers = onOpenLayers
        self.makeWeatherViewModel = makeWeatherViewModel
        self.makeAvalancheViewModel = makeAvalancheViewModel
        self.accountInitials = accountInitials
        self.makeRouteViewModel = makeRouteViewModel
        self.makePhotosViewModel = makePhotosViewModel
        self.shareResource = shareResource
        self.recording = recording
        self.onOpenRecording = onOpenRecording
        self.onStartRecording = onStartRecording
        self.follow = follow
        self.solveRoute = solveRoute
    }

    private var isFollowing: Bool { follow?.isFollowing ?? false }
    // Camera-follow is a single flag: on at app open + when a record/follow
    // session starts, off the moment the user pans (US-6).
    private var mapFollowing: Bool { viewModel.following }

    /// The polyline drawn on the map: the followed route, else the route/measure tool.
    private var drawnGeometry: [LatLng] {
        if isFollowing { return follow?.geometry ?? [] }
        if let routing { return routing.geometry }
        return []
    }

    /// Start following the currently-planned route, with auto-reroute via `solveRoute`.
    private func startFollowingPlannedRoute() {
        guard let routing, let plan = routing.plan, let follow else { return }
        let waypoints = routing.waypoints
        let route = FollowRoute(geometry: plan.geometry, distanceM: plan.distanceM,
                                ascentM: plan.ascentM, name: "Route", waypoints: waypoints)
        var reroute: (@Sendable ([LatLng]) async -> FollowRoute?)?
        if let solver = solveRoute {
            reroute = { points in
                guard let plan = await solver(points) else { return nil }
                return FollowRoute(geometry: plan.geometry, distanceM: plan.distanceM,
                                   ascentM: plan.ascentM, name: "Route", waypoints: points)
            }
        }
        follow.start(route, reroute: reroute)
        self.routing = nil
    }

    private var currentCenter: LatLng { mapCenter ?? LatLng(lat: 69.58, lng: 19.95) }

    /// A dragged route waypoint pin (id "wp-<index>") repositions that waypoint.
    private func handlePinMoved(_ id: String, _ coord: LatLng) {
        guard id.hasPrefix("wp-"), let index = Int(id.dropFirst(3)) else { return }
        routing?.moveWaypoint(at: index, to: coord)
    }

    /// Single-tap behaviour while building a track: if a waypoint is selected,
    /// move it to the tapped spot; otherwise add a new waypoint (route/line modes;
    /// draw uses the freehand gesture, where addWaypoint no-ops).
    private var tapHandler: ((LatLng) -> Void)? {
        guard routing != nil else { return nil }
        return { coord in
            if let index = selectedWaypoint {
                routing?.moveWaypoint(at: index, to: coord)
                selectedWaypoint = nil
            } else {
                routing?.addWaypoint(coord)
            }
        }
    }

    /// Tapping a waypoint pin selects it for moving (tap again to deselect); other
    /// pins open their marker detail.
    private func handleSelectPin(_ id: String) {
        if id.hasPrefix("wp-"), let index = Int(id.dropFirst(3)) {
            selectedWaypoint = (selectedWaypoint == index) ? nil : index
        } else {
            selectedMarker = viewModel.markers.first { $0.id == id }
        }
    }

    /// True while the track tool is in freehand Draw mode (drives the draw layer).
    private var isDrawing: Bool { routing?.mode == .draw }

    /// A transparent capture layer for freehand Draw — turns finger drags into
    /// track points (and consumes the gesture so the map doesn't pan). Mirrors
    /// Android's `detectDragGestures` draw overlay.
    @ViewBuilder
    private var drawLayer: some View {
        if isDrawing, let bounds = viewModel.visibleBounds {
            GeometryReader { geo in
                Color.clear
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture(minimumDistance: 0)
                            .onChanged { value in
                                let point = Self.coordinate(at: value.location, in: geo.size, bounds: bounds)
                                if drawStarted {
                                    routing?.appendDrawPoint(point)
                                } else {
                                    routing?.beginStroke()
                                    routing?.appendDrawPoint(point)
                                    drawStarted = true
                                }
                            }
                            .onEnded { _ in drawStarted = false }
                    )
            }
        }
    }

    /// Linear screen→geo mapping over the map's visible rect — fine at trail scale
    /// for freehand drawing.
    private static func coordinate(at point: CGPoint, in size: CGSize, bounds: GeoBounds) -> LatLng {
        let fx = min(max(point.x / max(size.width, 1), 0), 1)
        let fy = min(max(point.y / max(size.height, 1), 0), 1)
        return LatLng(
            lat: bounds.north - fy * (bounds.north - bounds.south),
            lng: bounds.west + fx * (bounds.east - bounds.west)
        )
    }

    public var body: some View {
        TurboMapView(
            baseLayer: viewModel.baseLayer,
            overlays: viewModel.overlays,
            pins: pins,
            following: mapFollowing,
            focus: viewModel.focusedPlace?.position,
            resetBearingToken: compassResetToken,
            routeGeometry: drawnGeometry,
            onLongPress: { longPressCoord = $0 },
            onRegionChange: { mapCenter = $0; mapMetersPerPoint = $1 },
            onVisibleBoundsChange: { viewModel.updateVisibleBounds($0) },
            onSelectPin: handleSelectPin,
            onTap: tapHandler,
            onPinMoved: handlePinMoved,
            onFollowDisengaged: { viewModel.setFollowing(false) }
        )
        .ignoresSafeArea()
        .overlay { drawLayer }   // below the chrome so the card stays interactive
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
            viewModel.setFollowing(true)   // auto-follow on open (US-6); releases on manual pan
            if let vm = makeWeatherViewModel?(currentCenter) {
                await vm.load()
                conditions = vm.state.value
            }
        }
        // Re-arm camera-follow when a follow/record session begins (still releases on pan).
        .onChange(of: isFollowing) { _, now in if now { viewModel.setFollowing(true) } }
        .onChange(of: recording?.isRecording ?? false) { _, now in if now { viewModel.setFollowing(true) } }
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
                onDelete: { viewModel.deleteMarker(id: marker.id) },
                makePhotos: makePhotosViewModel.map { f in { f(marker) } },
                shareResource: shareResource,
                makeWeather: makeWeatherViewModel,
                makeAvalanche: makeAvalancheViewModel
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
            if makeRouteViewModel != nil {
                // One track tool — Route / Line / Draw (Line/Draw double as the
                // measure tool via the live distance readout).
                Button("Plan a Track") {
                    let vm = makeRouteViewModel?()
                    vm?.addWaypoint(coord)
                    routing = vm
                    longPressCoord = nil
                }
            }
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
        let toolPoints = routing?.waypoints ?? []
        for (i, wp) in toolPoints.enumerated() {
            let selected = selectedWaypoint == i
            result.append(MapPin(
                id: "wp-\(i)", coordinate: wp, title: "\(i + 1)",
                symbolName: selected ? "scope" : "smallcircle.filled.circle",
                tint: selected ? t.orange : t.blue
            ))
        }
        // A pin at the long-pressed point, so the action sheet refers to a spot
        // the user can actually see.
        if let longPressCoord {
            result.append(MapPin(id: "longpress", coordinate: longPressCoord, title: "Dropped point", symbolName: "mappin", tint: t.red))
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
            if let recording { recordingPill(recording) }
            if let index = selectedWaypoint, index < (routing?.waypoints.count ?? 0) {
                movePointBanner(index)
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 8)
    }

    /// Shown while a waypoint is selected for moving — tap a new spot to place it.
    private func movePointBanner(_ index: Int) -> some View {
        HStack(spacing: 10) {
            Image(systemName: "scope").foregroundStyle(t.orange)
            Text("Moving point \(index + 1) — tap a new spot").font(.turboSubhead).foregroundStyle(t.label).lineLimit(1)
            Spacer(minLength: 8)
            Button("Cancel") { selectedWaypoint = nil }
                .font(.turboSubhead.weight(.semibold)).foregroundStyle(t.blue)
        }
        .padding(.horizontal, 14)
        .frame(height: 44)
        .liquidGlass(Capsule())
        .accessibilityIdentifier("route.movehint")
    }

    /// Ambient "recording in progress" pill — visible whenever a session is
    /// active even with the recording sheet closed, so the map stays usable.
    private func recordingPill(_ status: RecordingStatus) -> some View {
        Button { onOpenRecording?() } label: {
            HStack(spacing: 9) {
                Circle().fill(status.isRecording ? t.red : t.orange).frame(width: 10, height: 10)
                Text(status.isRecording ? "Recording" : "Paused").font(.turboHeadline).foregroundStyle(t.label)
                Text(status.label).font(.turboSubhead).monospacedDigit().foregroundStyle(t.label2)
                Spacer(minLength: 8)
                Image(systemName: "chevron.up").font(.system(size: 13, weight: .semibold)).foregroundStyle(t.label3)
            }
            .padding(.horizontal, 14)
            .frame(height: 48)
            .liquidGlass(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("map.recording")
        .accessibilityLabel("\(status.isRecording ? "Recording" : "Recording paused"); \(status.label). Open recording.")
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
            // Start a track recording. Hidden once a session is active — the
            // ambient recording pill takes over (tap it to manage / stop).
            if recording == nil, let onStartRecording {
                MapRailButton(symbol: "record.circle", action: onStartRecording)
                    .accessibilityIdentifier("map.record")
                    .accessibilityLabel("Record a track")
                MapRailDivider()
            }
            // Plan a track (Route / Line / Draw). Active while the tool is open.
            if makeRouteViewModel != nil {
                MapRailButton(symbol: "point.topleft.down.curvedto.point.bottomright.up", active: routing != nil) {
                    if routing == nil { routing = makeRouteViewModel?() }
                }
                .accessibilityIdentifier("map.track")
                .accessibilityLabel("Plan a track")
                MapRailDivider()
            }
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

    @ViewBuilder
    private var bottomBar: some View {
        if let follow, follow.isFollowing {
            FollowCard(controller: follow, onStop: { follow.stop() })
                .padding(.horizontal, 14)
                .padding(.bottom, 12)
        } else if let routing {
            RouteCard(viewModel: routing,
                      onClose: { self.routing = nil; selectedWaypoint = nil },
                      onFollow: follow != nil ? { startFollowingPlannedRoute() } : nil)
                .padding(.horizontal, 14)
                .padding(.bottom, 12)
        } else {
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
