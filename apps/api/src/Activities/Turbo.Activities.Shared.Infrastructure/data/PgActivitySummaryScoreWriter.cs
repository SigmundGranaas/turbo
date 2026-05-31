using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.data;

/// <summary>
/// Postgres-backed score writer. UPDATE rather than UPSERT — we only
/// ever score activities that already have a summary row; if the row
/// hasn't been projected yet (race against the upserter on a fresh
/// activity), the update is a no-op and the next analysis fetch retries.
/// </summary>
public sealed class PgActivitySummaryScoreWriter : IActivitySummaryScoreWriter
{
    private const int TopDriverLabelMaxLen = 64;

    private readonly ActivitySummariesContext _db;
    private readonly ILogger<PgActivitySummaryScoreWriter> _logger;

    public PgActivitySummaryScoreWriter(
        ActivitySummariesContext db,
        ILogger<PgActivitySummaryScoreWriter> logger)
    {
        _db = db;
        _logger = logger;
    }

    public async Task WriteAsync(
        Guid activityId,
        int? score,
        string? topDriverLabel,
        DateTimeOffset writtenAt,
        CancellationToken cancellationToken)
    {
        try
        {
            var row = await _db.Summaries
                .FirstOrDefaultAsync(s => s.Id == activityId, cancellationToken);
            if (row is null) return;
            row.SummaryScore = score;
            row.SummaryScoreAt = writtenAt.UtcDateTime;
            row.TopDriverLabel = topDriverLabel is null
                ? null
                : topDriverLabel.Length > TopDriverLabelMaxLen
                    ? topDriverLabel[..TopDriverLabelMaxLen]
                    : topDriverLabel;
            await _db.SaveChangesAsync(cancellationToken);
        }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "Score write-back failed for {ActivityId}", activityId);
        }
    }
}
