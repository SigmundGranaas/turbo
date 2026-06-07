import Foundation

/// A lightweight success/failure result for repository + use-case boundaries,
/// so the UI can render Loading/Content/Error without exceptions leaking up.
///
/// Mirrors `core.common.Outcome` in the Android app. We keep a bespoke type
/// (rather than Swift's `Result`) so both clients read the same way.
public enum Outcome<T> {
    case success(T)
    case failure(Error)

    public func getOrNil() -> T? {
        switch self {
        case .success(let value): value
        case .failure: nil
        }
    }

    public func map<R>(_ transform: (T) -> R) -> Outcome<R> {
        switch self {
        case .success(let value): .success(transform(value))
        case .failure(let error): .failure(error)
        }
    }

    /// Run `block`, capturing any thrown error as a `.failure`.
    public static func catching(_ block: () throws -> T) -> Outcome<T> {
        do {
            return .success(try block())
        } catch {
            return .failure(error)
        }
    }

    /// Async variant of ``catching(_:)`` for `await`-ing repositories.
    public static func catching(_ block: () async throws -> T) async -> Outcome<T> {
        do {
            return .success(try await block())
        } catch {
            return .failure(error)
        }
    }
}

extension Outcome: Sendable where T: Sendable {}
