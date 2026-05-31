using System.Text.Json;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Freediving.domain;
using Turboapi.Activities.Freediving.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Freediving.conditions;

/// <summary>
/// Freediving orchestrator — the kind that motivated this whole rewrite.
/// The original critique: the legacy advisor exposed a user-entered
/// <c>typicalVisibilityMeters</c> field which is asking the user to
/// predict the very thing the system should be predicting for them. This
/// orchestrator emits a <c>viz_estimate</c> driver computed from
/// season-of-year + recent precipitation runoff + wind-driven mixing,
/// plus surface-chop + sea-temp + tide-phase drivers and HAB warnings.
/// The user-entered visibility field stays on the wire for one release as
/// a deprecated mirror; the orchestrator doesn't read it.
///
/// Drivers:
/// <list type="bullet">
///   <item><c>viz_estimate</c> — composite forecast for visibility</item>
///   <item><c>surface_chop</c> — wind × open-water exposure</item>
///   <item><c>sea_temp</c> — proxy for thermal load (air temp until SST provider)</item>
///   <item><c>tide_phase</c> — slack-tide windows for sea entries</item>
///   <item><c>nearby_obs</c> — recent user-reported viz on this spot</item>
/// </list>
/// </summary>
public sealed class FreedivingOrchestrator
    : ActivityOrchestratorPipeline<FreedivingActivity, ActivityAnalysis>
{
    public const string ProviderKeyWeather = "weather";
    public const string ProviderKeyTide = "tide";
    public const string ProviderKeyTurbidity = "turbidity";

    private readonly IWeatherProvider _weather;
    private readonly ITideProvider? _tide;
    private readonly ITurbidityProvider _turbidity;
    private readonly TimeProvider _clock;

    public FreedivingOrchestrator(
        IActivityGeoContextService geoContext,
        IActivityObservationStore observations,
        IActivityVisitStore visits,
        IConditionsSnapshotStore snapshots,
        IActivitySummaryScoreWriter scoreWriter,
        ILogger<FreedivingOrchestrator> logger,
        IWeatherProvider weather,
        ITurbidityProvider turbidity,
        ITideProvider? tide = null,
        TimeProvider? clock = null)
        : base(geoContext, observations, visits, snapshots, logger, scoreWriter)
    {
        _weather = weather;
        _tide = tide;
        _turbidity = turbidity;
        _clock = clock ?? TimeProvider.System;
    }

    protected override string KindKey => "freediving";

    protected override NetTopologySuite.Geometries.Geometry ExtractGeometry(FreedivingActivity activity)
        => activity.Position;

    protected override IReadOnlyList<ProviderTask> PlanFanOut(
        FreedivingActivity activity, ActivityGeoContext? geoContext, QueryContext queryContext)
    {
        var p = activity.Position;
        var at = queryContext.At;
        var tasks = new List<ProviderTask>
        {
            new(
                ProviderKeyWeather,
                async ct => await _weather.GetAsync(p.Y, p.X, at, ct).ConfigureAwait(false),
                extractObservedAt: s => s is WeatherSlice w ? w.ValidAt : null),
            new(
                ProviderKeyTurbidity,
                async ct => await _turbidity.GetAsync(p.Y, p.X, at, ct).ConfigureAwait(false),
                extractObservedAt: s => s is TurbiditySlice t ? t.ValidAt : null),
        };
        if (_tide is not null && activity.Details.WaterBody == WaterBody.Sea)
        {
            tasks.Add(new ProviderTask(
                ProviderKeyTide,
                async ct => await _tide.GetAsync(p.Y, p.X, at, ct).ConfigureAwait(false),
                extractObservedAt: s => s is TideSlice t ? t.ValidAt : null));
        }
        return tasks;
    }

    protected override ActivityAnalysis Synthesize(SynthesisInput<FreedivingActivity> input)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        var weather = input.Get<WeatherSlice>(ProviderKeyWeather);
        var tide = input.Get<TideSlice>(ProviderKeyTide);
        var turbidity = input.Get<TurbiditySlice>(ProviderKeyTurbidity);
        var details = input.Activity.Details;
        var queryAt = input.QueryContext.At;

        var drivers = new List<Driver>();
        var warnings = new List<Warning>();

        // ---- Viz estimate (the headline driver) -------------------------
        // Computed, not asked. Falls back to a wide band when we have no
        // signal at all rather than echoing the user's stored estimate.
        var (vizLow, vizHigh, vizDirection, vizRationale, vizConfidence) =
            EstimateVisibility(weather, turbidity, queryAt, details);
        drivers.Add(new Driver(
            key: "viz_estimate",
            label: "Visibility",
            value: (vizLow + vizHigh) / 2.0,
            unit: "m",
            weight: 0.35,
            confidence: vizConfidence,
            direction: vizDirection,
            band: null,
            rationale: vizRationale));

        if (weather is not null)
        {
            var p1h = weather.PrecipitationNext1hMm ?? 0;
            var p6h = weather.PrecipitationNext6hMm ?? 0;
            if (details.ShoreEntry && (p1h > 5 || p6h > 15))
            {
                warnings.Add(new Warning(
                    code: "STORM_RUNOFF_24H",
                    severity: Severity.Caution,
                    title: "Storm runoff likely affecting visibility",
                    body: $"{p6h:F0} mm forecast over the next 6h — shore entries pick up sediment from runoff. Visibility band reflects the conservative estimate.",
                    sourceUrl: null));
            }
        }

        // ---- Surface chop ----------------------------------------------
        if (weather is not null)
        {
            var wind = weather.WindSpeedMs;
            string dir;
            string chopRationale;
            if (wind > 10) { dir = "choppy"; chopRationale = $"{wind:F0} m/s wind — surface chop, hard kick-up."; }
            else if (wind > 6) { dir = "light chop"; chopRationale = $"{wind:F0} m/s wind — manageable surface."; }
            else { dir = "glassy"; chopRationale = $"{wind:F0} m/s wind — calm surface."; }
            drivers.Add(new Driver(
                key: "surface_chop",
                label: "Surface conditions",
                value: wind,
                unit: "m/s",
                weight: 0.20,
                confidence: 0.85,
                direction: dir,
                band: null,
                rationale: chopRationale));
        }

        // ---- Sea temp proxy (air-temp until SST provider lands) --------
        if (weather is not null)
        {
            var t = weather.AirTemperatureCelsius;
            string dir;
            string tempRationale;
            if (t > 18) { dir = "warm"; tempRationale = "Air temp suggests warm water — thinner wetsuit possible."; }
            else if (t > 8) { dir = "moderate"; tempRationale = "Moderate temps — standard wetsuit."; }
            else if (t > 0) { dir = "cold"; tempRationale = "Cold conditions — thick wetsuit, plan warmup."; }
            else { dir = "very cold"; tempRationale = "Sub-zero air — water shock risk; consider drysuit."; }
            drivers.Add(new Driver(
                key: "sea_temp_proxy",
                label: "Thermal load",
                value: t,
                unit: "°C",
                weight: 0.15,
                confidence: 0.5,         // air ≠ water; low confidence by design
                direction: dir,
                band: null,
                rationale: tempRationale));
        }

        // ---- Tide phase (sea only) -------------------------------------
        if (tide is not null)
        {
            var heightM = tide.CurrentHeightMeters;
            drivers.Add(new Driver(
                key: "tide_phase",
                label: "Tide",
                value: heightM is null ? null : (double?)heightM.Value,
                unit: "m",
                weight: 0.15,
                confidence: 0.8,
                direction: tide.Summary,
                band: null,
                rationale: heightM is null
                    ? tide.Summary ?? "Tide phase"
                    : $"Sea level {heightM:F2} m — {tide.Summary}"));
        }
        else if (details.WaterBody == WaterBody.Sea)
        {
            drivers.Add(MissingDriver(
                "tide_phase",
                "Tide",
                0.15,
                "Tide data unavailable — check Sehavnivå before sea entries."));
        }

        // ---- Recent observations on this spot --------------------------
        var ownDriver = BuildObservationDriver(input);
        if (ownDriver is not null) drivers.Add(ownDriver);

        var (score, confidence) = CompositeScoreAndConfidence(drivers);
        var rationale = BuildRationale(drivers, weather, tide, vizLow, vizHigh, vizDirection);

        var windows = input.QueryContext.IncludeWindows
            ? BuildSuggestedWindows(tide, weather)
            : Array.Empty<TimeWindow>();

        var kindSlices = new Dictionary<string, JsonElement>();
        kindSlices["freediving"] = JsonSerializer.SerializeToElement(new
        {
            vizForecast = new
            {
                low = vizLow,
                high = vizHigh,
                direction = vizDirection,
                confidence = vizConfidence,
            },
            tide = tide is null
                ? null
                : new
                {
                    heightM = (double?)tide.CurrentHeightMeters,
                    summary = tide.Summary,
                },
            // Surface the user-entered field as a hint until it's removed —
            // not used in scoring but useful for the UI's "you said vs.
            // we estimate" comparison row.
            storedTypicalVizM = details.TypicalVisibilityMeters,
        });

        sw.Stop();
        return new ActivityAnalysis(
            activityId: input.ActivityId,
            kind: KindKey,
            validAt: weather?.ValidAt ?? queryAt,
            fetchedAt: _clock.GetUtcNow(),
            score: score,
            confidence: confidence,
            rationale: rationale,
            drivers: drivers,
            bands: Array.Empty<ForecastBand>(),
            warnings: warnings,
            suggestedWindows: windows,
            kindSlices: kindSlices,
            provenance: input.ToProvenance(sw.ElapsedMilliseconds));
    }

    /// <summary>
    /// Coarse visibility model. Returns (low, high) meters, a direction
    /// label, prose rationale, and a confidence value. The model is
    /// intentionally honest — we don't yet have Sentinel turbidity or a
    /// real upstream runoff signal, so the "Why" line names the drivers
    /// that *did* contribute, and confidence is low when only the
    /// season baseline is in play.
    /// </summary>
    private static (double Low, double High, string Direction, string Rationale, double Confidence)
        EstimateVisibility(
            WeatherSlice? weather,
            TurbiditySlice? turbidity,
            DateTimeOffset at,
            FreedivingDetails details)
    {
        // Seasonal baseline. Norwegian fjords: summer plankton blooms
        // mid-Jun→Aug cut viz; clearest is late winter to early spring.
        var doy = at.UtcDateTime.DayOfYear;
        var seasonalLow = 4.0;
        var seasonalHigh = 9.0;
        var seasonNote = "Late-winter to spring baseline (~4–9 m).";
        if (doy >= 160 && doy <= 244) // Jun → late Aug
        {
            seasonalLow = 2.0;
            seasonalHigh = 5.0;
            seasonNote = "Summer plankton bloom likely cuts visibility to ~2–5 m.";
        }
        else if (doy >= 105 && doy <= 159) // mid-Apr → early Jun
        {
            seasonalLow = 3.0;
            seasonalHigh = 7.0;
            seasonNote = "Late-spring transition (~3–7 m); blooms emerging.";
        }

        var low = seasonalLow;
        var high = seasonalHigh;
        var contributors = new List<string> { seasonNote };
        var confidence = 0.4;

        if (weather is not null)
        {
            confidence = 0.55;
            // Recent rain on a shore-entry → sediment plume.
            var p1h = weather.PrecipitationNext1hMm ?? 0;
            var p6h = weather.PrecipitationNext6hMm ?? 0;
            var rainBudget = p1h + p6h * 0.5;
            if (rainBudget > 5 && details.ShoreEntry)
            {
                var hit = Math.Min(3.0, rainBudget * 0.15);
                low -= hit;
                high -= hit;
                contributors.Add($"recent rain {p6h:F0} mm/6h cuts ~{hit:F0} m.");
            }

            // Wind-driven mixing reduces stratification → poorer viz
            // when surface is whipped up.
            if (weather.WindSpeedMs > 10)
            {
                low -= 1.5;
                high -= 1.5;
                contributors.Add($"{weather.WindSpeedMs:F0} m/s wind mixes surface water.");
            }
        }

        // Direct satellite turbidity overrides the heuristics when it's
        // fresh and clear of cloud cover. NTU → meters of expected
        // visibility is a rough log-ish mapping: 1 NTU ≈ 9 m, 4 NTU ≈ 5,
        // 8 NTU ≈ 2.5. We blend rather than replace because the latest
        // pixel can be several days old.
        if (turbidity is not null && turbidity.CloudCoveragePct < 50 && turbidity.AgeHours <= 96)
        {
            // Confidence rises with both freshness and clear pixels.
            var freshnessWeight = 1.0 - (turbidity.AgeHours / 96.0);
            var clarityWeight = 1.0 - (turbidity.CloudCoveragePct / 50.0);
            var blendWeight = Math.Clamp(freshnessWeight * clarityWeight, 0.0, 1.0);

            var turbidityMidpoint = 9.0 / Math.Max(1.0, turbidity.TurbidityNtu + 1);
            var turbidityRange = Math.Max(1.5, turbidityMidpoint * 0.35);
            var satLow = Math.Max(0.5, turbidityMidpoint - turbidityRange);
            var satHigh = Math.Max(satLow + 0.5, turbidityMidpoint + turbidityRange);

            low = low * (1 - blendWeight) + satLow * blendWeight;
            high = high * (1 - blendWeight) + satHigh * blendWeight;
            confidence = Math.Max(confidence, 0.5 + 0.4 * blendWeight);
            contributors.Add(
                $"Sentinel turbidity {turbidity.TurbidityNtu:F1} NTU "
                + $"({turbidity.AgeHours}h old, {turbidity.CloudCoveragePct:F0}% cloud).");
        }

        low = Math.Max(0.5, low);
        high = Math.Max(low + 0.5, high);
        // Direction is qualitative — if we made adjustments below baseline,
        // call it "dropping"; otherwise "stable for the season".
        var dropped = (seasonalLow + seasonalHigh) - (low + high) > 1.5;
        var direction = dropped ? "dropping" : "seasonal baseline";

        var rationale = string.Join(" ", contributors);
        return (low, high, direction, rationale, confidence);
    }

    private static Driver? BuildObservationDriver(SynthesisInput<FreedivingActivity> input)
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
            weight: 0.15,
            confidence: Math.Min(1.0, input.RecentObservations.Count / 5.0),
            direction: avgRating >= 4 ? "positive" : avgRating > 0 && avgRating < 3 ? "negative" : null,
            band: null,
            rationale: avgRating > 0
                ? $"{input.RecentObservations.Count} recent observation(s); avg rating {avgRating:F1}/5."
                : $"{input.RecentObservations.Count} recent observation(s) on this spot.");
    }

    private static Driver MissingDriver(string key, string label, double weight, string rationale) =>
        new(key, label, null, null, weight, 0.0, null, null, rationale);

    private static (int? Score, ScoreConfidence Confidence) CompositeScoreAndConfidence(
        IReadOnlyList<Driver> drivers)
    {
        double weightedSum = 0;
        double weightTotal = 0;
        double avgConfNum = 0;
        double weightSumForConf = 0;
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
        var composite = (int)Math.Round(weightedSum / weightTotal * 100);
        composite = Math.Clamp(composite, 0, 100);
        var avgConf = avgConfNum / Math.Max(1e-6, weightSumForConf);
        var band = avgConf > 0.7 ? ScoreConfidence.High
                  : avgConf > 0.4 ? ScoreConfidence.Medium
                  : ScoreConfidence.Low;
        return (composite, band);
    }

    private static double DriverScoreHint(Driver d) => d.Key switch
    {
        "viz_estimate" => d.Value is null ? 0.5 : Math.Clamp(d.Value.Value / 12.0, 0.05, 0.95),
        "surface_chop" => d.Direction switch
        {
            "choppy" => 0.20,
            "light chop" => 0.55,
            "glassy" => 0.95,
            _ => 0.6,
        },
        "sea_temp_proxy" => d.Direction switch
        {
            "warm" => 0.9,
            "moderate" => 0.8,
            "cold" => 0.55,
            "very cold" => 0.25,
            _ => 0.6,
        },
        "tide_phase" => 0.7,
        "nearby_obs" => d.Direction switch
        {
            "positive" => 0.85,
            "negative" => 0.3,
            _ => 0.6,
        },
        _ => 0.5,
    };

    private static string BuildRationale(
        IReadOnlyList<Driver> drivers,
        WeatherSlice? weather,
        TideSlice? tide,
        double vizLow,
        double vizHigh,
        string vizDirection)
    {
        var vizStr = $"viz {vizLow:F0}–{vizHigh:F0} m ({vizDirection})";
        var parts = new List<string> { vizStr };
        if (weather is not null)
        {
            if (weather.WindSpeedMs > 10) parts.Add("choppy surface");
            else if (weather.WindSpeedMs < 4) parts.Add("calm surface");
        }
        if (tide is not null && !string.IsNullOrEmpty(tide.Summary))
            parts.Add(tide.Summary);
        return string.Join("; ", parts) + ".";
    }

    private static IReadOnlyList<TimeWindow> BuildSuggestedWindows(TideSlice? tide, WeatherSlice? weather)
    {
        // Without a forecast horizon for either tide or wind we can't
        // emit precise windows. If we have a tide summary mentioning
        // "slack" or "high", surface a generic "best window after the
        // next high-water slack" suggestion. Otherwise, no window.
        if (tide is null) return Array.Empty<TimeWindow>();
        if (string.IsNullOrEmpty(tide.Summary)) return Array.Empty<TimeWindow>();
        var now = DateTimeOffset.UtcNow;
        return new[]
        {
            new TimeWindow(
                start: now,
                end: now.AddHours(2),
                quality: WindowQuality.Good,
                label: "Around slack tide",
                reason: tide.Summary),
        };
    }
}
