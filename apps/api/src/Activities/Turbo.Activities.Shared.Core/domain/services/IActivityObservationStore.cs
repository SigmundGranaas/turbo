using System.Text.Json;

namespace Turboapi.Activities.domain.services;

/// <summary>
/// User-contributed observation log. Each row is one user's report after
/// visiting an activity — kind-specific structured payload (freediving:
/// observed visibility; xc ski: track condition encountered; backcountry:
/// snow quality + any concerns) plus a free-text comment and an
/// optional rating. Orchestrators read recent observations (typically
/// last 14 days) as a <c>RecentObservations</c> driver, calibrating model
/// outputs against ground truth users have already provided.
/// </summary>
public interface IActivityObservationStore
{
    Task WriteAsync(ActivityObservation observation, CancellationToken cancellationToken);

    Task<IReadOnlyList<ActivityObservation>> GetForActivityAsync(
        Guid activityId,
        DateTimeOffset since,
        int limit,
        CancellationToken cancellationToken);

    /// <summary>Observations within the same watershed (or arbitrary
    /// geographic correlate). Used for cross-activity signal sharing —
    /// e.g. a freediving orchestrator reading upstream packrafting
    /// observations to estimate runoff load on visibility. The
    /// orchestrator resolves the correlate key from
    /// <see cref="ActivityGeoContext"/> before calling.</summary>
    Task<IReadOnlyList<ActivityObservation>> GetForWatershedAsync(
        string watershedHrefId,
        DateTimeOffset since,
        int limit,
        CancellationToken cancellationToken);

    Task<ActivityObservation?> GetByIdAsync(Guid id, CancellationToken cancellationToken);
}

public sealed record ActivityObservation(
    Guid Id,
    Guid ActivityId,
    Guid UserId,
    DateTimeOffset ObservedAt,
    string Kind,
    short? Rating,
    string? Comment,
    JsonElement KindPayload,
    short PhotoCount,
    DateTime CreatedAt);

/// <summary>
/// Lightweight "user was at this activity at time T" log. Lighter than
/// <see cref="IActivityObservationStore"/> because there's no structured
/// payload — just presence. Powers personalized messaging
/// ("you've been here in similar conditions") and crude rate-limiting
/// of repeat observations.
/// </summary>
public interface IActivityVisitStore
{
    Task WriteAsync(ActivityVisit visit, CancellationToken cancellationToken);

    Task<IReadOnlyList<ActivityVisit>> GetForUserAsync(
        Guid userId,
        Guid activityId,
        DateTimeOffset since,
        int limit,
        CancellationToken cancellationToken);
}

public sealed record ActivityVisit(
    Guid Id,
    Guid ActivityId,
    Guid UserId,
    DateTimeOffset VisitedAt,
    string Source);
