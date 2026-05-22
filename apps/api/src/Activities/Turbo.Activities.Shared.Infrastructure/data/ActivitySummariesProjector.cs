using System.Diagnostics;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.data.model;
using Turboapi.Activities.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.data;

/// <summary>
/// Read-model subscriber for the shared cross-kind summaries projection.
/// Listens for <see cref="ActivitySummaryUpserted"/> and
/// <see cref="ActivitySummaryDeleted"/> events emitted by any kind module's
/// write path, and upserts / tombstones a single row in
/// <c>activities.activity_summaries</c>. Idempotent via the
/// processed-events table — at-least-once redeliveries are safe.
/// </summary>
public sealed class ActivitySummaryUpsertedHandler : IEventHandler<ActivitySummaryUpserted>
{
    private readonly ActivitySummariesContext _db;
    private readonly IIdempotencyStore<ActivitySummariesContext> _idempotency;
    private readonly ILogger<ActivitySummaryUpsertedHandler> _logger;
    private readonly ActivitySource _activitySource;

    public ActivitySummaryUpsertedHandler(
        ActivitySummariesContext db,
        IIdempotencyStore<ActivitySummariesContext> idempotency,
        ILogger<ActivitySummaryUpsertedHandler> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("ActivitySummaryUpsertedHandler");
    }

    public async Task HandleAsync(ActivitySummaryUpserted @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle ActivitySummaryUpserted");
        activity?.SetTag("activity.id", @event.ActivityId);
        activity?.SetTag("activity.kind", @event.Kind);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed ActivitySummaryUpserted {EventId}", @event.EventId);
            return;
        }

        var existing = await _db.Summaries.FirstOrDefaultAsync(
            s => s.Id == @event.ActivityId, cancellationToken);

        var geom = ParseWkt(@event.Geometry);

        if (existing is null)
        {
            _db.Summaries.Add(new ActivitySummaryEntity
            {
                Id = @event.ActivityId,
                OwnerId = @event.OwnerId,
                Kind = @event.Kind,
                Name = @event.Name,
                Geometry = geom,
                IconKey = @event.IconKey,
                ColorHex = @event.ColorHex,
                CreatedAt = @event.OccurredAt,
                UpdatedAt = @event.OccurredAt,
                DeletedAt = null,
                Version = @event.Version,
            });
        }
        else
        {
            if (@event.Version < existing.Version)
            {
                _logger.LogDebug("Stale summary upsert for {Id}: event v={EventVersion} row v={RowVersion}",
                    @event.ActivityId, @event.Version, existing.Version);
                return;
            }
            existing.Name = @event.Name;
            existing.Geometry = geom;
            existing.IconKey = @event.IconKey;
            existing.ColorHex = @event.ColorHex;
            existing.UpdatedAt = @event.OccurredAt;
            existing.Version = @event.Version;
            existing.DeletedAt = null;
        }

        await _db.SaveChangesAsync(cancellationToken);
    }

    private static Geometry ParseWkt(ActivityGeometryWkt wkt)
    {
        var reader = new WKTReader();
        var g = reader.Read(wkt.Wkt);
        if (g.SRID != 4326) g.SRID = 4326;
        return g;
    }
}

public sealed class ActivitySummaryDeletedHandler : IEventHandler<ActivitySummaryDeleted>
{
    private readonly ActivitySummariesContext _db;
    private readonly IIdempotencyStore<ActivitySummariesContext> _idempotency;
    private readonly ILogger<ActivitySummaryDeletedHandler> _logger;
    private readonly ActivitySource _activitySource;

    public ActivitySummaryDeletedHandler(
        ActivitySummariesContext db,
        IIdempotencyStore<ActivitySummariesContext> idempotency,
        ILogger<ActivitySummaryDeletedHandler> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("ActivitySummaryDeletedHandler");
    }

    public async Task HandleAsync(ActivitySummaryDeleted @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle ActivitySummaryDeleted");
        activity?.SetTag("activity.id", @event.ActivityId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed ActivitySummaryDeleted {EventId}", @event.EventId);
            return;
        }

        var row = await _db.Summaries.FirstOrDefaultAsync(s => s.Id == @event.ActivityId, cancellationToken);
        if (row is null)
        {
            _logger.LogDebug("Tombstone for unknown summary {Id} — projection may not have caught up", @event.ActivityId);
            return;
        }
        row.DeletedAt = @event.OccurredAt;
        row.UpdatedAt = @event.OccurredAt;
        row.Version += 1;
        await _db.SaveChangesAsync(cancellationToken);
    }
}
