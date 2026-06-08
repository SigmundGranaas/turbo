import Foundation
import Observation
import CoreAuth

/// Drives sign-in and exposes the current ``AuthState``. Mirrors
/// `feature.auth.AuthViewModel` (Android).
@MainActor
@Observable
public final class AuthViewModel {
    public private(set) var state: AuthState = .signedOut
    public private(set) var isWorking = false
    /// Set when a sign-in attempt fails so the screen can surface it; cleared on
    /// the next attempt or via ``dismissError()``.
    public var errorMessage: String?

    private let repository: AuthRepository
    private var observation: Task<Void, Never>?

    public init(repository: AuthRepository) {
        self.repository = repository
    }

    public func start() {
        guard observation == nil else { return }
        observation = Task { [weak self, repository] in
            for await value in await repository.state() {
                self?.state = value
            }
        }
    }

    public func stop() { observation?.cancel(); observation = nil }

    public func signIn() {
        isWorking = true
        errorMessage = nil
        Task { [repository] in
            let outcome = await repository.signIn()
            self.isWorking = false
            // A successful sign-in flips `state` via the observation above; only a
            // genuine failure needs surfacing here.
            if case .failure = outcome, self.state.account == nil {
                self.errorMessage = "Sign-in didn't complete. Please try again."
            }
        }
    }

    public func dismissError() { errorMessage = nil }

    public func signOut() {
        Task { [repository] in await repository.signOut() }
    }
}
