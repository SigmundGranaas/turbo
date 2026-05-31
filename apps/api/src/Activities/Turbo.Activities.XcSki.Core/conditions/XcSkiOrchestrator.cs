using System.Text.Json;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;
using Turboapi.Activities.XcSki.domain;
using Turboapi.Activities.XcSki.value;

namespace Turboapi.Activities.XcSki.conditions;

/// <summary>
/// XcSki orchestrator. Subclasses the shared
/// <see cref="ActivityOrchestratorPipeline{TActivity,TAnalysis}"/> base —
/// the base handles geo-context lookup, parallel provider fan-out with
/// soft-failure capture, own-data fan-in, and OTel tracing. This class
/// owns only the per-kind <c>PlanFanOut</c> + pure <c>Synthesize</c>
/// pieces, plus the wax-band emission that ships in
/// <see cref="ActivityAnalysis.KindSlices"/>.
///
/// Drivers consumed:
/// <list type="bullet">
///   <item><c>temp_band</c> — air temperature window for stable snow</item>
///   <item><c>wind</c> — wind exposure on the route</item>
///   <item><c>live_grooming_age</c> — hours since the last grooming pass</item>
///   <item><c>snow_depth</c> — seNorge depth vs the skiable threshold</item>
///   <item><c>fresh_snow_24h</c> — recent accumulation</item>
///   <item><c>nearby_obs</c> — recent xc_ski observations near the route</item>
/// </list>
/// </summary>
public sealed class XcSkiOrchestrator : ActivityOrchestratorPipeline<XcSkiActivity, ActivityAnalysis>
{
    public const string ProviderKeyWeather = "weather";
    public const string ProviderKeyGrooming = "grooming";
    public const string ProviderKeySnowpack = "snowpack";
    public const string ProviderKeyGriddedSnow = "gridded_snow";

    private readonly IWeatherProvider _weather;
    private readonly IGroomingProvider? _grooming;
    private readonly ISnowpackProvider _snowpack;
    private readonly IGriddedSnowProvider _griddedSnow;
    private readonly TimeProvider _clock;

    public XcSkiOrchestrator(
        IActivityGeoContextService geoContext,
        IActivityObservationStore observations,
        IActivityVisitStore visits,
        IConditionsSnapshotStore snapshots,
        IActivitySummaryScoreWriter scoreWriter,
        ILogger<XcSkiOrchestrator> logger,
        IWeatherProvider weather,
        ISnowpackProvider snowpack,
        IGriddedSnowProvider griddedSnow,
        IGroomingProvider? grooming = null,
        TimeProvider? clock = null)
        : base(geoContext, observations, visits, snapshots, logger, scoreWriter)
    {
        _weather = weather;
        _grooming = grooming;
        _snowpack = snowpack;
        _griddedSnow = griddedSnow;
        _clock = clock ?? TimeProvider.System;
    }

    protected override string KindKey => "xc_ski";

    protected override NetTopologySuite.Geometries.Geometry ExtractGeometry(XcSkiActivity activity)
        => activity.Route;

    protected override IReadOnlyList<ProviderTask> PlanFanOut(
        XcSkiActivity activity, ActivityGeoContext? geoContext, QueryContext queryContext)
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
            new(
                ProviderKeySnowpack,
                async ct => await _snowpack.GetAsync(mid.Y, mid.X, at, lookbackDays: 7, ct).ConfigureAwait(false),
                extractObservedAt: s => s is SnowpackSlice sp ? sp.ValidAt : null),
            new(
                ProviderKeyGriddedSnow,
                async ct => await _griddedSnow.GetAsync(mid.Y, mid.X, at, ct).ConfigureAwait(false),
                extractObservedAt: s => s is GriddedSnowSlice gs ? gs.ValidAt : null),
        };

        if (_grooming is not null && activity.Details.GroomingFeedKey is { } feedKey)
        {
            tasks.Add(new ProviderTask(
                ProviderKeyGrooming,
                async ct => await _grooming.GetAsync(feedKey, at, ct).ConfigureAwait(false),
                extractObservedAt: s => s is GroomingSlice gs ? gs.ValidAt : null));
        }

        return tasks;
    }

    protected override ActivityAnalysis Synthesize(SynthesisInput<XcSkiActivity> input)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        var weather = input.Get<WeatherSlice>(ProviderKeyWeather);
        var snowpack = input.Get<SnowpackSlice>(ProviderKeySnowpack);
        var gridded = input.Get<GriddedSnowSlice>(ProviderKeyGriddedSnow);
        var grooming = input.Get<GroomingSlice>(ProviderKeyGrooming);

        var drivers = new List<Driver>();
        var warnings = new List<Warning>();
        var bands = new List<ForecastBand>();

        // ---- Temperature band ----
        if (weather is not null)
        {
            var temp = weather.AirTemperatureCelsius;
            string dir;
            string tempRationale;
            if (temp > 3) { dir = "above freezing"; tempRationale = $"{temp:F0}°C — track is melting / icy."; }
            else if (temp > 0) { dir = "near freezing"; tempRationale = $"{temp:F0}°C — sticky snow likely; klister wax."; }
            else if (temp < -15) { dir = "very cold"; tempRationale = $"{temp:F0}°C — bring serious wax, dry-cold snow."; }
            else { dir = "cold and dry"; tempRationale = $"{temp:F0}°C — sweet spot for kick and glide."; }
            drivers.Add(new Driver(
                key: "temp_band",
                label: "Temperature",
                value: temp,
                unit: "°C",
                weight: 0.25,
                confidence: 0.9,
                direction: dir,
                band: null,
                rationale: tempRationale));
        }
        else
        {
            drivers.Add(MissingDriver("temp_band", "Temperature", 0.25, "Weather upstream unavailable."));
        }

        // ---- Wind ----
        if (weather is not null)
        {
            var wind = weather.WindSpeedMs;
            string dir;
            string windRationale;
            if (wind > 14) { dir = "strong"; windRationale = $"Wind {wind:F0} m/s — exposed sections will be slow."; }
            else if (wind > 8) { dir = "moderate"; windRationale = $"Wind {wind:F0} m/s — manageable, dress for it."; }
            else { dir = "calm"; windRationale = $"Wind {wind:F0} m/s — calm."; }
            drivers.Add(new Driver(
                key: "wind",
                label: "Wind",
                value: wind,
                unit: "m/s",
                weight: 0.15,
                confidence: 0.85,
                direction: dir,
                band: null,
                rationale: windRationale));
        }

        // ---- Grooming freshness ----
        var groomingDriver = BuildGroomingDriver(grooming, input.Activity.Details);
        drivers.Add(groomingDriver);

        // ---- Snow depth ----
        if (gridded is not null)
        {
            var depth = gridded.SnowDepthCm;
            string dir;
            string depthRationale;
            if (depth < 5) { dir = "no base"; depthRationale = $"{depth:F0} cm base — track unlikely to be open."; }
            else if (depth < 15) { dir = "thin"; depthRationale = $"{depth:F0} cm base — thin coverage, expect rocks."; }
            else if (depth < 30) { dir = "skiable"; depthRationale = $"{depth:F0} cm base — skiable but lean."; }
            else { dir = "deep"; depthRationale = $"{depth:F0} cm base — well-covered."; }
            drivers.Add(new Driver(
                key: "snow_depth",
                label: "Snow depth",
                value: depth,
                unit: "cm",
                weight: 0.18,
                confidence: 0.7,
                direction: dir,
                band: null,
                rationale: depthRationale));

            // Below-threshold base → hard warning the user sees front and center.
            if (depth < 5)
            {
                warnings.Add(new Warning(
                    code: "NO_SNOW_BASE",
                    severity: Severity.Caution,
                    title: "No skiable base",
                    body: $"Modelled snow depth at the trail midpoint is only {depth:F0} cm.",
                    sourceUrl: null));
            }
        }

        // ---- Fresh snow 24h ----
        if (gridded is not null)
        {
            var fresh = gridded.FreshSnowLast24hCm;
            string dir;
            string freshRationale;
            if (fresh < 0.5) { dir = "no fresh"; freshRationale = "No fresh accumulation."; }
            else if (fresh < 5) { dir = "light fresh"; freshRationale = $"{fresh:F0} cm fresh — good base reset."; }
            else { dir = "deep fresh"; freshRationale = $"{fresh:F0} cm fresh — slower until groomed."; }
            drivers.Add(new Driver(
                key: "fresh_snow_24h",
                label: "Fresh snow (24h)",
                value: fresh,
                unit: "cm",
                weight: 0.07,
                confidence: 0.7,
                direction: dir,
                band: null,
                rationale: freshRationale));
        }

        // ---- Recent nearby observations (own-data) ----
        var ownDriver = BuildOwnDataDriver(input);
        if (ownDriver is not null) drivers.Add(ownDriver);

        // ---- Snowpack / regObs slide activity ----
        if (snowpack is { RecentSlideActivity: > 0 } sp)
        {
            warnings.Add(new Warning(
                code: "NEARBY_SLIDES",
                severity: Severity.Info,
                title: "Recent avalanche activity nearby",
                body: $"{sp.RecentSlideActivity} slide observation(s) within 10 km in the last week — not a direct xc-ski concern, but worth knowing for any cross-country / backcountry route.",
                sourceUrl: null));
        }

        // ---- Composite score ----
        var (score, confidence) = CompositeScoreAndConfidence(drivers);
        var rationale = BuildRationale(drivers, weather, gridded, grooming, input.Activity.Details);

        // ---- Suggested windows ----
        var windows = input.QueryContext.IncludeWindows
            ? BuildSuggestedWindows(weather, input.QueryContext.At)
            : Array.Empty<TimeWindow>();

        // ---- Kind-specific extras: wax band ----
        var waxBand = PredictedWaxBand(weather);
        var kindSlices = new Dictionary<string, JsonElement>();
        if (waxBand is not null)
        {
            var payload = JsonSerializer.SerializeToElement(new
            {
                predictedWax = waxBand,
            });
            kindSlices["xc_ski"] = payload;
        }

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
            bands: bands,
            warnings: warnings,
            suggestedWindows: windows,
            kindSlices: kindSlices,
            provenance: input.ToProvenance(sw.ElapsedMilliseconds));
    }

    private static Driver BuildGroomingDriver(GroomingSlice? live, XcSkiDetails details)
    {
        if (live is not null)
        {
            string dir;
            string groomingRationale;
            if (live.HoursAgo < 6) { dir = "freshly groomed"; groomingRationale = $"Groomed {live.HoursAgo}h ago."; }
            else if (live.HoursAgo < 36) { dir = "recently groomed"; groomingRationale = $"Groomed {live.HoursAgo}h ago."; }
            else if (live.HoursAgo < 72) { dir = "stale"; groomingRationale = $"Groomed {live.HoursAgo}h ago — track is wearing."; }
            else { dir = "very stale"; groomingRationale = $"Groomed {live.HoursAgo / 24}d ago — expect ruts."; }
            return new Driver(
                key: "live_grooming_age",
                label: "Grooming freshness",
                value: live.HoursAgo,
                unit: "h",
                weight: 0.25,
                confidence: 0.9,
                direction: dir,
                band: null,
                rationale: groomingRationale);
        }

        var (dirStored, rStored) = details.GroomingStatus switch
        {
            GroomingStatus.Today => ("today (stored)", "Groomed today (stored estimate)."),
            GroomingStatus.Yesterday => ("yesterday (stored)", "Groomed yesterday (stored estimate)."),
            GroomingStatus.OlderThanTwoDays => (">2d (stored)", "Last groomed >2 days ago (stored estimate)."),
            GroomingStatus.NeverGroomed => ("never groomed", "Backcountry / never-groomed track."),
            _ => ("unknown", "Grooming status unknown — no live feed."),
        };
        return new Driver(
            key: "live_grooming_age",
            label: "Grooming freshness",
            value: null,
            unit: null,
            weight: 0.20,
            confidence: 0.45,
            direction: dirStored,
            band: null,
            rationale: rStored);
    }

    private static Driver? BuildOwnDataDriver(SynthesisInput<XcSkiActivity> input)
    {
        var obs = input.RecentObservations;
        if (obs.Count == 0) return null;
        var avgRating = obs.Where(o => o.Rating.HasValue).Select(o => (double)o.Rating!.Value).DefaultIfEmpty(0).Average();
        var rationale = avgRating > 0
            ? $"{obs.Count} recent observation(s); average rating {avgRating:F1}/5."
            : $"{obs.Count} recent observation(s) on this track.";
        return new Driver(
            key: "nearby_obs",
            label: "Recent observations",
            value: obs.Count,
            unit: null,
            weight: 0.10,
            confidence: Math.Min(1.0, obs.Count / 5.0),
            direction: avgRating >= 4 ? "positive" : avgRating > 0 && avgRating < 3 ? "negative" : null,
            band: null,
            rationale: rationale);
    }

    private static Driver MissingDriver(string key, string label, double weight, string rationale) =>
        new(
            key: key,
            label: label,
            value: null,
            unit: null,
            weight: weight,
            confidence: 0.0,
            direction: null,
            band: null,
            rationale: rationale);

    private static (int? Score, ScoreConfidence Confidence) CompositeScoreAndConfidence(
        IReadOnlyList<Driver> drivers)
    {
        double weightedSum = 0;
        double weightTotal = 0;
        double confidenceWeightedSum = 0;
        foreach (var d in drivers)
        {
            if (d.Confidence <= 0) continue;
            // Map each driver's qualitative rationale back to a 0..1 score
            // via the `value` channel where possible. Without a numeric
            // score baked into the Driver record itself, use the weight
            // as the contribution band and confidence as gain.
            var rawScore = DriverScoreHint(d);
            weightedSum += rawScore * d.Weight * d.Confidence;
            weightTotal += d.Weight * d.Confidence;
            confidenceWeightedSum += d.Confidence * d.Weight;
        }
        if (weightTotal < 1e-6) return (null, ScoreConfidence.Low);
        var composite = (int)Math.Round(weightedSum / weightTotal * 100);
        composite = Math.Clamp(composite, 0, 100);
        var avgConfidence = confidenceWeightedSum /
            Math.Max(1e-6, drivers.Sum(d => d.Weight));
        var band = avgConfidence > 0.75 ? ScoreConfidence.High
                  : avgConfidence > 0.45 ? ScoreConfidence.Medium
                  : ScoreConfidence.Low;
        return (composite, band);
    }

    /// <summary>
    /// Per-driver score hint. The synthesizer used the driver's
    /// <c>direction</c> string as a coarse classifier when computing
    /// each Driver record; this maps the same buckets back to 0..1 so the
    /// composite stays consistent. Centralised here rather than computed
    /// inline so future driver additions can register their own mapping.
    /// </summary>
    private static double DriverScoreHint(Driver d) => d.Key switch
    {
        "temp_band" => d.Direction switch
        {
            "above freezing" => 0.05,
            "near freezing" => 0.45,
            "very cold" => 0.55,
            "cold and dry" => 0.95,
            _ => 0.5,
        },
        "wind" => d.Direction switch
        {
            "strong" => 0.15,
            "moderate" => 0.55,
            "calm" => 0.9,
            _ => 0.6,
        },
        "live_grooming_age" => d.Direction switch
        {
            "freshly groomed" => 0.95,
            "recently groomed" => 0.8,
            "stale" => 0.45,
            "very stale" => 0.2,
            "today (stored)" => 0.85,
            "yesterday (stored)" => 0.7,
            ">2d (stored)" => 0.25,
            "never groomed" => 0.4,
            _ => 0.5,
        },
        "snow_depth" => d.Direction switch
        {
            "no base" => 0.05,
            "thin" => 0.35,
            "skiable" => 0.7,
            "deep" => 0.95,
            _ => 0.5,
        },
        "fresh_snow_24h" => d.Direction switch
        {
            "no fresh" => 0.5,
            "light fresh" => 0.85,
            "deep fresh" => 0.7,
            _ => 0.5,
        },
        "nearby_obs" => d.Direction switch
        {
            "positive" => 0.85,
            "negative" => 0.3,
            _ => 0.6,
        },
        _ => 0.5,
    };

    private static string BuildRationale(
        IReadOnlyList<Driver> drivers, WeatherSlice? weather,
        GriddedSnowSlice? gridded, GroomingSlice? grooming, XcSkiDetails details)
    {
        var parts = new List<string>();
        if (gridded is not null && gridded.SnowDepthCm < 5)
            parts.Add($"only {gridded.SnowDepthCm:F0} cm base");
        if (weather is not null)
        {
            if (weather.AirTemperatureCelsius > 3) parts.Add("track is melting");
            else if (weather.AirTemperatureCelsius < -15) parts.Add("very cold — heavy wax");
        }
        if (grooming is not null && grooming.HoursAgo > 72)
            parts.Add($"groomed {grooming.HoursAgo / 24}d ago");
        if (grooming is not null && grooming.HoursAgo < 6)
            parts.Add("freshly groomed");
        if (gridded is not null && gridded.FreshSnowLast24hCm > 5)
            parts.Add($"{gridded.FreshSnowLast24hCm:F0} cm fresh");
        if (parts.Count == 0) return "Cold and dry — good track conditions.";
        return $"Conditions: {string.Join(", ", parts)}.";
    }

    private static IReadOnlyList<TimeWindow> BuildSuggestedWindows(WeatherSlice? weather, DateTimeOffset now)
    {
        // Without a full forecast band from met.no we can't infer windows
        // with conviction. Surface a single "before midday" window when
        // the temp is climbing through 0°C: rising-temp days favour
        // morning starts. This is a placeholder until the orchestrator
        // pulls the hourly forecast directly.
        if (weather is null) return Array.Empty<TimeWindow>();
        if (weather.AirTemperatureCelsius is < -1 or > 5) return Array.Empty<TimeWindow>();
        var date = now.UtcDateTime.Date;
        var start = new DateTimeOffset(date.AddHours(7), TimeSpan.Zero);
        var end = new DateTimeOffset(date.AddHours(11), TimeSpan.Zero);
        return new[]
        {
            new TimeWindow(
                start: start, end: end,
                quality: WindowQuality.Good,
                label: "Best before midday",
                reason: "Temperature climbs through 0 °C today — go before the surface softens."),
        };
    }

    /// <summary>
    /// Coarse wax/glide recommendation from current air temperature.
    /// The user-facing string follows the Norwegian classic-wax convention.
    /// </summary>
    private static string? PredictedWaxBand(WeatherSlice? weather)
    {
        if (weather is null) return null;
        var t = weather.AirTemperatureCelsius;
        return t switch
        {
            > 0 => "Klister (above freezing)",
            > -2 => "Soft red (-2 to 0 °C)",
            > -8 => "Blue (-2 to -8 °C)",
            > -15 => "Hard blue / green (-8 to -15 °C)",
            _ => "Polar / hard green (below -15 °C)",
        };
    }
}
