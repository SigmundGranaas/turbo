using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;
using Turboapi.Activities.XcSki.domain.handler;

namespace Turboapi.Activities.XcSki.conditions;

/// <summary>
/// Adapter that lets the cross-kind recommendation endpoint score XC ski
/// activities without taking a hard dep on this module. Loads each
/// candidate via the kind's reader, runs the orchestrator's
/// <c>QuickScoreAsync</c> path (cheap provider subset, no
/// observation/snapshot lookups), and projects to a compact
/// <see cref="RecommendationScore"/>.
/// </summary>
public sealed class XcSkiRecommendationScorer : IActivityRecommendationScorer
{
    private readonly IXcSkiActivityReader _reader;
    private readonly XcSkiOrchestrator _orchestrator;
    private readonly IActivityGeoContextService _geoContext;
    private readonly ILogger<XcSkiRecommendationScorer> _logger;

    public XcSkiRecommendationScorer(
        IXcSkiActivityReader reader,
        XcSkiOrchestrator orchestrator,
        IActivityGeoContextService geoContext,
        ILogger<XcSkiRecommendationScorer> logger)
    {
        _reader = reader;
        _orchestrator = orchestrator;
        _geoContext = geoContext;
        _logger = logger;
    }

    public string Kind => "xc_ski";

    private static readonly HashSet<string> CheapKeys = new(StringComparer.Ordinal)
    {
        XcSkiOrchestrator.ProviderKeyWeather,
        XcSkiOrchestrator.ProviderKeyGriddedSnow,
        XcSkiOrchestrator.ProviderKeyGrooming,
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
                _logger.LogWarning(ex, "XC ski quick-score failed for {Id}", id);
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
