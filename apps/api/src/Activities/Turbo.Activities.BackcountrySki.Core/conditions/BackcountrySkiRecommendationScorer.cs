using Microsoft.Extensions.Logging;
using Turboapi.Activities.BackcountrySki.domain.handler;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;

namespace Turboapi.Activities.BackcountrySki.conditions;

/// <summary>
/// Per-kind scorer for backcountry ski. Same shape as the XC adapter —
/// loads the activity, runs the orchestrator's QuickScore path, projects
/// to <see cref="RecommendationScore"/>. The cheap provider subset
/// excludes regObs (a heavier upstream) so recommendation fan-out stays
/// bounded.
/// </summary>
public sealed class BackcountrySkiRecommendationScorer : IActivityRecommendationScorer
{
    private readonly IBackcountrySkiActivityReader _reader;
    private readonly BackcountrySkiOrchestrator _orchestrator;
    private readonly IActivityGeoContextService _geoContext;
    private readonly ILogger<BackcountrySkiRecommendationScorer> _logger;

    public BackcountrySkiRecommendationScorer(
        IBackcountrySkiActivityReader reader,
        BackcountrySkiOrchestrator orchestrator,
        IActivityGeoContextService geoContext,
        ILogger<BackcountrySkiRecommendationScorer> logger)
    {
        _reader = reader;
        _orchestrator = orchestrator;
        _geoContext = geoContext;
        _logger = logger;
    }

    public string Kind => "backcountry_ski";

    private static readonly HashSet<string> CheapKeys = new(StringComparer.Ordinal)
    {
        BackcountrySkiOrchestrator.ProviderKeyWeather,
        BackcountrySkiOrchestrator.ProviderKeyAvalanche,
        BackcountrySkiOrchestrator.ProviderKeyGriddedSnow,
    };

    public async Task<IReadOnlyList<RecommendationScore>> ScoreAsync(
        IReadOnlyList<Guid> activityIds,
        QueryContext queryContext,
        CancellationToken cancellationToken)
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
                _logger.LogWarning(ex, "Backcountry ski quick-score failed for {Id}", id);
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
        var topWarnings = a.Warnings
            .OrderByDescending(w => w.Severity)
            .Take(2)
            .ToArray();
        var headline = a.Score.HasValue
            ? a.Rationale
            : "Insufficient signal to score";
        return new RecommendationScore(
            ActivityId: id,
            Score: a.Score,
            Confidence: a.Confidence,
            Headline: headline,
            TopDriverLabel: topDriver?.Label,
            SuggestedWindow: window,
            TopWarnings: topWarnings);
    }
}
