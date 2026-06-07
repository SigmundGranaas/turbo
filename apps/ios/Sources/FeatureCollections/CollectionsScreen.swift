import SwiftUI
import CoreModel
import CoreDesignSystem

/// The user's collections (folders of markers/tracks). Mirrors
/// `feature.collections.CollectionsScreen` (Android) + the M3 collections design.
public struct CollectionsScreen: View {
    @Environment(\.turbo) private var t
    @State private var viewModel: CollectionsViewModel

    public init(viewModel: CollectionsViewModel) {
        _viewModel = State(initialValue: viewModel)
    }

    public var body: some View {
        List {
            Section {
                ForEach(viewModel.collections) { collection in
                    HStack(spacing: 13) {
                        Glyph(symbol: collection.icon ?? "folder.fill", color: color(for: collection), size: 38, cornerRadius: 10)
                        Text(collection.name).font(.turboBody).foregroundStyle(t.label)
                        Spacer()
                        Text("\(collection.itemCount)").font(.turboBody).foregroundStyle(t.label2)
                    }
                    .swipeActions {
                        Button(role: .destructive) { viewModel.delete(id: collection.id) } label: {
                            Label("Delete", systemImage: "trash")
                        }
                    }
                }
            }

            Section {
                Label("New Collection", systemImage: "plus")
                    .foregroundStyle(t.blue)
            }
        }
        .navigationTitle("Collections")
        .task { viewModel.start() }
    }

    private func color(for collection: MapCollection) -> Color {
        let palette = [t.blue, t.green, t.orange, t.purple, t.teal, t.pink]
        let index = abs(collection.id.hashValue) % palette.count
        return palette[index]
    }
}
