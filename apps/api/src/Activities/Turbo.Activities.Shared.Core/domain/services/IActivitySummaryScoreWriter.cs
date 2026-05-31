namespace Turboapi.Activities.domain.services;

/// <summary>
/// Writes the latest analysis score back into the cross-kind summary
/// projection. The pipeline calls this at the end of every successful
/// <c>RunAsync</c> so the map layer's score halos (and the recommendation
/// endpoint's quick filter) see fresh values without doing a per-pin
/// analysis fetch.
///
/// Soft-fails on write — the user already has their analysis result; we
/// don't want a stale-projection write blowing up the response.
/// </summary>
public interface IActivitySummaryScoreWriter
{
    Task WriteAsync(
        Guid activityId,
        int? score,
        string? topDriverLabel,
        DateTimeOffset writtenAt,
        CancellationToken cancellationToken);
}
