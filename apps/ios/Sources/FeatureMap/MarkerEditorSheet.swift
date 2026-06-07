import SwiftUI
import CoreModel
import CoreDesignSystem

/// The new-marker / edit-marker sheet. Name, activity kind, notes — backed by
/// ``MarkerEditorViewModel``. Mirrors `NewMarker` / `MarkerEditor` (design).
public struct MarkerEditorSheet: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss
    @State private var viewModel: MarkerEditorViewModel

    public init(viewModel: MarkerEditorViewModel) {
        _viewModel = State(initialValue: viewModel)
    }

    public var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Name", text: $viewModel.name, prompt: Text(viewModel.kind.label))
                        .accessibilityIdentifier("editor.name")
                }

                Section("Activity") {
                    kindGrid
                }

                Section("Notes") {
                    TextField("Notes", text: $viewModel.notes, axis: .vertical)
                        .lineLimit(3, reservesSpace: true)
                }

                if viewModel.isEditing {
                    Section {
                        Button(role: .destructive) {
                            viewModel.delete(); dismiss()
                        } label: {
                            Label("Delete Marker", systemImage: "trash")
                        }
                    }
                }
            }
            .navigationTitle(viewModel.isEditing ? "Edit Marker" : "New Marker")
            .toolbarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { viewModel.save(); dismiss() }
                        .accessibilityIdentifier("editor.save")
                }
            }
        }
    }

    private var kindGrid: some View {
        LazyVGrid(columns: Array(repeating: GridItem(.flexible()), count: 5), spacing: 14) {
            ForEach(ActivityKindId.allCases, id: \.self) { kind in
                let selected = kind == viewModel.kind
                Button { viewModel.kind = kind } label: {
                    VStack(spacing: 4) {
                        Glyph(symbol: kind.symbolName, color: kind.tint(t), size: 40, cornerRadius: 11)
                            .overlay(
                                RoundedRectangle(cornerRadius: 11, style: .continuous)
                                    .stroke(t.blue, lineWidth: selected ? 3 : 0)
                            )
                    }
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.vertical, 4)
    }
}
