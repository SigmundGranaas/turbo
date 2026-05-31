using System.Text.Json;
using Microsoft.EntityFrameworkCore;
using Turboapi.Activities.data.model;
using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.data;

/// <summary>
/// Postgres-backed <see cref="IActivityObservationStore"/>. Reads are
/// no-tracking and index-anchored on
/// (activity_id, observed_at desc), (user_id, kind, observed_at desc), and
/// (watershed_href_id, observed_at desc).
/// </summary>
public sealed class PgActivityObservationStore : IActivityObservationStore
{
    private readonly ActivitySummariesContext _db;

    public PgActivityObservationStore(ActivitySummariesContext db) => _db = db;

    public async Task WriteAsync(ActivityObservation observation, CancellationToken cancellationToken)
    {
        var payload = JsonSerializer.SerializeToDocument(observation.KindPayload);
        _db.ActivityObservations.Add(new ActivityObservationEntity
        {
            Id = observation.Id == Guid.Empty ? Guid.NewGuid() : observation.Id,
            ActivityId = observation.ActivityId,
            UserId = observation.UserId,
            ObservedAt = observation.ObservedAt.UtcDateTime,
            Kind = observation.Kind,
            Rating = observation.Rating,
            Comment = observation.Comment,
            KindPayload = payload,
            PhotoCount = observation.PhotoCount,
            CreatedAt = observation.CreatedAt == default ? DateTime.UtcNow : observation.CreatedAt,
        });
        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task<IReadOnlyList<ActivityObservation>> GetForActivityAsync(
        Guid activityId, DateTimeOffset since, int limit, CancellationToken cancellationToken)
    {
        var sinceUtc = since.UtcDateTime;
        var rows = await _db.ActivityObservations.AsNoTracking()
            .Where(o => o.ActivityId == activityId && o.ObservedAt >= sinceUtc)
            .OrderByDescending(o => o.ObservedAt)
            .Take(limit)
            .ToListAsync(cancellationToken);
        return rows.Select(ToDomain).ToArray();
    }

    public async Task<IReadOnlyList<ActivityObservation>> GetForWatershedAsync(
        string watershedHrefId, DateTimeOffset since, int limit, CancellationToken cancellationToken)
    {
        var sinceUtc = since.UtcDateTime;
        var rows = await _db.ActivityObservations.AsNoTracking()
            .Where(o => o.WatershedHrefId == watershedHrefId && o.ObservedAt >= sinceUtc)
            .OrderByDescending(o => o.ObservedAt)
            .Take(limit)
            .ToListAsync(cancellationToken);
        return rows.Select(ToDomain).ToArray();
    }

    public async Task<ActivityObservation?> GetByIdAsync(Guid id, CancellationToken cancellationToken)
    {
        var row = await _db.ActivityObservations.AsNoTracking()
            .FirstOrDefaultAsync(o => o.Id == id, cancellationToken);
        return row is null ? null : ToDomain(row);
    }

    private static ActivityObservation ToDomain(ActivityObservationEntity row) =>
        new(
            Id: row.Id,
            ActivityId: row.ActivityId,
            UserId: row.UserId,
            ObservedAt: new DateTimeOffset(DateTime.SpecifyKind(row.ObservedAt, DateTimeKind.Utc)),
            Kind: row.Kind,
            Rating: row.Rating,
            Comment: row.Comment,
            KindPayload: row.KindPayload.RootElement.Clone(),
            PhotoCount: row.PhotoCount,
            CreatedAt: DateTime.SpecifyKind(row.CreatedAt, DateTimeKind.Utc));
}

public sealed class PgActivityVisitStore : IActivityVisitStore
{
    private readonly ActivitySummariesContext _db;

    public PgActivityVisitStore(ActivitySummariesContext db) => _db = db;

    public async Task WriteAsync(ActivityVisit visit, CancellationToken cancellationToken)
    {
        _db.ActivityVisits.Add(new ActivityVisitEntity
        {
            Id = visit.Id == Guid.Empty ? Guid.NewGuid() : visit.Id,
            ActivityId = visit.ActivityId,
            UserId = visit.UserId,
            VisitedAt = visit.VisitedAt.UtcDateTime,
            Source = visit.Source,
            CreatedAt = DateTime.UtcNow,
        });
        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task<IReadOnlyList<ActivityVisit>> GetForUserAsync(
        Guid userId, Guid activityId, DateTimeOffset since, int limit, CancellationToken cancellationToken)
    {
        var sinceUtc = since.UtcDateTime;
        var rows = await _db.ActivityVisits.AsNoTracking()
            .Where(v => v.UserId == userId && v.ActivityId == activityId && v.VisitedAt >= sinceUtc)
            .OrderByDescending(v => v.VisitedAt)
            .Take(limit)
            .ToListAsync(cancellationToken);
        return rows.Select(r => new ActivityVisit(
            Id: r.Id,
            ActivityId: r.ActivityId,
            UserId: r.UserId,
            VisitedAt: new DateTimeOffset(DateTime.SpecifyKind(r.VisitedAt, DateTimeKind.Utc)),
            Source: r.Source)).ToArray();
    }
}
