using System.Diagnostics;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using Turboapi.Activities.value;

namespace Turboapi.Activities.domain.services;

/// <summary>
/// Shared orchestration runner every per-kind advisor inherits. Encapsulates
/// the parts every kind got slightly wrong in v1 — parallel provider
/// fan-out with error capture, geo-context lookup, own-data fan-in, OTel
/// tracing — and leaves the proprietary piece (<see cref="Synthesize"/>)
/// pure and easy to unit-test.
/// </summary>
public abstract class ActivityOrchestratorPipeline<TActivity, TAnalysis>
    where TActivity : class
    where TAnalysis : class
{
    private static readonly ActivitySource ActivitySource = new("Turboapi.Activities.Orchestrator");

    private readonly IActivityGeoContextService _geoContext;
    private readonly IActivityObservationStore _observations;
    private readonly IActivityVisitStore _visits;
    private readonly IConditionsSnapshotStore _snapshots;
    private readonly IActivitySummaryScoreWriter? _scoreWriter;
    private readonly ILogger _logger;

    protected ActivityOrchestratorPipeline(
        IActivityGeoContextService geoContext,
        IActivityObservationStore observations,
        IActivityVisitStore visits,
        IConditionsSnapshotStore snapshots,
        ILogger logger,
        IActivitySummaryScoreWriter? scoreWriter = null)
    {
        _geoContext = geoContext;
        _observations = observations;
        _visits = visits;
        _snapshots = snapshots;
        _scoreWriter = scoreWriter;
        _logger = logger;
    }

    /// <summary>Stable kind identifier used for tracing and i18n
    /// (e.g. <c>"xc_ski"</c>, <c>"backcountry_ski"</c>).</summary>
    protected abstract string KindKey { get; }

    /// <summary>Pull the geometry off a kind-specific activity so the
    /// pipeline can lazy-populate the geo context on lookup misses.
    /// Cheap accessor; no IO.</summary>
    protected abstract Geometry ExtractGeometry(TActivity activity);

    /// <summary>Pull the geometry-derived correlate the orchestrator
    /// reads observations against. Default: the watershed id from
    /// the geo context, which is enough for cross-activity signal
    /// sharing on water bodies. Override per kind if needed.</summary>
    protected virtual string? WatershedCorrelate(ActivityGeoContext geo) => geo.WatershedHrefId;

    /// <summary>Per-kind: the providers + parameters to fan out for
    /// this activity in this query context.</summary>
    protected abstract IReadOnlyList<ProviderTask> PlanFanOut(
        TActivity activity, ActivityGeoContext? geoContext, QueryContext queryContext);

    /// <summary>Per-kind: produce the analysis from the gathered
    /// inputs. <b>Must be pure</b> — no IO, no clocks. Trivially
    /// unit-testable.</summary>
    protected abstract TAnalysis Synthesize(SynthesisInput<TActivity> input);

    /// <summary>Project the typed analysis to the (score, topDriverLabel)
    /// pair the summary projection persists for pin halos + the
    /// recommendation endpoint's quick filter. Returns null to suppress
    /// the write (e.g. when the analysis carries no defensible score).
    /// Default impl handles <see cref="ActivityAnalysis"/>; kinds with a
    /// custom analysis shape override.</summary>
    protected virtual (int? Score, string? TopDriverLabel)? ProjectSummaryScore(TAnalysis analysis)
    {
        if (analysis is ActivityAnalysis a)
        {
            string? topDriverLabel = null;
            if (a.Drivers.Count > 0)
            {
                Driver? best = null;
                double bestWeight = -1;
                foreach (var d in a.Drivers)
                {
                    var w = d.Weight * d.Confidence;
                    if (w > bestWeight) { best = d; bestWeight = w; }
                }
                topDriverLabel = best?.Label;
            }
            return (a.Score, topDriverLabel);
        }
        return null;
    }

    /// <summary>
    /// Run the full pipeline. Soft failures (an upstream timing out,
    /// missing geo context, no recent observations) all show up as
    /// signals on <see cref="SynthesisInput"/> rather than thrown
    /// exceptions. The synthesizer decides what to do with them.
    /// </summary>
    public async Task<TAnalysis> RunAsync(
        TActivity activity,
        Guid activityId,
        QueryContext queryContext,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(activity);

        using var span = ActivitySource.StartActivity($"orchestrator.{KindKey}");
        span?.SetTag("activity.id", activityId);
        span?.SetTag("activity.kind", KindKey);

        var sw = Stopwatch.StartNew();

        var geoContext = await SafeGeoContextLookup(activityId, cancellationToken).ConfigureAwait(false);
        if (geoContext is null && activityId != Guid.Empty)
        {
            // First analysis for this activity — compute and persist the
            // geo context lazily so subsequent reads are cheap. Soft-
            // fails: if the DEM call blows up we keep going with null.
            try
            {
                var geometry = ExtractGeometry(activity);
                geoContext = await _geoContext
                    .ComputeAndStoreAsync(activityId, geometry, cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Lazy geo-context compute failed for {ActivityId}", activityId);
            }
        }
        var providerTasks = PlanFanOut(activity, geoContext, queryContext);

        var providerResultsTask = RunProvidersAsync(providerTasks, cancellationToken);
        var ownDataTask = LoadOwnDataAsync(activityId, geoContext, queryContext, cancellationToken);

        await Task.WhenAll(providerResultsTask, ownDataTask).ConfigureAwait(false);

        var providerResults = providerResultsTask.Result;
        var (recentObservations, watershedObservations, userVisits) = ownDataTask.Result;

        var input = new SynthesisInput<TActivity>(
            activity: activity,
            activityId: activityId,
            queryContext: queryContext,
            geoContext: geoContext,
            providerResults: providerResults,
            recentObservations: recentObservations,
            watershedObservations: watershedObservations,
            userVisits: userVisits,
            snapshots: _snapshots);

        TAnalysis analysis;
        try
        {
            analysis = Synthesize(input);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Synthesis failed for kind {Kind} activity {ActivityId}", KindKey, activityId);
            throw;
        }

        sw.Stop();
        span?.SetTag("orchestrator.duration_ms", sw.ElapsedMilliseconds);
        span?.SetTag("orchestrator.providers", providerResults.Count);
        span?.SetTag("orchestrator.providers_ok", providerResults.Count(p => p.Ok));

        // Write the score back into the cross-kind summary projection so
        // the map can render pin halos + the recommendation endpoint can
        // pre-filter on cached scores. Soft-fails: a write error here
        // doesn't poison the analysis result the caller is waiting for.
        if (_scoreWriter is not null && activityId != Guid.Empty)
        {
            var projected = ProjectSummaryScore(analysis);
            if (projected is { } p)
            {
                try
                {
                    await _scoreWriter
                        .WriteAsync(activityId, p.Score, p.TopDriverLabel, DateTimeOffset.UtcNow, cancellationToken)
                        .ConfigureAwait(false);
                }
                catch (Exception ex)
                {
                    _logger.LogDebug(ex, "Summary score write-back failed for {ActivityId}", activityId);
                }
            }
        }

        return analysis;
    }

    /// <summary>Cheaper variant for the recommendation endpoint — skips
    /// own-data and snapshot lookups, runs only the providers
    /// <paramref name="cheapKeys"/> selects from the kind's fan-out plan,
    /// uses a transient geo context (no DB read) when the activity isn't
    /// persisted yet. Subclasses can override for full control.</summary>
    public virtual async Task<TAnalysis> QuickScoreAsync(
        TActivity activity,
        Guid? activityIdOrNull,
        ActivityGeoContext geoContext,
        QueryContext queryContext,
        ISet<string> cheapKeys,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(activity);
        ArgumentNullException.ThrowIfNull(geoContext);
        ArgumentNullException.ThrowIfNull(cheapKeys);

        using var span = ActivitySource.StartActivity($"orchestrator.{KindKey}.quick");
        span?.SetTag("activity.kind", KindKey);

        var plan = PlanFanOut(activity, geoContext, queryContext)
            .Where(p => cheapKeys.Contains(p.Key))
            .ToArray();

        var providerResults = await RunProvidersAsync(plan, cancellationToken).ConfigureAwait(false);

        var input = new SynthesisInput<TActivity>(
            activity: activity,
            activityId: activityIdOrNull ?? Guid.Empty,
            queryContext: queryContext,
            geoContext: geoContext,
            providerResults: providerResults,
            recentObservations: Array.Empty<ActivityObservation>(),
            watershedObservations: Array.Empty<ActivityObservation>(),
            userVisits: Array.Empty<ActivityVisit>(),
            snapshots: _snapshots);

        return Synthesize(input);
    }

    private async Task<ActivityGeoContext?> SafeGeoContextLookup(Guid activityId, CancellationToken ct)
    {
        if (activityId == Guid.Empty) return null;
        try
        {
            return await _geoContext.GetAsync(activityId, ct).ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "GeoContext lookup failed for {ActivityId}", activityId);
            return null;
        }
    }

    private async Task<IReadOnlyList<ProviderResult>> RunProvidersAsync(
        IReadOnlyList<ProviderTask> tasks, CancellationToken ct)
    {
        if (tasks.Count == 0) return Array.Empty<ProviderResult>();

        var running = tasks.Select(t => RunOneAsync(t, ct)).ToArray();
        await Task.WhenAll(running).ConfigureAwait(false);
        return running.Select(r => r.Result).ToArray();
    }

    private async Task<ProviderResult> RunOneAsync(ProviderTask task, CancellationToken ct)
    {
        var fetchedAt = DateTimeOffset.UtcNow;
        try
        {
            var slice = await task.Run(ct).ConfigureAwait(false);
            int? ageSeconds = null;
            if (slice is not null && task.ExtractObservedAt is { } extract)
            {
                var observedAt = extract(slice);
                if (observedAt is not null)
                {
                    var delta = fetchedAt - observedAt.Value;
                    if (delta > TimeSpan.Zero) ageSeconds = (int)delta.TotalSeconds;
                }
            }
            return new ProviderResult(task.Key, slice, slice is not null, FromCache: false, fetchedAt, ageSeconds, Error: null);
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "Provider {Key} failed in pipeline", task.Key);
            return new ProviderResult(task.Key, Slice: null, Ok: false, FromCache: false, fetchedAt, AgeSeconds: null, Error: ex.GetType().Name);
        }
    }

    private async Task<(
            IReadOnlyList<ActivityObservation> Recent,
            IReadOnlyList<ActivityObservation> Watershed,
            IReadOnlyList<ActivityVisit> Visits)>
        LoadOwnDataAsync(
            Guid activityId,
            ActivityGeoContext? geoContext,
            QueryContext queryContext,
            CancellationToken ct)
    {
        if (activityId == Guid.Empty)
        {
            return (Array.Empty<ActivityObservation>(), Array.Empty<ActivityObservation>(), Array.Empty<ActivityVisit>());
        }

        var since = queryContext.At - queryContext.Lookback;

        Task<IReadOnlyList<ActivityObservation>> recentTask;
        try { recentTask = _observations.GetForActivityAsync(activityId, since, limit: 50, ct); }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "Observation lookup failed for {ActivityId}", activityId);
            recentTask = Task.FromResult<IReadOnlyList<ActivityObservation>>(Array.Empty<ActivityObservation>());
        }

        var watershedKey = geoContext is not null ? WatershedCorrelate(geoContext) : null;
        Task<IReadOnlyList<ActivityObservation>> watershedTask;
        if (string.IsNullOrEmpty(watershedKey))
        {
            watershedTask = Task.FromResult<IReadOnlyList<ActivityObservation>>(Array.Empty<ActivityObservation>());
        }
        else
        {
            try { watershedTask = _observations.GetForWatershedAsync(watershedKey, since, limit: 50, ct); }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Watershed observation lookup failed for {Watershed}", watershedKey);
                watershedTask = Task.FromResult<IReadOnlyList<ActivityObservation>>(Array.Empty<ActivityObservation>());
            }
        }

        Task<IReadOnlyList<ActivityVisit>> visitTask;
        if (queryContext.RequestingUserId is { } userId && userId != Guid.Empty)
        {
            try { visitTask = _visits.GetForUserAsync(userId, activityId, since, limit: 20, ct); }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Visit lookup failed for {ActivityId}", activityId);
                visitTask = Task.FromResult<IReadOnlyList<ActivityVisit>>(Array.Empty<ActivityVisit>());
            }
        }
        else
        {
            visitTask = Task.FromResult<IReadOnlyList<ActivityVisit>>(Array.Empty<ActivityVisit>());
        }

        await Task.WhenAll(recentTask, watershedTask, visitTask).ConfigureAwait(false);
        return (recentTask.Result, watershedTask.Result, visitTask.Result);
    }
}

/// <summary>
/// Bundle of everything a kind's <c>Synthesize</c> needs. Pure data — no
/// services on it other than the read-only <see cref="Snapshots"/> store
/// (synthesizers querying the snapshot store are expected to do it
/// sparingly; heavier history pulls should be modelled as
/// <see cref="ProviderTask"/>s instead).
/// </summary>
public sealed class SynthesisInput<TActivity>
{
    public TActivity Activity { get; }
    public Guid ActivityId { get; }
    public QueryContext QueryContext { get; }
    public ActivityGeoContext? GeoContext { get; }
    public IReadOnlyList<ProviderResult> ProviderResults { get; }
    public IReadOnlyList<ActivityObservation> RecentObservations { get; }
    public IReadOnlyList<ActivityObservation> WatershedObservations { get; }
    public IReadOnlyList<ActivityVisit> UserVisits { get; }
    public IConditionsSnapshotStore Snapshots { get; }

    public SynthesisInput(
        TActivity activity,
        Guid activityId,
        QueryContext queryContext,
        ActivityGeoContext? geoContext,
        IReadOnlyList<ProviderResult> providerResults,
        IReadOnlyList<ActivityObservation> recentObservations,
        IReadOnlyList<ActivityObservation> watershedObservations,
        IReadOnlyList<ActivityVisit> userVisits,
        IConditionsSnapshotStore snapshots)
    {
        Activity = activity;
        ActivityId = activityId;
        QueryContext = queryContext;
        GeoContext = geoContext;
        ProviderResults = providerResults;
        RecentObservations = recentObservations;
        WatershedObservations = watershedObservations;
        UserVisits = userVisits;
        Snapshots = snapshots;
    }

    /// <summary>Resolve a typed slice from a successful provider result
    /// by key. Returns <c>null</c> when the provider failed or wasn't in
    /// the fan-out plan. Synthesizers should check this and degrade
    /// gracefully (lower the affected driver's confidence rather than
    /// throwing).</summary>
    public T? Get<T>(string key) where T : class
    {
        foreach (var r in ProviderResults)
        {
            if (r.Key == key && r.Ok && r.Slice is T t) return t;
        }
        return null;
    }

    public ProviderResult? Find(string key)
    {
        foreach (var r in ProviderResults)
        {
            if (r.Key == key) return r;
        }
        return null;
    }

    /// <summary>Roll provider results into the <see cref="Provenance"/>
    /// shape the analysis surfaces to the client. Convenience used by
    /// every kind synthesizer.</summary>
    public Provenance ToProvenance(long durationMs)
    {
        var hits = new SourceHit[ProviderResults.Count];
        for (var i = 0; i < ProviderResults.Count; i++)
        {
            var r = ProviderResults[i];
            hits[i] = new SourceHit(
                providerKey: r.Key,
                ok: r.Ok,
                fromCache: r.FromCache,
                ageSeconds: r.AgeSeconds,
                error: r.Error);
        }
        return new Provenance(hits, durationMs);
    }
}
