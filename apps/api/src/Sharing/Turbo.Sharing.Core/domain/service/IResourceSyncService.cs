namespace Turboapi.Sharing.domain.service;

/// <summary>
/// Unified delta sync. Returns resources visible to a user — owned or
/// reachable via a user/group grant — that have changed since the cursor.
/// The Sharing service intentionally returns envelopes only; payload
/// modules expose their own GET-by-id endpoints which clients call for
/// the typed body. This keeps Sharing free of compile-time dependencies
/// on Collections / Markers / Paths.
/// </summary>
public interface IResourceSyncService
{
    Task<ResourceSyncPage> SyncAsync(
        Guid userId,
        DateTime? since,
        IReadOnlyCollection<string>? types,
        int limit,
        CancellationToken cancellationToken = default);
}

public sealed record ResourceSyncPage(
    IReadOnlyList<ResourceEnvelopeDto> Items,
    DateTime ServerTime);

public sealed record ResourceEnvelopeDto(
    Guid Id,
    string Type,
    Guid OwnerId,
    string Visibility,
    string MyRole,
    long Version,
    DateTime UpdatedAt,
    bool Deleted);
