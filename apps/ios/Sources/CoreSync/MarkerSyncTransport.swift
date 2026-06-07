import Foundation
import CoreCommon

/// The server side of marker sync — pull the current remote set, push local
/// changes. Mirrors `core.sync.SyncHttp` + `MarkerSync`. The HTTP implementation
/// talks to apps/api; the in-memory one is a server replica for tests.
public protocol MarkerSyncTransport: Sendable {
    func pull() async throws -> [SyncRecord<MarkerPayload>]
    func push(_ records: [SyncRecord<MarkerPayload>]) async throws
}

/// An in-memory "server" — applies pushes with last-write-wins, serves pulls.
/// Used by tests and as an offline stand-in.
public actor InMemoryMarkerSyncTransport: MarkerSyncTransport {
    private var store: [String: SyncRecord<MarkerPayload>]

    public init(seed: [SyncRecord<MarkerPayload>] = []) {
        store = Dictionary(seed.map { ($0.id, $0) }, uniquingKeysWith: { a, b in a.updatedAt >= b.updatedAt ? a : b })
    }

    public func pull() -> [SyncRecord<MarkerPayload>] { Array(store.values) }

    public func push(_ records: [SyncRecord<MarkerPayload>]) {
        for record in records {
            if let existing = store[record.id], existing.updatedAt >= record.updatedAt { continue }
            store[record.id] = record
        }
    }
}

/// HTTP transport against the Turbo API. Bearer-token auth is supplied per call;
/// JSON bodies match the Android `SyncDtos`. (Network path — exercised at runtime,
/// not in unit tests.)
public struct HttpMarkerSyncTransport: MarkerSyncTransport {
    private let baseURL: URL
    private let token: @Sendable () async -> String?
    private let session: URLSession

    public init(baseURL: URL, token: @escaping @Sendable () async -> String?, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.token = token
        self.session = session
    }

    public func pull() async throws -> [SyncRecord<MarkerPayload>] {
        var request = URLRequest(url: baseURL.appendingPathComponent("markers"))
        await authorize(&request)
        let (data, _) = try await session.data(for: request)
        return try JSONDecoder().decode([SyncRecord<MarkerPayload>].self, from: data)
    }

    public func push(_ records: [SyncRecord<MarkerPayload>]) async throws {
        guard !records.isEmpty else { return }
        var request = URLRequest(url: baseURL.appendingPathComponent("markers"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        await authorize(&request)
        request.httpBody = try JSONEncoder().encode(records)
        _ = try await session.data(for: request)
    }

    private func authorize(_ request: inout URLRequest) async {
        if let token = await token() {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
    }
}
