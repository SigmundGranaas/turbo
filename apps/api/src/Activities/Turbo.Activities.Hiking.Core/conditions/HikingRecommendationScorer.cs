using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Hiking.domain.handler;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Hiking.conditions;

public sealed class HikingRecommendationScorer : IActivityRecommendationScorer
{
    private readonly IHikingActivityReader _reader;
    private readonly HikingOrchestrator _orchestrator;
    private readonly IActivityGeoContextService _geoContext;
    private readonly ILogger<HikingRecommendationScorer> _logger;

    public HikingRecommendationScorer(
        IHikingActivityReader reader,
        HikingOrchestrator orchestrator,
        IActivityGeoContextService geoContext,
        ILogger<HikingRecommendationScorer> logger)
    {
        _reader = reader;
        _orchestrator = orchestrator;
        _geoContext = geoContext;
        _logger = logger;
    }

    public string Kind => "hiking";

    private static readonly HashSet<string> CheapKeys = new(StringComparer.Ordinal)
    {
        HikingOrchestrator.ProviderKeyWeather,
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
                          ?? await _geoContext.ComputeTransientAsync(activity.Route, cancellationToken);
                var analysis = await _orchestrator.QuickScoreAsync(
                    activity, id, geo, queryContext, CheapKeys, cancellationToken);
                results.Add(Project(id, analysis));
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Hiking quick-score failed for {Id}", id);
            }
        }
        return results;
    }

    private static RecommendationScore Project(Guid id, ActivityAnalysis a)
    {
        var topDriver = a.Drivers.OrderByDescending(d => d.Weight * d.Confidence).FirstOrDefault();
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
