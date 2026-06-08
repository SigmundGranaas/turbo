import SwiftUI
import CoreCommon

/// Renders a ``LoadState`` consistently — a spinner while loading, a standard
/// unavailable view for empty / failed, and the caller's content once loaded.
/// One place for the Loading/Empty/Error treatment so every async screen agrees.
public struct AsyncContent<Value: Sendable, Content: View>: View {
    private let state: LoadState<Value>
    private let emptyTitle: String
    private let emptySymbol: String
    private let emptyMessage: String?
    private let content: (Value) -> Content

    public init(
        _ state: LoadState<Value>,
        emptyTitle: String = "Nothing Here",
        emptySymbol: String = "tray",
        emptyMessage: String? = nil,
        @ViewBuilder content: @escaping (Value) -> Content
    ) {
        self.state = state
        self.emptyTitle = emptyTitle
        self.emptySymbol = emptySymbol
        self.emptyMessage = emptyMessage
        self.content = content
    }

    public var body: some View {
        switch state {
        case .idle, .loading:
            ProgressView()
                .frame(maxWidth: .infinity)
                .padding(.vertical, 40)
        case .loaded(let value):
            content(value)
        case .empty:
            unavailable(emptyTitle, emptySymbol, emptyMessage)
        case .failed(let message):
            unavailable("Something Went Wrong", "exclamationmark.triangle", message)
        }
    }

    @ViewBuilder
    private func unavailable(_ title: String, _ symbol: String, _ message: String?) -> some View {
        if let message {
            ContentUnavailableView(title, systemImage: symbol, description: Text(message))
        } else {
            ContentUnavailableView(title, systemImage: symbol)
        }
    }
}
