import Foundation

/// A small actor that holds a value and fans every change out to all subscribers
/// — the Swift-concurrency analogue of Android's `MutableStateFlow`. Repositories
/// wrap one of these to expose a reactive snapshot + stream without re-deriving
/// the continuation bookkeeping each time.
public actor ReactiveStore<Value: Sendable> {
    private var value: Value
    private var continuations: [UUID: AsyncStream<Value>.Continuation] = [:]

    public init(_ initial: Value) { value = initial }

    /// The latest value.
    public func current() -> Value { value }

    /// A stream that immediately yields the current value, then every change.
    public func stream() -> AsyncStream<Value> {
        AsyncStream { continuation in
            let key = UUID()
            continuations[key] = continuation
            continuation.yield(value)
            continuation.onTermination = { [weak self] _ in
                Task { await self?.remove(key) }
            }
        }
    }

    /// Replace the value and notify subscribers.
    public func set(_ newValue: Value) {
        value = newValue
        emit()
    }

    /// Mutate the value in place and notify subscribers.
    public func update(_ transform: (Value) -> Value) {
        value = transform(value)
        emit()
    }

    private func emit() {
        for continuation in continuations.values { continuation.yield(value) }
    }

    private func remove(_ key: UUID) { continuations[key] = nil }
}
