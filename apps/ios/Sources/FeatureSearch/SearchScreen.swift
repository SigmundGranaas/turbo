import SwiftUI
import CoreModel
import CoreDesignSystem

/// Place search — top results while typing, recent picks when empty. Picking a
/// result hands its coordinate back to the map. Mirrors `MapSearch` (design) /
/// `feature.search.SearchScreen` (Android).
public struct SearchScreen: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss
    @State private var viewModel: SearchViewModel
    private let onPick: (String, LatLng) -> Void

    public init(viewModel: SearchViewModel, onPick: @escaping (String, LatLng) -> Void) {
        _viewModel = State(initialValue: viewModel)
        self.onPick = onPick
    }

    public var body: some View {
        NavigationStack {
            List {
                if viewModel.query.isEmpty {
                    Section("Recents") {
                        ForEach(viewModel.recents) { recent in
                            Button { pick(recent.name, recent.position) } label: {
                                ResultRow(symbol: "clock", tint: t.gray, title: recent.name, subtitle: recent.sub)
                            }
                        }
                    }
                } else {
                    Section("Top Results") {
                        if viewModel.isSearching {
                            HStack(spacing: 10) { ProgressView(); Text("Searching…").foregroundStyle(t.label2) }
                        } else if viewModel.results.isEmpty {
                            Text("No results found.").foregroundStyle(t.label2)
                        }
                        ForEach(viewModel.results) { hit in
                            Button { viewModel.remember(hit); pick(hit.name, hit.position) } label: {
                                ResultRow(
                                    symbol: hit.kind?.symbolName ?? "mappin",
                                    tint: hit.kind?.tint(t) ?? t.blue,
                                    title: hit.name,
                                    subtitle: hit.description
                                )
                            }
                        }
                    }
                }
            }
            .navigationTitle("Search")
            .toolbarTitleDisplayMode(.inline)
            .searchable(text: $viewModel.query, placement: searchPlacement, prompt: "Search Turbo")
            .onChange(of: viewModel.query) { viewModel.runSearch() }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
            .task { viewModel.start() }
        }
    }

    private func pick(_ name: String, _ position: LatLng) {
        onPick(name, position)
        dismiss()
    }

    private var searchPlacement: SearchFieldPlacement {
        #if os(iOS)
        .navigationBarDrawer(displayMode: .always)
        #else
        .automatic
        #endif
    }
}

private struct ResultRow: View {
    @Environment(\.turbo) private var t
    let symbol: String
    let tint: Color
    let title: String
    let subtitle: String

    var body: some View {
        HStack(spacing: 12) {
            Glyph(symbol: symbol, color: tint, size: 32, cornerRadius: 8)
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.turboBody).foregroundStyle(t.label)
                Text(subtitle).font(.turboFootnote).foregroundStyle(t.label2)
            }
            Spacer()
        }
    }
}
