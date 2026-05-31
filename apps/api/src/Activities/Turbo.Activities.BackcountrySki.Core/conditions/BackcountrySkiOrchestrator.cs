using System.Text.Json;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.BackcountrySki.domain;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;
using BcAspectShare = Turboapi.Activities.BackcountrySki.value.AspectShare;

namespace Turboapi.Activities.BackcountrySki.conditions;

/// <summary>
/// BackcountrySki orchestrator. The highest-stakes kind — safety matters.
/// Composes Varsom regional bulletin + recent regObs observations + the
/// route's aspect histogram (from <see cref="ActivityGeoContext"/> or the
/// activity's stored <see cref="BackcountrySkiDetails.AspectMix"/>) +
/// fresh-snow-and-wind loading on lee aspects.
///
/// Drivers:
/// <list type="bullet">
///   <item><c>avalanche_danger</c> — Varsom level (1–5)</item>
///   <item><c>wind_loading</c> — precip + wind × dominant-aspect → lee-loading score</item>
///   <item><c>recent_slide_activity</c> — regObs slides reported within 10 km in the lookback window</item>
///   <item><c>weak_layers</c> — presence of persistent / weak-layer reports</item>
///   <item><c>route_exposure</c> — ATES rating + slope histogram</item>
///   <item><c>fresh_snow_24h</c> — seNorge accumulation</item>
/// </list>
///
/// Warnings: <c>LEVEL_4_OR_5_AVOID</c>, <c>WIND_LOADING_ON_LEE</c>,
/// <c>PERSISTENT_WEAK_LAYER</c>, <c>EXCEEDS_USER_PREFERENCE</c>.
/// </summary>
public sealed class BackcountrySkiOrchestrator
    : ActivityOrchestratorPipeline<BackcountrySkiActivity, ActivityAnalysis>
{
    public const string ProviderKeyWeather = "weather";
    public const string ProviderKeyAvalanche = "avalanche";
    public const string ProviderKeySnowpack = "snowpack";
    public const string ProviderKeyGriddedSnow = "gridded_snow";

    private readonly IWeatherProvider _weather;
    private readonly IAvalancheProvider? _avalanche;
    private readonly ISnowpackProvider _snowpack;
    private readonly IGriddedSnowProvider _griddedSnow;
    private readonly TimeProvider _clock;

    public BackcountrySkiOrchestrator(
        IActivityGeoContextService geoContext,
        IActivityObservationStore observations,
        IActivityVisitStore visits,
        IConditionsSnapshotStore snapshots,
        IActivitySummaryScoreWriter scoreWriter,
        ILogger<BackcountrySkiOrchestrator> logger,
        IWeatherProvider weather,
        ISnowpackProvider snowpack,
        IGriddedSnowProvider griddedSnow,
        IAvalancheProvider? avalanche = null,
        TimeProvider? clock = null)
        : base(geoContext, observations, visits, snapshots, logger, scoreWriter)
    {
        _weather = weather;
        _avalanche = avalanche;
        _snowpack = snowpack;
        _griddedSnow = griddedSnow;
        _clock = clock ?? TimeProvider.System;
    }

    protected override string KindKey => "backcountry_ski";

    protected override NetTopologySuite.Geometries.Geometry ExtractGeometry(BackcountrySkiActivity activity)
        => activity.Route;

    protected override IReadOnlyList<ProviderTask> PlanFanOut(
        BackcountrySkiActivity activity, ActivityGeoContext? geoContext, QueryContext queryContext)
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

        if (_avalanche is not null && activity.Details.VarsomRegionId is { } region)
        {
            tasks.Add(new ProviderTask(
                ProviderKeyAvalanche,
                async ct => await _avalanche.GetAsync(region, at, ct).ConfigureAwait(false),
                extractObservedAt: s => s is AvalancheSlice a ? a.ValidFor : null));
        }

        return tasks;
    }

    protected override ActivityAnalysis Synthesize(SynthesisInput<BackcountrySkiActivity> input)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        var weather = input.Get<WeatherSlice>(ProviderKeyWeather);
        var avalanche = input.Get<AvalancheSlice>(ProviderKeyAvalanche);
        var snowpack = input.Get<SnowpackSlice>(ProviderKeySnowpack);
        var gridded = input.Get<GriddedSnowSlice>(ProviderKeyGriddedSnow);

        var details = input.Activity.Details;
        var drivers = new List<Driver>();
        var warnings = new List<Warning>();

        // ---- Avalanche danger (the dominant driver) ----
        if (avalanche is not null)
        {
            var lvl = avalanche.DangerLevel;
            string dir = lvl switch
            {
                5 => "extreme",
                4 => "high",
                3 => "considerable",
                2 => "moderate",
                _ => "low",
            };
            drivers.Add(new Driver(
                key: "avalanche_danger",
                label: "Avalanche danger",
                value: lvl,
                unit: "/5",
                weight: 0.40,
                confidence: 0.95,
                direction: dir,
                band: null,
                rationale: avalanche.Summary));

            if (lvl >= 4)
            {
                warnings.Add(new Warning(
                    code: "LEVEL_4_OR_5_AVOID",
                    severity: Severity.Danger,
                    title: $"Avalanche danger level {lvl}",
                    body: lvl == 5
                        ? "Avoid all avalanche terrain."
                        : "Travel in avalanche terrain not recommended.",
                    sourceUrl: "https://varsom.no/"));
            }
            if (details.PreferredAvalancheMaxLevel is { } maxLevel && lvl > maxLevel)
            {
                warnings.Add(new Warning(
                    code: "EXCEEDS_USER_PREFERENCE",
                    severity: Severity.Caution,
                    title: "Above your preferred danger ceiling",
                    body: $"Forecast level {lvl} exceeds your stored max ({maxLevel}).",
                    sourceUrl: null));
            }
        }
        else
        {
            drivers.Add(MissingDriver(
                "avalanche_danger", "Avalanche danger", 0.40,
                "No avalanche bulletin available — verify Varsom before going."));
        }

        // ---- Wind loading on lee aspects ----
        if (weather is not null)
        {
            var precip6h = weather.PrecipitationNext6hMm ?? 0;
            var wind = weather.WindSpeedMs;
            var loadingScore = WindLoadingScore(wind, precip6h);
            string dir = loadingScore < 0.3 ? "severe" : loadingScore < 0.6 ? "elevated" : "low";
            drivers.Add(new Driver(
                key: "wind_loading",
                label: "Wind loading",
                value: wind,
                unit: "m/s",
                weight: 0.20,
                confidence: 0.85,
                direction: dir,
                band: null,
                rationale: precip6h > 10 && wind > 12
                    ? $"{precip6h:F0} mm / 6h + wind {wind:F0} m/s — fresh slabs on lee aspects."
                    : wind > 18
                        ? $"Ridge wind {wind:F0} m/s — exposed ridges dangerous."
                        : $"Wind {wind:F0} m/s, precip {precip6h:F0} mm / 6h."));

            if (precip6h > 10 && wind > 12)
            {
                var leeAspect = LeeAspectOf(weather.WindFromDegrees);
                warnings.Add(new Warning(
                    code: "WIND_LOADING_ON_LEE",
                    severity: Severity.Caution,
                    title: "Wind-loading on lee aspects",
                    body: $"Fresh snow + strong wind from {AspectName(weather.WindFromDegrees)} — expect loaded slabs on {leeAspect} aspects.",
                    sourceUrl: null));
            }
        }

        // ---- Recent slide activity (regObs) ----
        if (snowpack is not null)
        {
            var slides = snowpack.RecentSlideActivity;
            double confidence = Math.Min(1.0, snowpack.ObservationCount / 10.0);
            string dir = slides switch
            {
                0 => "none reported",
                1 => "isolated",
                _ => "multiple",
            };
            drivers.Add(new Driver(
                key: "recent_slide_activity",
                label: "Recent slide activity",
                value: slides,
                unit: null,
                weight: 0.15,
                confidence: confidence,
                direction: dir,
                band: null,
                rationale: $"{slides} slide observation(s) in the last 7 days within ~10 km. {snowpack.ObservationCount} total observation(s)."));
        }

        // ---- Persistent weak layer ----
        if (snowpack is not null && snowpack.WeakLayers.Count > 0)
        {
            var persistent = snowpack.WeakLayers
                .Any(l => l.Contains("persistent", StringComparison.OrdinalIgnoreCase)
                       || l.Contains("buried", StringComparison.OrdinalIgnoreCase));
            drivers.Add(new Driver(
                key: "weak_layers",
                label: "Weak layers",
                value: snowpack.WeakLayers.Count,
                unit: null,
                weight: 0.10,
                confidence: Math.Min(1.0, snowpack.ObservationCount / 8.0),
                direction: persistent ? "persistent" : "near-surface",
                band: null,
                rationale: $"Reported layers: {string.Join(", ", snowpack.WeakLayers)}."));

            if (persistent)
            {
                warnings.Add(new Warning(
                    code: "PERSISTENT_WEAK_LAYER",
                    severity: Severity.Caution,
                    title: "Persistent weak layer reported",
                    body: "Recent regObs reports include a persistent / buried weak layer in the area. These layers stay dangerous for days to weeks.",
                    sourceUrl: "https://regobs.no/"));
            }
        }

        // ---- Route exposure (ATES + aspect mix) ----
        var atesLabel = details.AtesRating switch
        {
            AtesRating.Complex => "complex",
            AtesRating.Challenging => "challenging",
            AtesRating.Simple => "simple",
            _ => "unrated",
        };
        drivers.Add(new Driver(
            key: "route_exposure",
            label: "Route exposure",
            value: (int)details.AtesRating,
            unit: null,
            weight: 0.10,
            confidence: details.AtesRating == AtesRating.Unrated ? 0.3 : 0.8,
            direction: atesLabel,
            band: null,
            rationale: details.AtesRating == AtesRating.Unrated
                ? "Route not yet ATES-rated."
                : $"ATES {atesLabel}; dominant aspect {details.DominantAspect?.ToString() ?? "n/a"}."));

        // ---- Fresh snow 24h ----
        if (gridded is not null)
        {
            var fresh = gridded.FreshSnowLast24hCm;
            string dir = fresh < 1 ? "no fresh" : fresh < 10 ? "moderate fresh" : "heavy fresh";
            drivers.Add(new Driver(
                key: "fresh_snow_24h",
                label: "Fresh snow (24h)",
                value: fresh,
                unit: "cm",
                weight: 0.05,
                confidence: 0.7,
                direction: dir,
                band: null,
                rationale: fresh < 1
                    ? "No fresh accumulation in the last 24h."
                    : $"{fresh:F0} cm fresh in the last 24h — combined with wind, watch for loaded slabs."));
        }

        var (score, confidence2) = CompositeScoreAndConfidence(drivers);
        var rationale = BuildRationale(drivers, avalanche, weather, snowpack);

        // Suggested windows — for backcountry the main timing variable is
        // solar warming on south-facing slopes. Without solar geometry
        // wired up, surface a single morning window when daytime is
        // expected to thaw (above-freezing forecast).
        var windows = input.QueryContext.IncludeWindows
            ? BuildSuggestedWindows(weather, input.QueryContext.At)
            : Array.Empty<TimeWindow>();

        // Kind-specific slice — per-aspect loading + aspect mix.
        var kindSlices = new Dictionary<string, JsonElement>();
        var aspectLoading = AspectLoadingPayload(weather, details.AspectMix);
        if (aspectLoading is not null)
        {
            kindSlices["backcountry_ski"] = JsonSerializer.SerializeToElement(aspectLoading);
        }

        sw.Stop();
        return new ActivityAnalysis(
            activityId: input.ActivityId,
            kind: KindKey,
            validAt: weather?.ValidAt ?? input.QueryContext.At,
            fetchedAt: _clock.GetUtcNow(),
            score: score,
            confidence: confidence2,
            rationale: rationale,
            drivers: drivers,
            bands: Array.Empty<ForecastBand>(),
            warnings: warnings,
            suggestedWindows: windows,
            kindSlices: kindSlices,
            provenance: input.ToProvenance(sw.ElapsedMilliseconds));
    }

    private static double WindLoadingScore(double wind, double precip6h)
    {
        if (precip6h > 10 && wind > 12) return 0.15;
        if (wind > 18) return 0.20;
        if (wind > 12) return 0.50;
        if (precip6h > 10) return 0.55;
        return 0.85;
    }

    private static (int? Score, ScoreConfidence Confidence) CompositeScoreAndConfidence(
        IReadOnlyList<Driver> drivers)
    {
        double weightedSum = 0;
        double weightTotal = 0;
        double avgConfidenceNumerator = 0;
        double weightSumForConfidence = 0;
        foreach (var d in drivers)
        {
            if (d.Confidence <= 0) continue;
            var raw = DriverScoreHint(d);
            weightedSum += raw * d.Weight * d.Confidence;
            weightTotal += d.Weight * d.Confidence;
            avgConfidenceNumerator += d.Confidence * d.Weight;
            weightSumForConfidence += d.Weight;
        }
        if (weightTotal < 1e-6) return (null, ScoreConfidence.Low);
        var composite = (int)Math.Round(weightedSum / weightTotal * 100);
        composite = Math.Clamp(composite, 0, 100);
        var avgConfidence = avgConfidenceNumerator / Math.Max(1e-6, weightSumForConfidence);
        var band = avgConfidence > 0.75 ? ScoreConfidence.High
                  : avgConfidence > 0.45 ? ScoreConfidence.Medium
                  : ScoreConfidence.Low;
        return (composite, band);
    }

    private static double DriverScoreHint(Driver d) => d.Key switch
    {
        "avalanche_danger" => d.Direction switch
        {
            "extreme" => 0.05,
            "high" => 0.15,
            "considerable" => 0.45,
            "moderate" => 0.75,
            _ => 0.9,
        },
        "wind_loading" => d.Direction switch
        {
            "severe" => 0.15,
            "elevated" => 0.45,
            _ => 0.85,
        },
        "recent_slide_activity" => d.Direction switch
        {
            "multiple" => 0.20,
            "isolated" => 0.55,
            _ => 0.85,
        },
        "weak_layers" => d.Direction switch
        {
            "persistent" => 0.25,
            "near-surface" => 0.55,
            _ => 0.75,
        },
        "route_exposure" => d.Direction switch
        {
            "complex" => 0.50,
            "challenging" => 0.65,
            "simple" => 0.85,
            _ => 0.60,
        },
        "fresh_snow_24h" => d.Direction switch
        {
            "no fresh" => 0.85,
            "moderate fresh" => 0.65,
            "heavy fresh" => 0.40,
            _ => 0.75,
        },
        _ => 0.5,
    };

    private static Driver MissingDriver(string key, string label, double weight, string rationale) =>
        new(key, label, null, null, weight, 0.0, null, null, rationale);

    private static string BuildRationale(
        IReadOnlyList<Driver> drivers,
        AvalancheSlice? avalanche,
        WeatherSlice? weather,
        SnowpackSlice? snowpack)
    {
        if (avalanche is null) return "No avalanche bulletin — verify Varsom before going.";
        var parts = new List<string> { $"Varsom level {avalanche.DangerLevel}" };
        if (weather is not null)
        {
            var precip6h = weather.PrecipitationNext6hMm ?? 0;
            if (precip6h > 10 && weather.WindSpeedMs > 12)
                parts.Add($"{precip6h:F0} mm/6h with {weather.WindSpeedMs:F0} m/s wind");
            else if (weather.WindSpeedMs > 18)
                parts.Add($"ridge wind {weather.WindSpeedMs:F0} m/s");
        }
        if (snowpack?.RecentSlideActivity > 0)
            parts.Add($"{snowpack.RecentSlideActivity} slide(s) reported within 10 km");
        return string.Join("; ", parts) + ".";
    }

    private static IReadOnlyList<TimeWindow> BuildSuggestedWindows(WeatherSlice? weather, DateTimeOffset now)
    {
        if (weather is null) return Array.Empty<TimeWindow>();
        if (weather.AirTemperatureCelsius < 0) return Array.Empty<TimeWindow>();
        var date = now.UtcDateTime.Date;
        var start = new DateTimeOffset(date.AddHours(8), TimeSpan.Zero);
        var end = new DateTimeOffset(date.AddHours(11), TimeSpan.Zero);
        return new[]
        {
            new TimeWindow(
                start: start, end: end,
                quality: WindowQuality.Good,
                label: "Ski south aspects before 11:00",
                reason: "Above-freezing forecast — solar warming destabilises south aspects after midday."),
        };
    }

    private static string AspectName(double windFromDegrees)
    {
        var idx = (int)Math.Round(((windFromDegrees % 360) / 45)) % 8;
        return new[] { "N", "NE", "E", "SE", "S", "SW", "W", "NW" }[idx];
    }

    private static string LeeAspectOf(double windFromDegrees)
    {
        var leeBearing = (windFromDegrees + 180) % 360;
        return AspectName(leeBearing);
    }

    private static object? AspectLoadingPayload(WeatherSlice? weather, IReadOnlyList<BcAspectShare> mix)
    {
        if (weather is null || mix.Count == 0) return null;
        var leeAspect = LeeAspectOf(weather.WindFromDegrees);
        var wind = weather.WindSpeedMs;
        var precip6h = weather.PrecipitationNext6hMm ?? 0;
        var loadingFactor = precip6h > 10 && wind > 12 ? 0.85
                          : wind > 18 ? 0.6
                          : wind > 12 ? 0.3
                          : 0.0;

        var perAspect = mix.Select(share => new
        {
            aspect = share.Aspect.ToString(),
            fraction = share.Fraction,
            loadedFractionOfFraction = share.Aspect.ToString() == leeAspect ? loadingFactor : loadingFactor * 0.25,
        });

        return new
        {
            windFromDegrees = weather.WindFromDegrees,
            leeAspect,
            loadingFactor,
            perAspect,
        };
    }
}
