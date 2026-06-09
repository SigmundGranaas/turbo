import SwiftUI
import MapKit
import CoreModel

#if canImport(UIKit)
import UIKit

/// The full-bleed Turbo map: a raster `MKTileOverlay` of the chosen ``BaseLayer``
/// with the user's markers and live location on top. Lives in `CoreMap` — the
/// only place allowed to touch the map SDK — mirroring Android's `ui.map.TurboMap`.
///
/// All chrome (search, control rail, FAB) floats *over* this in `FeatureMap`.
public struct TurboMapView: UIViewRepresentable {
    private let baseLayer: BaseLayer
    private let overlays: Set<OverlayId>
    private let pins: [MapPin]
    private let following: Bool
    private let focus: LatLng?
    private let resetBearingToken: Int
    private let routeGeometry: [LatLng]
    private let onLongPress: ((LatLng) -> Void)?
    private let onRegionChange: ((LatLng, Double) -> Void)?
    private let onSelectPin: ((String) -> Void)?
    private let onTap: ((LatLng) -> Void)?

    /// Default camera — the Lyngen/Tromsø region, so topo tiles show on launch.
    private static let defaultCenter = CLLocationCoordinate2D(latitude: 69.58, longitude: 19.95)

    public init(
        baseLayer: BaseLayer,
        overlays: Set<OverlayId> = [],
        pins: [MapPin],
        following: Bool = false,
        focus: LatLng? = nil,
        resetBearingToken: Int = 0,
        routeGeometry: [LatLng] = [],
        onLongPress: ((LatLng) -> Void)? = nil,
        onRegionChange: ((LatLng, Double) -> Void)? = nil,
        onSelectPin: ((String) -> Void)? = nil,
        onTap: ((LatLng) -> Void)? = nil
    ) {
        self.baseLayer = baseLayer
        self.overlays = overlays
        self.pins = pins
        self.following = following
        self.focus = focus
        self.resetBearingToken = resetBearingToken
        self.routeGeometry = routeGeometry
        self.onLongPress = onLongPress
        self.onRegionChange = onRegionChange
        self.onSelectPin = onSelectPin
        self.onTap = onTap
    }

    public func makeCoordinator() -> Coordinator { Coordinator(onLongPress: onLongPress) }

    public func makeUIView(context: Context) -> MKMapView {
        let map = MKMapView()
        map.delegate = context.coordinator
        map.pointOfInterestFilter = .excludingAll
        map.showsCompass = false   // we draw our own Liquid Glass compass
        map.showsScale = false
        map.accessibilityIdentifier = "map.canvas"
        map.region = MKCoordinateRegion(
            center: Self.defaultCenter,
            span: MKCoordinateSpan(latitudeDelta: 0.35, longitudeDelta: 0.35)
        )
        let press = UILongPressGestureRecognizer(
            target: context.coordinator,
            action: #selector(Coordinator.handleLongPress(_:))
        )
        map.addGestureRecognizer(press)
        let tap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleTap(_:)))
        map.addGestureRecognizer(tap)
        context.coordinator.installOverlay(on: map, base: baseLayer)
        return map
    }

    public func updateUIView(_ map: MKMapView, context: Context) {
        context.coordinator.onLongPress = onLongPress
        context.coordinator.onRegionChange = onRegionChange
        context.coordinator.onSelectPin = onSelectPin
        context.coordinator.onTap = onTap
        context.coordinator.syncOverlay(on: map, base: baseLayer)
        context.coordinator.syncDataOverlays(on: map, overlays: overlays)
        context.coordinator.syncRoute(on: map, geometry: routeGeometry)
        context.coordinator.syncAnnotations(on: map, pins: pins)
        context.coordinator.applyBearingReset(on: map, token: resetBearingToken)
        // Always show the blue user-location dot; follow mode only changes whether
        // the camera tracks it.
        map.showsUserLocation = true
        let desiredMode: MKUserTrackingMode = following ? .follow : .none
        if map.userTrackingMode != desiredMode {
            map.setUserTrackingMode(desiredMode, animated: true)
        }
        context.coordinator.applyFocus(on: map, focus: focus)
    }

    // MARK: - Coordinator

    public final class Coordinator: NSObject, MKMapViewDelegate {
        var onLongPress: ((LatLng) -> Void)?
        var onRegionChange: ((LatLng, Double) -> Void)?
        var onSelectPin: ((String) -> Void)?
        var onTap: ((LatLng) -> Void)?
        private var currentBase: BaseLayer?
        private var tileOverlay: MKTileOverlay?
        private var dataOverlays: [OverlayId: MKTileOverlay] = [:]
        private var routeOverlay: MKPolyline?
        private var lastRouteCount = -1
        private var lastFocus: LatLng?
        private var lastBearingToken = 0

        /// Draw/replace the route polyline.
        func syncRoute(on map: MKMapView, geometry: [LatLng]) {
            guard geometry.count != lastRouteCount || (geometry.isEmpty && routeOverlay != nil) else { return }
            lastRouteCount = geometry.count
            if let old = routeOverlay { map.removeOverlay(old); routeOverlay = nil }
            guard geometry.count >= 2 else { return }
            let coords = geometry.map { CLLocationCoordinate2D(latitude: $0.lat, longitude: $0.lng) }
            let line = MKPolyline(coordinates: coords, count: coords.count)
            map.addOverlay(line, level: .aboveLabels)
            routeOverlay = line
        }

        @objc func handleTap(_ gesture: UITapGestureRecognizer) {
            guard onTap != nil, gesture.state == .ended, let map = gesture.view as? MKMapView else { return }
            let point = gesture.location(in: map)
            // Ignore taps that hit an annotation (those select the pin).
            if map.annotations.contains(where: { annotation in
                guard let view = map.view(for: annotation) else { return false }
                return view.frame.contains(point)
            }) { return }
            let coord = map.convert(point, toCoordinateFrom: map)
            onTap?(LatLng(lat: coord.latitude, lng: coord.longitude))
        }

        /// Add/remove transparent data-overlay tile layers above the base map.
        func syncDataOverlays(on map: MKMapView, overlays: Set<OverlayId>) {
            for (id, overlay) in dataOverlays where !overlays.contains(id) {
                map.removeOverlay(overlay)
                dataOverlays[id] = nil
            }
            for id in overlays where dataOverlays[id] == nil {
                guard let template = MapTileStyles.overlayTemplate(for: id) else { continue }
                let overlay = MKTileOverlay(urlTemplate: template)
                overlay.canReplaceMapContent = false
                map.addOverlay(overlay, level: .aboveLabels)
                dataOverlays[id] = overlay
            }
        }

        /// Reset the camera bearing to north when the compass is tapped.
        func applyBearingReset(on map: MKMapView, token: Int) {
            guard token != lastBearingToken else { return }
            lastBearingToken = token
            let camera = map.camera
            camera.heading = 0
            map.setCamera(camera, animated: true)
        }

        init(onLongPress: ((LatLng) -> Void)?) { self.onLongPress = onLongPress }

        /// Center the camera on a new focus coordinate (search pick), once per change.
        func applyFocus(on map: MKMapView, focus: LatLng?) {
            guard let focus, focus != lastFocus else { return }
            lastFocus = focus
            let region = MKCoordinateRegion(
                center: CLLocationCoordinate2D(latitude: focus.lat, longitude: focus.lng),
                span: MKCoordinateSpan(latitudeDelta: 0.08, longitudeDelta: 0.08)
            )
            map.setRegion(region, animated: true)
        }

        public func mapView(_ mapView: MKMapView, didSelect view: MKAnnotationView) {
            if let pin = view.annotation as? PinAnnotation { onSelectPin?(pin.pin.id) }
        }

        public func mapView(_ mapView: MKMapView, regionDidChangeAnimated animated: Bool) {
            let c = mapView.centerCoordinate
            let width = mapView.bounds.width
            let metersPerPoint = width > 0
                ? mapView.visibleMapRect.size.width * MKMetersPerMapPointAtLatitude(c.latitude) / Double(width)
                : 0
            onRegionChange?(LatLng(lat: c.latitude, lng: c.longitude), metersPerPoint)
        }

        func installOverlay(on map: MKMapView, base: BaseLayer) {
            // Cache-first overlay: downloaded regions render offline, else network.
            let overlay = CachingTileOverlay(base: base)
            map.addOverlay(overlay, level: .aboveLabels)
            tileOverlay = overlay
            currentBase = base
        }

        func syncOverlay(on map: MKMapView, base: BaseLayer) {
            guard base != currentBase else { return }
            if let old = tileOverlay { map.removeOverlay(old) }
            installOverlay(on: map, base: base)
        }

        func syncAnnotations(on map: MKMapView, pins: [MapPin]) {
            let existing = map.annotations.compactMap { $0 as? PinAnnotation }
            let existingIds = Set(existing.map(\.pin.id))
            let incomingIds = Set(pins.map(\.id))

            let toRemove = existing.filter { !incomingIds.contains($0.pin.id) }
            if !toRemove.isEmpty { map.removeAnnotations(toRemove) }

            let toAdd = pins.filter { !existingIds.contains($0.id) }.map(PinAnnotation.init)
            if !toAdd.isEmpty { map.addAnnotations(toAdd) }
        }

        @objc func handleLongPress(_ gesture: UILongPressGestureRecognizer) {
            guard gesture.state == .began, let map = gesture.view as? MKMapView else { return }
            let point = gesture.location(in: map)
            let coord = map.convert(point, toCoordinateFrom: map)
            onLongPress?(LatLng(lat: coord.latitude, lng: coord.longitude))
        }

        public func mapView(_ mapView: MKMapView, rendererFor overlay: MKOverlay) -> MKOverlayRenderer {
            if let tile = overlay as? MKTileOverlay {
                return MKTileOverlayRenderer(tileOverlay: tile)
            }
            if let line = overlay as? MKPolyline {
                let renderer = MKPolylineRenderer(polyline: line)
                renderer.strokeColor = UIColor(red: 0.04, green: 0.52, blue: 1.0, alpha: 0.9)  // system blue
                renderer.lineWidth = 5
                renderer.lineJoin = .round
                renderer.lineCap = .round
                return renderer
            }
            return MKOverlayRenderer(overlay: overlay)
        }

        public func mapView(_ mapView: MKMapView, viewFor annotation: MKAnnotation) -> MKAnnotationView? {
            guard let pinAnnotation = annotation as? PinAnnotation else { return nil }
            let id = "turbo-pin"
            let view = (mapView.dequeueReusableAnnotationView(withIdentifier: id) as? MKMarkerAnnotationView)
                ?? MKMarkerAnnotationView(annotation: pinAnnotation, reuseIdentifier: id)
            view.annotation = pinAnnotation
            view.markerTintColor = UIColor(pinAnnotation.pin.tint)
            view.glyphImage = UIImage(systemName: pinAnnotation.pin.symbolName)
            view.canShowCallout = false
            // Make the pin discoverable to UI tests / VoiceOver by its name.
            view.isAccessibilityElement = true
            view.accessibilityLabel = pinAnnotation.pin.title
            return view
        }
    }

    /// Bridges a ``MapPin`` to MapKit's `MKAnnotation`.
    final class PinAnnotation: NSObject, MKAnnotation {
        let pin: MapPin
        var coordinate: CLLocationCoordinate2D {
            CLLocationCoordinate2D(latitude: pin.coordinate.lat, longitude: pin.coordinate.lng)
        }
        var title: String? { pin.title }
        init(_ pin: MapPin) { self.pin = pin }
    }
}

#else

/// Non-UIKit fallback so the package builds & unit-tests on the macOS host.
/// The real map only ships on iOS.
public struct TurboMapView: View {
    public init(
        baseLayer: BaseLayer,
        overlays: Set<OverlayId> = [],
        pins: [MapPin],
        following: Bool = false,
        focus: LatLng? = nil,
        resetBearingToken: Int = 0,
        routeGeometry: [LatLng] = [],
        onLongPress: ((LatLng) -> Void)? = nil,
        onRegionChange: ((LatLng, Double) -> Void)? = nil,
        onSelectPin: ((String) -> Void)? = nil,
        onTap: ((LatLng) -> Void)? = nil
    ) {}

    public var body: some View {
        Rectangle().fill(Color(white: 0.92))
    }
}

#endif
