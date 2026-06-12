import SwiftUI
import CoreModel
import CoreDesignSystem
#if canImport(UIKit)
import PhotosUI
import UIKit
#endif

/// An Apple Maps-style place card for a saved marker — identity, coordinate,
/// notes, and actions (edit, export, delete). Used as a sheet from a tapped pin
/// and pushed from the markers list.
public struct MarkerDetailScreen: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss
    private let marker: Marker
    private let onEdit: (() -> Void)?
    private let onDelete: () -> Void
    @State private var confirmingDelete = false
    @State private var photos: MarkerPhotosViewModel?
    @State private var weather: WeatherViewModel?
    @State private var showWeather = false
    private let makePhotos: (() -> MarkerPhotosViewModel)?
    private let shareResource: ((String) async -> URL?)?
    private let makeWeather: ((LatLng) -> WeatherViewModel)?
    private let makeAvalanche: ((LatLng) -> AvalancheViewModel)?

    public init(
        marker: Marker,
        onEdit: (() -> Void)? = nil,
        onDelete: @escaping () -> Void,
        makePhotos: (() -> MarkerPhotosViewModel)? = nil,
        shareResource: ((String) async -> URL?)? = nil,
        makeWeather: ((LatLng) -> WeatherViewModel)? = nil,
        makeAvalanche: ((LatLng) -> AvalancheViewModel)? = nil
    ) {
        self.marker = marker
        self.onEdit = onEdit
        self.onDelete = onDelete
        self.makePhotos = makePhotos
        self.shareResource = shareResource
        self.makeWeather = makeWeather
        self.makeAvalanche = makeAvalanche
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 14) {
                    Glyph(symbol: marker.kind.symbolName, color: marker.displayColor(t), size: 56, cornerRadius: 14)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(marker.name).font(.turboTitle2).foregroundStyle(t.label)
                        Text(marker.kind.label).font(.turboSubhead).foregroundStyle(t.label2)
                    }
                }

                HStack(spacing: 10) {
                    if let onEdit {
                        action("Edit", "pencil") { onEdit(); dismiss() }
                    }
                    if let url = try? MarkerExport.writeTemporaryFile(marker) {
                        ShareLink(item: url) {
                            actionLabel("Export", "square.and.arrow.up")
                        }
                    }
                    if let shareResource {
                        ShareLinkButton(create: { await shareResource(marker.id) }) {
                            actionLabel("Share", "person.2")
                        }
                        .accessibilityIdentifier("marker.share")
                    }
                    action("Delete", "trash", role: .destructive) { confirmingDelete = true }
                }

                if makeWeather != nil { weatherCard }
                infoRow("Coordinate", Geo.formatCoords(marker.position))
                if let notes = marker.notes, !notes.isEmpty {
                    infoRow("Notes", notes)
                }
                photoSection
            }
            .padding(16)
        }
        .task {
            if photos == nil { photos = makePhotos?() }
            await photos?.load()
            if weather == nil, let makeWeather { weather = makeWeather(marker.position); await weather?.load() }
        }
        .background(t.grouped)
        .navigationTitle(marker.name)
        .toolbarTitleDisplayMode(.inline)
        .confirmationDialog("Delete Marker?", isPresented: $confirmingDelete, titleVisibility: .visible) {
            Button("Delete", role: .destructive) { onDelete(); dismiss() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This action is permanent and cannot be undone.")
        }
        .sheet(isPresented: $showWeather) {
            if let makeWeather, let makeAvalanche {
                WeatherDetailScreen(weather: makeWeather(marker.position), avalanche: makeAvalanche(marker.position))
            }
        }
    }

    /// A compact forecast for the marker's location; taps through to the full sheet.
    @ViewBuilder
    private var weatherCard: some View {
        Button { showWeather = true } label: {
            HStack(spacing: 12) {
                if let s = weather?.state.value {
                    Image(systemName: s.symbol.sfSymbol).font(.title2).foregroundStyle(t.blue).frame(width: 30)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(WeatherSummary.formatTemperature(s.temperatureC)).font(.turboHeadline).foregroundStyle(t.label)
                        Text(s.summary).font(.turboFootnote).foregroundStyle(t.label2).lineLimit(1)
                    }
                } else if weather?.state.isLoading ?? true {
                    ProgressView().frame(width: 30)
                    Text("Loading weather…").font(.turboSubhead).foregroundStyle(t.label2)
                } else {
                    Image(systemName: "cloud.slash").foregroundStyle(t.label3).frame(width: 30)
                    Text("Weather unavailable").font(.turboSubhead).foregroundStyle(t.label2)
                }
                Spacer()
                Image(systemName: "chevron.right").font(.system(size: 14, weight: .semibold)).foregroundStyle(t.label3)
            }
            .padding(14)
            .background(t.groupedCard, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("marker.weather")
    }

    @ViewBuilder
    private var photoSection: some View {
        #if canImport(UIKit)
        if let photos {
            VStack(alignment: .leading, spacing: 8) {
                Text("Photos").font(.turboFootnote).foregroundStyle(t.label2).textCase(.uppercase)
                PhotoStrip(viewModel: photos)
            }
        }
        #endif
    }

    private func action(_ title: String, _ symbol: String, role: ButtonRole? = nil, _ act: @escaping () -> Void) -> some View {
        Button(role: role, action: act) { actionLabel(title, symbol, danger: role == .destructive) }
            .accessibilityIdentifier("marker.\(title.lowercased())")
    }

    private func actionLabel(_ title: String, _ symbol: String, danger: Bool = false) -> some View {
        VStack(spacing: 5) {
            Image(systemName: symbol)
            Text(title).font(.turboFootnote)
        }
        .foregroundStyle(danger ? t.red : t.blue)
        .frame(maxWidth: .infinity, minHeight: 60)
        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
    }

    private func infoRow(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(.turboFootnote).foregroundStyle(t.label2).textCase(.uppercase)
            Text(value).font(.turboBody).foregroundStyle(t.label)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
    }
}
