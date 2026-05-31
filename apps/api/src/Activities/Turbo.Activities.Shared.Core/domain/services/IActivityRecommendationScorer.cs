using Turboapi.Activities.value;

namespace Turboapi.Activities.domain.services;

/// <summary>
/// Per-kind adapter that the cross-kind recommendation endpoint uses to
/// score candidates without taking a hard dependency on each Core
/// assembly. Each kind module registers an implementation in DI that:
///
/// <list type="bullet">
///   <item>Loads the typed activity for each candidate id through its
///         own reader.</item>
///   <item>Runs the kind's orchestrator via <c>QuickScoreAsync</c> with
///         a constrained provider set (no observation lookup, no
///         snapshot history) for cheap scoring.</item>
///   <item>Projects the resulting <see cref="ActivityAnalysis"/> into a
///         compact <see cref="RecommendationScore"/> the controller can
///         rank and serialize.</item>
/// </list>
/// </summary>
public interface IActivityRecommendationScorer
{
    /// <summary>Kind key this scorer handles (e.g. <c>"xc_ski"</c>).</summary>
    string Kind { get; }

    Task<IReadOnlyList<RecommendationScore>> ScoreAsync(
        IReadOnlyList<Guid> activityIds,
        QueryContext queryContext,
        CancellationToken cancellationToken);
}

public sealed record RecommendationScore(
    Guid ActivityId,
    int? Score,
    ScoreConfidence Confidence,
    string Headline,
    string? TopDriverLabel,
    TimeWindow? SuggestedWindow,
    IReadOnlyList<Warning> TopWarnings);
