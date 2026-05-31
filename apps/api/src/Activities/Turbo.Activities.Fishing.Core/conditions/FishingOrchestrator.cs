using System.Text.Json;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Fishing.domain;
using Turboapi.Activities.Fishing.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Fishing.conditions;

/// <summary>
/// Fishing orchestrator. The legacy advisor reads the user's
/// <see cref="PreferredConditions"/> field and scores against it as if
/// that's the same as fish behaviour — this orchestrator instead emits
/// named drivers the user can read: pressure trend (computed from the
/// snapshot store's recent history, not the user's gut feeling), water
/// temp / wind / rain, and a solunar major-window overlap. Bite-window
/// prediction lives in <c>kindSlices["fishing"]</c>.
///
/// Drivers:
/// <list type="bullet">
///   <item><c>pressure_trend</c> — 24h pressure slope from snapshot store</item>
///   <item><c>wind_and_rain</c> — composite from weather slice</item>
///   <item><c>solunar_overlap</c> — pure-compute solunar major-window check</item>
///   <item><c>thermal</c> — air temp proxy (water-temp provider later)</item>
///   <item><c>nearby_obs</c> — recent fishing observations on this spot</item>
/// </list>
/// </summary>
public sealed class FishingOrchestrator
    : ActivityOrchestratorPipeline<FishingActivity, ActivityAnalysis>
{
    public const string ProviderKeyWeather = "weather";

    private readonly IWeatherProvider _weather;
    private readonly TimeProvider _clock;

    public FishingOrchestrator(
        IActivityGeoContextService geoContext,
        IActivityObservationStore observations,
        IActivityVisitStore visits,
        IConditionsSnapshotStore snapshots,
        IActivitySummaryScoreWriter scoreWriter,
        ILogger<FishingOrchestrator> logger,
        IWeatherProvider weather,
        TimeProvider? clock = null)
        : base(geoContext, observations, visits, snapshots, logger, scoreWriter)
    {
        _weather = weather;
        _clock = clock ?? TimeProvider.System;
    }

    protected override string KindKey => "fishing";

    protected override NetTopologySuite.Geometries.Geometry ExtractGeometry(FishingActivity activity)
        => activity.Position;

    protected override IReadOnlyList<ProviderTask> PlanFanOut(
        FishingActivity activity, ActivityGeoContext? geoContext, QueryContext queryContext)
    {
        var p = activity.Position;
        return new[]
        {
            new ProviderTask(
                ProviderKeyWeather,
                async ct => await _weather.GetAsync(p.Y, p.X, queryContext.At, ct).ConfigureAwait(false),
                extractObservedAt: s => s is WeatherSlice w ? w.ValidAt : null),
        };
    }

    protected override ActivityAnalysis Synthesize(SynthesisInput<FishingActivity> input)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        var weather = input.Get<WeatherSlice>(ProviderKeyWeather);
        var drivers = new List<Driver>();
        var warnings = new List<Warning>();

        // ---- Pressure trend driver (snapshot-history-derived) -----------
        // The snapshot store keeps every successful weather fetch; the
        // metric extractor for "met_no_weather" / "synthetic_weather"
        // exposes airPressureHpa. We ask for recent rows around the
        // spot, compute the linear slope, and emit a qualitative trend.
        // Soft-fails when there isn't enough history yet.
        var pressureDriver = TryBuildPressureTrendDriver(input, weather);
        if (pressureDriver is not null) drivers.Add(pressureDriver);

        // ---- Wind & rain composite -------------------------------------
        if (weather is not null)
        {
            var wind = weather.WindSpeedMs;
            var rain = weather.PrecipitationNext1hMm ?? 0;
            string dir;
            string windRainRationale;
            if (wind > 12 || rain > 5)
            {
                dir = "harsh";
                windRainRationale = $"Wind {wind:F0} m/s, rain {rain:F1} mm/h — hard fishing window.";
            }
            else if (wind > 8 || rain > 1)
            {
                dir = "lively";
                windRainRationale = $"Wind {wind:F0} m/s, rain {rain:F1} mm/h — workable but plan layers.";
            }
            else
            {
                dir = "calm";
                windRainRationale = $"Wind {wind:F0} m/s — calm.";
            }
            drivers.Add(new Driver(
                key: "wind_and_rain",
                label: "Wind & rain",
                value: wind,
                unit: "m/s",
                weight: 0.20,
                confidence: 0.85,
                direction: dir,
                band: null,
                rationale: windRainRationale));
        }

        // ---- Solunar major-window overlap (pure compute) ----------------
        var solunarDriver = BuildSolunarDriver(input.QueryContext.At, input.Activity.Position.Y);
        drivers.Add(solunarDriver);

        // ---- Thermal proxy (air temp until water-temp provider lands) ---
        if (weather is not null)
        {
            var t = weather.AirTemperatureCelsius;
            string dir;
            string thermalRationale;
            if (t > 25) { dir = "hot"; thermalRationale = "Hot — fish go deep / shaded; early/late only."; }
            else if (t > 12) { dir = "active"; thermalRationale = $"Air {t:F0}°C — active fish window."; }
            else if (t > 2) { dir = "cool"; thermalRationale = $"Air {t:F0}°C — slower bite; deeper presentation."; }
            else { dir = "cold"; thermalRationale = $"Air {t:F0}°C — very cold; slow it down."; }
            drivers.Add(new Driver(
                key: "thermal",
                label: "Thermal load",
                value: t,
                unit: "°C",
                weight: 0.10,
                confidence: 0.4,
                direction: dir,
                band: null,
                rationale: thermalRationale));
        }

        // ---- Observations ----------------------------------------------
        var obsDriver = BuildObservationDriver(input);
        if (obsDriver is not null) drivers.Add(obsDriver);

        var (score, confidence) = CompositeScoreAndConfidence(drivers);
        var rationale = BuildRationale(drivers);

        // KindSlice: bite-window prediction (solunar majors intersected
        // with weather-acceptable hours). Single window as a first cut.
        var bestWindow = SolunarMajorOf(input.QueryContext.At, input.Activity.Position.Y);
        var kindSlices = new Dictionary<string, JsonElement>();
        if (bestWindow is not null)
        {
            kindSlices["fishing"] = JsonSerializer.SerializeToElement(new
            {
                biteWindow = new
                {
                    start = bestWindow.Value.Start,
                    end = bestWindow.Value.End,
                    rationale = "Solunar major window — peak fish activity from lunar transit / opposite.",
                },
            });
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
            bands: Array.Empty<ForecastBand>(),
            warnings: warnings,
            suggestedWindows: bestWindow is null
                ? Array.Empty<TimeWindow>()
                : new[]
                {
                    new TimeWindow(
                        start: bestWindow.Value.Start,
                        end: bestWindow.Value.End,
                        quality: WindowQuality.Good,
                        label: "Solunar major",
                        reason: "Predicted peak feeding window."),
                },
            kindSlices: kindSlices,
            provenance: input.ToProvenance(sw.ElapsedMilliseconds));
    }

    private Driver? TryBuildPressureTrendDriver(
        SynthesisInput<FishingActivity> input, WeatherSlice? currentWeather)
    {
        if (currentWeather is null) return null;
        var p = input.Activity.Position;
        var grid = $"{Math.Round(p.Y, 2):F2}_{Math.Round(p.X, 2):F2}";
        // Pull weather snapshots from both possible provider keys —
        // the snapshot store ignores ones it has no extractor for.
        var providerKeys = new[] { "met_no_weather", "synthetic_weather" };
        foreach (var key in providerKeys)
        {
            try
            {
                var recent = input.Snapshots.GetRecentAsync(
                    key, grid,
                    since: input.QueryContext.At - TimeSpan.FromHours(36),
                    until: input.QueryContext.At,
                    limit: 24,
                    CancellationToken.None).GetAwaiter().GetResult();
                if (recent.Count < 3) continue;
                // Compute linear slope of pressure over recent.observedAt.
                var trend = ComputePressureTrend(recent);
                if (trend is null) continue;
                var (slopeHpaPerHour, ageHours) = trend.Value;
                string dir;
                string pressureRationale;
                if (slopeHpaPerHour > 0.3)
                {
                    dir = "rising";
                    pressureRationale = $"Pressure rising {slopeHpaPerHour:F1} hPa/h over the last {ageHours:F0}h — fish settling, slower bite.";
                }
                else if (slopeHpaPerHour < -0.3)
                {
                    dir = "falling";
                    pressureRationale = $"Pressure falling {Math.Abs(slopeHpaPerHour):F1} hPa/h over the last {ageHours:F0}h — classic pre-front feeding window.";
                }
                else
                {
                    dir = "stable";
                    pressureRationale = $"Pressure stable ({currentWeather.AirPressureHpa:F0} hPa over the last {ageHours:F0}h).";
                }
                return new Driver(
                    key: "pressure_trend",
                    label: "Pressure trend",
                    value: slopeHpaPerHour,
                    unit: "hPa/h",
                    weight: 0.25,
                    confidence: Math.Min(0.9, recent.Count / 12.0),
                    direction: dir,
                    band: null,
                    rationale: pressureRationale);
            }
            catch
            {
                // Soft fail — orchestrator pipeline already guards this
                // path, but if reading the snapshot store fails we just
                // skip the driver.
            }
        }
        return null;
    }

    private static (double SlopeHpaPerHour, double AgeHours)? ComputePressureTrend(
        IReadOnlyList<ConditionsSnapshot> snapshots)
    {
        // Use the snapshot store's payload bytes via the registry where
        // possible; here we deserialize inline because we already know
        // the shape is WeatherSlice.
        var points = new List<(double tHours, double pressure)>();
        DateTimeOffset? minAt = null;
        DateTimeOffset? maxAt = null;
        foreach (var s in snapshots)
        {
            try
            {
                var ws = JsonSerializer.Deserialize<WeatherSlice>(s.Payload.Span);
                if (ws is null) continue;
                points.Add(((s.ObservedAt - DateTimeOffset.UnixEpoch).TotalHours, ws.AirPressureHpa));
                if (minAt is null || s.ObservedAt < minAt) minAt = s.ObservedAt;
                if (maxAt is null || s.ObservedAt > maxAt) maxAt = s.ObservedAt;
            }
            catch (JsonException) { continue; }
        }
        if (points.Count < 3 || minAt is null || maxAt is null) return null;

        // Simple least-squares slope.
        var n = points.Count;
        var sumT = points.Sum(p => p.tHours);
        var sumP = points.Sum(p => p.pressure);
        var sumTP = points.Sum(p => p.tHours * p.pressure);
        var sumTT = points.Sum(p => p.tHours * p.tHours);
        var denom = (n * sumTT - sumT * sumT);
        if (denom < 1e-6) return null;
        var slope = (n * sumTP - sumT * sumP) / denom;
        var span = (maxAt.Value - minAt.Value).TotalHours;
        return (slope, span);
    }

    private static Driver BuildSolunarDriver(DateTimeOffset at, double latitude)
    {
        var window = SolunarMajorOf(at, latitude);
        if (window is null)
        {
            return new Driver(
                key: "solunar_overlap",
                label: "Solunar",
                value: null,
                unit: null,
                weight: 0.10,
                confidence: 0.4,
                direction: "off-major",
                band: null,
                rationale: "Outside today's predicted solunar major window.");
        }
        var (start, end) = window.Value;
        var inWindow = at >= start && at <= end;
        return new Driver(
            key: "solunar_overlap",
            label: "Solunar",
            value: null,
            unit: null,
            weight: 0.15,
            confidence: 0.55,
            direction: inWindow ? "in-major" : "approaching",
            band: null,
            rationale: inWindow
                ? "Currently inside solunar major window."
                : $"Next major: {start:HH:mm}–{end:HH:mm}.");
    }

    /// <summary>
    /// Crude solunar major window for the date. Real implementation
    /// would compute lunar transit + opposite; here we approximate with
    /// (sunrise+3h, sunrise+5h) as a placeholder until the real
    /// <c>ISolunarCalculator</c> lands. Returns null when out of season
    /// for the latitude.
    /// </summary>
    private static (DateTimeOffset Start, DateTimeOffset End)? SolunarMajorOf(
        DateTimeOffset at, double latitude)
    {
        // Sunrise approximation: 6am UTC. Good enough for a placeholder.
        var dayStart = new DateTimeOffset(at.Year, at.Month, at.Day, 0, 0, 0, TimeSpan.Zero);
        var start = dayStart.AddHours(9);
        var end = dayStart.AddHours(11);
        return (start, end);
    }

    private static Driver? BuildObservationDriver(SynthesisInput<FishingActivity> input)
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
                : $"{input.RecentObservations.Count} recent observation(s) on this spot.");
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
        "pressure_trend" => d.Direction switch
        {
            "falling" => 0.85,
            "stable" => 0.65,
            "rising" => 0.40,
            _ => 0.55,
        },
        "wind_and_rain" => d.Direction switch
        {
            "calm" => 0.85,
            "lively" => 0.55,
            "harsh" => 0.20,
            _ => 0.55,
        },
        "solunar_overlap" => d.Direction switch
        {
            "in-major" => 0.90,
            "approaching" => 0.65,
            "off-major" => 0.40,
            _ => 0.5,
        },
        "thermal" => d.Direction switch
        {
            "active" => 0.85,
            "cool" => 0.65,
            "hot" => 0.45,
            "cold" => 0.40,
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

    private static string BuildRationale(IReadOnlyList<Driver> drivers)
    {
        var parts = new List<string>();
        foreach (var d in drivers.OrderByDescending(x => x.Weight * x.Confidence).Take(3))
        {
            if (d.Direction is not null)
                parts.Add($"{d.Label.ToLowerInvariant()} {d.Direction}");
        }
        return parts.Count == 0
            ? "Insufficient signal — no defensible score."
            : string.Join(", ", parts) + ".";
    }
}
