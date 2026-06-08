import SwiftUI
import CoreModel
import CoreDesignSystem

/// The user's collections (folders of markers/tracks). Mirrors
/// `feature.collections.CollectionsScreen` (Android) + the M3 collections design.
public struct CollectionsScreen: View {
    @Environment(\.turbo) private var t
    @State private var viewModel: CollectionsViewModel
    @State private var showNew = false
    @State private var newName = ""

    public init(viewModel: CollectionsViewModel) {
        _viewModel = State(initialValue: viewModel)
    }

    public var body: some View {
        List {
            if viewModel.collections.isEmpty {
                ContentUnavailableView(
                    "No Collections",
                    systemImage: "folder",
                    description: Text("Tap + to group markers and tracks into a collection. They sync across your devices.")
                )
            }
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
        .navigationTitle("Collections")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button { newName = ""; showNew = true } label: { Image(systemName: "plus") }
                    .accessibilityLabel("New collection")
                    .accessibilityIdentifier("collections.new")
            }
        }
        .alert("New Collection", isPresented: $showNew) {
            TextField("Name", text: $newName)
            Button("Create") { viewModel.create(name: newName) }
            Button("Cancel", role: .cancel) {}
        }
        .task { viewModel.start() }
    }

    private func color(for collection: MapCollection) -> Color {
        let palette = [t.blue, t.green, t.orange, t.purple, t.teal, t.pink]
        let index = abs(collection.id.hashValue) % palette.count
        return palette[index]
    }
}
