using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Fishing.domain.handler;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Fishing.conditions;

public sealed class FishingRecommendationScorer : IActivityRecommendationScorer
{
    private readonly IFishingActivityReader _reader;
    private readonly FishingOrchestrator _orchestrator;
    private readonly IActivityGeoContextService _geoContext;
    private readonly ILogger<FishingRecommendationScorer> _logger;

    public FishingRecommendationScorer(
        IFishingActivityReader reader,
        FishingOrchestrator orchestrator,
        IActivityGeoContextService geoContext,
        ILogger<FishingRecommendationScorer> logger)
    {
        _reader = reader;
        _orchestrator = orchestrator;
        _geoContext = geoContext;
        _logger = logger;
    }

    public string Kind => "fishing";

    private static readonly HashSet<string> CheapKeys = new(StringComparer.Ordinal)
    {
        FishingOrchestrator.ProviderKeyWeather,
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
                _logger.LogWarning(ex, "Fishing quick-score failed for {Id}", id);
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
