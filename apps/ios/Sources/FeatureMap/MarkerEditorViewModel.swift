import Foundation
import Observation
import CoreModel
import CoreData
import CoreDesignSystem

/// Backs the new-marker / edit-marker sheet: holds the editable fields and
/// persists through ``MarkerRepository``. Mirrors the marker create/edit flow in
/// Android's `feature.markers`.
@MainActor
@Observable
public final class MarkerEditorViewModel {
    public var name: String
    public var kind: ActivityKindId
    public var notes: String
    public let position: LatLng

    /// `nil` for a new marker; the existing id when editing.
    public let editingId: String?
    public var isEditing: Bool { editingId != nil }

    private let repository: MarkerRepository

    /// New marker dropped at `position`, optionally prefilled (e.g. from a search result).
    public init(repository: MarkerRepository, position: LatLng, kind: ActivityKindId = .mountain, name: String = "") {
        self.repository = repository
        self.position = position
        self.kind = kind
        self.name = name
        self.notes = ""
        self.editingId = nil
    }

    /// Edit an existing `marker`.
    public init(repository: MarkerRepository, marker: Marker) {
        self.repository = repository
        self.position = marker.position
        self.kind = marker.kind
        self.name = marker.name
        self.notes = marker.notes ?? ""
        self.editingId = marker.id
    }

    /// Persist the marker. A blank name falls back to the kind's label (matching
    /// the Android behaviour); editing reuses the id so the row is replaced.
    public func save() {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedNotes = notes.trimmingCharacters(in: .whitespacesAndNewlines)
        let marker = Marker(
            id: editingId ?? "m-\(UUID().uuidString)",
            name: trimmedName.isEmpty ? kind.label : trimmedName,
            kind: kind,
            position: position,
            notes: trimmedNotes.isEmpty ? nil : trimmedNotes
        )
        Task { [repository] in await repository.upsert(marker) }
    }

    public func delete() {
        guard let editingId else { return }
        Task { [repository] in await repository.delete(id: editingId) }
    }
}
