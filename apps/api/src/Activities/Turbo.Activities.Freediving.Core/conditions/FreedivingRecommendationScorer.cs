using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Freediving.domain.handler;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Freediving.conditions;

/// <summary>
/// Per-kind scorer for the cross-kind recommendation endpoint. Cheap
/// provider subset: weather + tide only — the visibility estimate is
/// pure compute from season + recent rain.
/// </summary>
public sealed class FreedivingRecommendationScorer : IActivityRecommendationScorer
{
    private readonly IFreedivingActivityReader _reader;
    private readonly FreedivingOrchestrator _orchestrator;
    private readonly IActivityGeoContextService _geoContext;
    private readonly ILogger<FreedivingRecommendationScorer> _logger;

    public FreedivingRecommendationScorer(
        IFreedivingActivityReader reader,
        FreedivingOrchestrator orchestrator,
        IActivityGeoContextService geoContext,
        ILogger<FreedivingRecommendationScorer> logger)
    {
        _reader = reader;
        _orchestrator = orchestrator;
        _geoContext = geoContext;
        _logger = logger;
    }

    public string Kind => "freediving";

    private static readonly HashSet<string> CheapKeys = new(StringComparer.Ordinal)
    {
        FreedivingOrchestrator.ProviderKeyWeather,
        FreedivingOrchestrator.ProviderKeyTide,
    };

    public async Task<IReadOnlyList<RecommendationScore>> ScoreAsync(
        IReadOnlyList<Guid> activityIds, QueryContext queryContext, CancellationToken cancellationToken)
    {
        if (activityIds.Count == 0) return Array.Empty<RecommendationScore>();
        var results = new List<RecommendationScore>(activityIds.Count);
        foreach (var id in activityIds)
        {
            if (cancellationToken.IsCancellationRequested) break;
            try
            {
                var activity = await _reader.GetByIdAsync(id, cancellationToken);
                if (activity is null) continue;
                var geo = await _geoContext.GetAsync(id, cancellationToken)
                          ?? await _geoContext.ComputeTransientAsync(activity.Position, cancellationToken);
                var analysis = await _orchestrator.QuickScoreAsync(
                    activity, id, geo, queryContext, CheapKeys, cancellationToken);
                results.Add(Project(id, analysis));
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Freediving quick-score failed for {Id}", id);
            }
        }
        return results;
    }

    private static RecommendationScore Project(Guid id, ActivityAnalysis a)
    {
        var topDriver = a.Drivers
            .OrderByDescending(d => d.Weight * d.Confidence)
            .FirstOrDefault();
        var window = a.SuggestedWindows.Count > 0 ? a.SuggestedWindows[0] : null;
        var topWarnings = a.Warnings.OrderByDescending(w => w.Severity).Take(2).ToArray();
        return new RecommendationScore(
            ActivityId: id,
            Score: a.Score,
            Confidence: a.Confidence,
            Headline: a.Score.HasValue ? a.Rationale : "Insufficient signal to score",
            TopDriverLabel: topDriver?.Label,
            SuggestedWindow: window,
            TopWarnings: topWarnings);
    }
}
