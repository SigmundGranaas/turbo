import SwiftUI
import CoreModel
import CoreDesignSystem

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

    public init(marker: Marker, onEdit: (() -> Void)? = nil, onDelete: @escaping () -> Void) {
        self.marker = marker
        self.onEdit = onEdit
        self.onDelete = onDelete
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
                    action("Delete", "trash", role: .destructive) { confirmingDelete = true }
                }

                infoRow("Coordinate", Geo.formatCoords(marker.position))
                if let notes = marker.notes, !notes.isEmpty {
                    infoRow("Notes", notes)
                }
            }
            .padding(16)
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
