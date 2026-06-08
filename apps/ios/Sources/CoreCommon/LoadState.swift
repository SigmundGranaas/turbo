import Foundation

/// The state of a single async load, so every screen renders Loading / Empty /
/// Error / Content the same way instead of each view model inventing its own
/// `loaded` bool + optional value (which conflates "still loading" with "failed").
/// The UI counterpart is `AsyncContent` in CoreDesignSystem.
public enum LoadState<Value: Sendable>: Sendable {
    case idle
    case loading
    case loaded(Value)
    /// Loaded successfully, but there's nothing to show (e.g. no search results).
    case empty
    /// Failed — carries a user-facing message.
    case failed(String)

    public var value: Value? {
        if case .loaded(let v) = self { return v }
        return nil
    }

    public var isLoading: Bool {
        if case .loading = self { return true }
        return false
    }

    public var errorMessage: String? {
        if case .failed(let message) = self { return message }
        return nil
    }
}

public extension LoadState {
    /// Map a successful optional into `.loaded` or `.failed(message)` — for
    /// single-value loads where `nil` means the fetch didn't produce a result.
    static func resolve(_ value: Value?, failure: String) -> LoadState {
        value.map(LoadState.loaded) ?? .failed(failure)
    }
}

public extension LoadState where Value: Collection {
    /// Map a collection into `.empty` (nothing) or `.loaded` (has items).
    static func resolve(_ collection: Value) -> LoadState {
        collection.isEmpty ? .empty : .loaded(collection)
    }
}
