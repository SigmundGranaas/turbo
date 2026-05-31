using System.Text.Json;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Packrafting.domain;
using Turboapi.Activities.Packrafting.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Packrafting.conditions;

/// <summary>
/// Packrafting orchestrator. River flow is the dominant driver; weather
/// gives cold-swim risk. Without a real solar / snowmelt forecast feed
/// we surface a single qualitative spike-risk note when forecast warming
/// + recent precip combine.
///
/// Drivers:
/// <list type="bullet">
///   <item><c>flow_vs_user_window</c> — current NVE flow vs. the user's
///         stored min/max preference</item>
///   <item><c>flow_percentile_doy</c> — current flow vs DOY history from
///         the snapshot store</item>
///   <item><c>cold_swim_risk</c> — air temp + (water temp proxy)</item>
///   <item><c>nearby_obs</c> — recent paddler observations on this river</item>
/// </list>
/// </summary>
public sealed class PackraftingOrchestrator
    : ActivityOrchestratorPipeline<PackraftingActivity, ActivityAnalysis>
{
    public const string ProviderKeyWeather = "weather";
    public const string ProviderKeyRiverFlow = "river_flow";

    private readonly IWeatherProvider _weather;
    private readonly IRiverFlowProvider? _flow;
    private readonly TimeProvider _clock;

    public PackraftingOrchestrator(
        IActivityGeoContextService geoContext,
        IActivityObservationStore observations,
        IActivityVisitStore visits,
        IConditionsSnapshotStore snapshots,
        IActivitySummaryScoreWriter scoreWriter,
        ILogger<PackraftingOrchestrator> logger,
        IWeatherProvider weather,
        IRiverFlowProvider? flow = null,
        TimeProvider? clock = null)
        : base(geoContext, observations, visits, snapshots, logger, scoreWriter)
    {
        _weather = weather;
        _flow = flow;
        _clock = clock ?? TimeProvider.System;
    }

    protected override string KindKey => "packrafting";

    protected override NetTopologySuite.Geometries.Geometry ExtractGeometry(PackraftingActivity activity)
        => activity.Route;

    protected override IReadOnlyList<ProviderTask> PlanFanOut(
        PackraftingActivity activity, ActivityGeoContext? geoContext, QueryContext queryContext)
    {
        var line = activity.Route;
        var mid = line.Coordinates[line.NumPoints / 2];
        var at = queryContext.At;
        var tasks = new List<ProviderTask>
        {
            new(
                ProviderKeyWeather,
                async ct => await _weather.GetAsync(mid.Y, mid.X, at, ct).ConfigureAwait(false),
                extractObservedAt: s => s is WeatherSlice w ? w.ValidAt : null),
        };
        if (_flow is not null && activity.Details.NveStationCode is { } code)
        {
            tasks.Add(new ProviderTask(
                ProviderKeyRiverFlow,
                async ct => await _flow.GetAsync(code, at, ct).ConfigureAwait(false),
                extractObservedAt: s => s is RiverFlowSlice f ? f.ValidAt : null));
        }
        return tasks;
    }

    protected override ActivityAnalysis Synthesize(SynthesisInput<PackraftingActivity> input)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        var weather = input.Get<WeatherSlice>(ProviderKeyWeather);
        var flow = input.Get<RiverFlowSlice>(ProviderKeyRiverFlow);
        var details = input.Activity.Details;
        var drivers = new List<Driver>();
        var warnings = new List<Warning>();

        // ---- Flow vs user window --------------------------------------
        if (flow is not null)
        {
            var current = flow.CurrentCumecs;
            string dir;
            string flowRationale;
            var min = details.MinFlowCumecs;
            var max = details.MaxFlowCumecs;
            if (min is { } mn && current < mn)
            {
                dir = "below-navigable";
                flowRationale = $"Flow {current:F0} m³/s is below your min ({mn:F0}). Run will be scrappy or unrunnable.";
                warnings.Add(new Warning(
                    code: "BELOW_NAVIGABLE",
                    severity: Severity.Caution,
                    title: "Flow below your minimum",
                    body: "Expect dragging over shallow sections.",
                    sourceUrl: null));
            }
            else if (max is { } mx && current > mx)
            {
                dir = "above-window";
                flowRationale = $"Flow {current:F0} m³/s is above your max ({mx:F0}) — class likely bumped up.";
                warnings.Add(new Warning(
                    code: "ABOVE_USER_WINDOW",
                    severity: Severity.Caution,
                    title: "Flow above your window",
                    body: "Water is pushier than you set as your upper bound — bigger features, less margin.",
                    sourceUrl: null));
            }
            else
            {
                dir = "in-window";
                flowRationale = $"Flow {current:F0} m³/s — within your stored window.";
            }
            drivers.Add(new Driver(
                key: "flow_vs_user_window",
                label: "Flow",
                value: current,
                unit: "m³/s",
                weight: 0.35,
                confidence: 0.9,
                direction: dir,
                band: null,
                rationale: flowRationale));

            // Trend warning.
            if (string.Equals(flow.Trend, "rising", StringComparison.OrdinalIgnoreCase))
            {
                warnings.Add(new Warning(
                    code: "RISING_FAST",
                    severity: Severity.Info,
                    title: "Flow rising",
                    body: "Discharge is on the way up — features may grow during the run.",
                    sourceUrl: null));
            }
        }
        else
        {
            drivers.Add(MissingDriver(
                "flow_vs_user_window",
                "Flow",
                0.35,
                details.NveStationCode is null
                    ? "No NVE station linked — flow context unavailable."
                    : "Flow data unavailable for this station — check NVE manually."));
        }

        // ---- Flow percentile for day-of-year (snapshot-history) --------
        var pctDriver = TryBuildFlowPercentileDriver(input, flow, details);
        if (pctDriver is not null) drivers.Add(pctDriver);

        // ---- Cold-swim risk -------------------------------------------
        if (weather is not null)
        {
            var t = weather.AirTemperatureCelsius;
            string dir;
            string riskRationale;
            if (t < 5)
            {
                dir = "drysuit";
                riskRationale = $"Air {t:F0}°C — water is colder still; drysuit required.";
            }
            else if (t < 12)
            {
                dir = "wetsuit";
                riskRationale = $"Air {t:F0}°C — wetsuit recommended; cold-shock real.";
            }
            else
            {
                dir = "ambient";
                riskRationale = $"Air {t:F0}°C — standard kit.";
            }
            drivers.Add(new Driver(
                key: "cold_swim_risk",
                label: "Cold-swim risk",
                value: t,
                unit: "°C",
                weight: 0.20,
                confidence: 0.7,
                direction: dir,
                band: null,
                rationale: riskRationale));
        }

        // ---- Observations ---------------------------------------------
        var obsDriver = BuildObservationDriver(input);
        if (obsDriver is not null) drivers.Add(obsDriver);

        var (score, confidence) = CompositeScoreAndConfidence(drivers);
        var rationale = BuildRationale(drivers);

        // KindSlice: current flow + percentile + trend.
        var kindSlices = new Dictionary<string, JsonElement>();
        kindSlices["packrafting"] = JsonSerializer.SerializeToElement(new
        {
            currentCumecs = flow?.CurrentCumecs,
            trend = flow?.Trend,
            userMinCumecs = details.MinFlowCumecs,
            userMaxCumecs = details.MaxFlowCumecs,
            percentile = (drivers.FirstOrDefault(d => d.Key == "flow_percentile_doy")?.Value),
        });

        sw.Stop();
        return new ActivityAnalysis(
            activityId: input.ActivityId,
            kind: KindKey,
            validAt: weather?.ValidAt ?? input.QueryContext.At,
            fetchedAt: _clock.GetUtcNow(),
            score: score,
            confidence: confidence,
            rationale: rationale,
            drivers: drivers,
            bands: Array.Empty<ForecastBand>(),
            warnings: warnings,
            suggestedWindows: Array.Empty<TimeWindow>(),
            kindSlices: kindSlices,
            provenance: input.ToProvenance(sw.ElapsedMilliseconds));
    }

    private static Driver? TryBuildFlowPercentileDriver(
        SynthesisInput<PackraftingActivity> input,
        RiverFlowSlice? flow,
        PackraftingDetails details)
    {
        if (flow is null) return null;
        if (details.NveStationCode is null) return null;
        // Look the percentile up across both possible provider keys.
        var providerKeys = new[] { "nve_river_flow", "synthetic_river_flow" };
        foreach (var key in providerKeys)
        {
            try
            {
                var pct = input.Snapshots.GetPercentileAsync(
                    providerKey: key,
                    gridCell: $"station_{details.NveStationCode}",
                    doyWindowDays: 7,
                    metricKey: "currentCumecs",
                    currentValue: flow.CurrentCumecs,
                    cancellationToken: CancellationToken.None).GetAwaiter().GetResult();
                if (pct is null) continue;
                var pctInt = (int)Math.Round(pct.Value * 100);
                string dir;
                string rationale;
                if (pctInt >= 80) { dir = "very high"; rationale = $"Flow at {pctInt}th percentile vs DOY ±7 — water pushier than typical."; }
                else if (pctInt >= 60) { dir = "high"; rationale = $"Flow at {pctInt}th percentile — above-typical levels."; }
                else if (pctInt >= 30) { dir = "typical"; rationale = $"Flow at {pctInt}th percentile — normal for this date."; }
                else { dir = "low"; rationale = $"Flow at {pctInt}th percentile — below typical."; }
                return new Driver(
                    key: "flow_percentile_doy",
                    label: "Flow percentile",
                    value: pctInt,
                    unit: "%",
                    weight: 0.15,
                    confidence: 0.7,
                    direction: dir,
                    band: null,
                    rationale: rationale);
            }
            catch
            {
                // Soft fail
            }
        }
        return null;
    }

    private static Driver MissingDriver(string key, string label, double weight, string rationale) =>
        new(key, label, null, null, weight, 0.0, null, null, rationale);

    private static Driver? BuildObservationDriver(SynthesisInput<PackraftingActivity> input)
    {
        if (input.RecentObservations.Count == 0) return null;
        var avgRating = input.RecentObservations
            .Where(o => o.Rating.HasValue)
            .Select(o => (double)o.Rating!.Value)
            .DefaultIfEmpty(0)
            .Average();
        return new Driver(
            key: "nearby_obs",
            label: "Recent observations",
            value: input.RecentObservations.Count,
            unit: null,
            weight: 0.10,
            confidence: Math.Min(1.0, input.RecentObservations.Count / 5.0),
            direction: avgRating >= 4 ? "positive" : avgRating > 0 && avgRating < 3 ? "negative" : null,
            band: null,
            rationale: avgRating > 0
                ? $"{input.RecentObservations.Count} recent observation(s); avg rating {avgRating:F1}/5."
                : $"{input.RecentObservations.Count} recent observation(s) on this river.");
    }

    private static (int? Score, ScoreConfidence Confidence) CompositeScoreAndConfidence(
        IReadOnlyList<Driver> drivers)
    {
        double weightedSum = 0, weightTotal = 0, avgConfNum = 0, weightSumForConf = 0;
        foreach (var d in drivers)
        {
            if (d.Confidence <= 0) continue;
            var raw = DriverScoreHint(d);
            weightedSum += raw * d.Weight * d.Confidence;
            weightTotal += d.Weight * d.Confidence;
            avgConfNum += d.Confidence * d.Weight;
            weightSumForConf += d.Weight;
        }
        if (weightTotal < 1e-6) return (null, ScoreConfidence.Low);
        var composite = Math.Clamp((int)Math.Round(weightedSum / weightTotal * 100), 0, 100);
        var avgConf = avgConfNum / Math.Max(1e-6, weightSumForConf);
        var band = avgConf > 0.7 ? ScoreConfidence.High
                  : avgConf > 0.4 ? ScoreConfidence.Medium
                  : ScoreConfidence.Low;
        return (composite, band);
    }

    private static double DriverScoreHint(Driver d) => d.Key switch
    {
        "flow_vs_user_window" => d.Direction switch
        {
            "in-window" => 0.95,
            "below-navigable" => 0.15,
            "above-window" => 0.30,
            _ => 0.5,
        },
        "flow_percentile_doy" => d.Direction switch
        {
            "typical" => 0.90,
            "high" => 0.65,
            "low" => 0.55,
            "very high" => 0.35,
            _ => 0.6,
        },
        "cold_swim_risk" => d.Direction switch
        {
            "ambient" => 0.95,
            "wetsuit" => 0.65,
            "drysuit" => 0.35,
            _ => 0.6,
        },
        "nearby_obs" => d.Direction switch
        {
            "positive" => 0.85,
            "negative" => 0.3,
            _ => 0.6,
        },
        _ => 0.5,
    };

    private static string BuildRationale(IReadOnlyList<Driver> drivers)
    {
        var parts = new List<string>();
        foreach (var d in drivers.OrderByDescending(x => x.Weight * x.Confidence).Take(3))
        {
            if (d.Direction is not null) parts.Add($"{d.Label.ToLowerInvariant()} {d.Direction}");
        }
        return parts.Count == 0 ? "Insufficient signal." : string.Join(", ", parts) + ".";
    }
}
