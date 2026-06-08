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

                Section("Colour") {
                    colorRow
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

    private var colorRow: some View {
        HStack(spacing: 14) {
            colorSwatch(nil, fill: viewModel.kind.tint(t))   // "default" = kind tint
            ForEach(PathPalette.swatches, id: \.argb) { swatch in
                colorSwatch(swatch.argb, fill: Color(argb: swatch.argb))
            }
        }
        .padding(.vertical, 4)
    }

    private func colorSwatch(_ argb: Int64?, fill: Color) -> some View {
        let selected = viewModel.colorArgb == argb
        return Button { viewModel.colorArgb = argb } label: {
            Circle()
                .fill(fill)
                .frame(width: 30, height: 30)
                .overlay(Circle().stroke(t.blue, lineWidth: selected ? 3 : 0))
                .overlay(argb == nil ? Image(systemName: "sparkles").font(.system(size: 12)).foregroundStyle(.white) : nil)
        }
        .buttonStyle(.plain)
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
