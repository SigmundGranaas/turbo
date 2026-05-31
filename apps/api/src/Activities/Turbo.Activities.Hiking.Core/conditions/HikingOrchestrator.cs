using System.Text.Json;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Hiking.domain;
using Turboapi.Activities.Hiking.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Hiking.conditions;

/// <summary>
/// Hiking orchestrator. Drivers and warnings are tuned to "can I go
/// today, and what should I plan for" — exposure, rain, daylight
/// feasibility (distance × pace × hours-of-daylight), thermal load.
/// Cabin availability + DNT trail catalog plug in when the providers
/// land.
///
/// Drivers:
/// <list type="bullet">
///   <item><c>thermal</c> — heat/cold exposure for the date</item>
///   <item><c>rain</c> — precipitation expectation</item>
///   <item><c>wind_above_treeline</c> — composes weather wind with the
///         route's treeline fraction from <see cref="ActivityGeoContext"/></item>
///   <item><c>daylight_feasibility</c> — estimatedHours vs hours of
///         daylight at the route's latitude/date</item>
///   <item><c>nearby_obs</c> — recent hiking observations</item>
/// </list>
/// </summary>
public sealed class HikingOrchestrator
    : ActivityOrchestratorPipeline<HikingActivity, ActivityAnalysis>
{
    public const string ProviderKeyWeather = "weather";

    private readonly IWeatherProvider _weather;
    private readonly TimeProvider _clock;

    public HikingOrchestrator(
        IActivityGeoContextService geoContext,
        IActivityObservationStore observations,
        IActivityVisitStore visits,
        IConditionsSnapshotStore snapshots,
        IActivitySummaryScoreWriter scoreWriter,
        ILogger<HikingOrchestrator> logger,
        IWeatherProvider weather,
        TimeProvider? clock = null)
        : base(geoContext, observations, visits, snapshots, logger, scoreWriter)
    {
        _weather = weather;
        _clock = clock ?? TimeProvider.System;
    }

    protected override string KindKey => "hiking";

    protected override NetTopologySuite.Geometries.Geometry ExtractGeometry(HikingActivity activity)
        => activity.Route;

    protected override IReadOnlyList<ProviderTask> PlanFanOut(
        HikingActivity activity, ActivityGeoContext? geoContext, QueryContext queryContext)
    {
        var line = activity.Route;
        var mid = line.Coordinates[line.NumPoints / 2];
        return new[]
        {
            new ProviderTask(
                ProviderKeyWeather,
                async ct => await _weather.GetAsync(mid.Y, mid.X, queryContext.At, ct).ConfigureAwait(false),
                extractObservedAt: s => s is WeatherSlice w ? w.ValidAt : null),
        };
    }

    protected override ActivityAnalysis Synthesize(SynthesisInput<HikingActivity> input)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        var weather = input.Get<WeatherSlice>(ProviderKeyWeather);
        var details = input.Activity.Details;
        var drivers = new List<Driver>();
        var warnings = new List<Warning>();

        // ---- Thermal --------------------------------------------------
        if (weather is not null)
        {
            var t = weather.AirTemperatureCelsius;
            string dir;
            string thermalRationale;
            if (t > 28) { dir = "hot"; thermalRationale = $"Air {t:F0}°C — heat exposure; bring extra water."; }
            else if (t > 18) { dir = "warm"; thermalRationale = $"Air {t:F0}°C — comfortable walking weather."; }
            else if (t > 5) { dir = "mild"; thermalRationale = $"Air {t:F0}°C — layer for ridges."; }
            else if (t > -5) { dir = "cold"; thermalRationale = $"Air {t:F0}°C — winter layers, insulated boots."; }
            else { dir = "very cold"; thermalRationale = $"Air {t:F0}°C — windchill, frostbite risk above treeline."; }
            drivers.Add(new Driver(
                key: "thermal",
                label: "Thermal",
                value: t,
                unit: "°C",
                weight: 0.20,
                confidence: 0.9,
                direction: dir,
                band: null,
                rationale: thermalRationale));

            if (t > 28)
                warnings.Add(new Warning(
                    code: "HEAT_EXPOSURE",
                    severity: Severity.Caution,
                    title: "Heat exposure",
                    body: "Above 28°C — start early, plan shaded breaks, double water.",
                    sourceUrl: null));
        }

        // ---- Rain -----------------------------------------------------
        if (weather is not null)
        {
            var p1h = weather.PrecipitationNext1hMm ?? 0;
            var p6h = weather.PrecipitationNext6hMm ?? 0;
            string dir;
            string rainRationale;
            if (p6h > 15) { dir = "heavy"; rainRationale = $"{p6h:F0} mm/6h — wet hike; check creeks for fordability."; }
            else if (p6h > 3) { dir = "light"; rainRationale = $"{p6h:F0} mm/6h — pack shell."; }
            else if (p1h > 0.2) { dir = "showers"; rainRationale = "Scattered showers."; }
            else { dir = "dry"; rainRationale = "Dry forecast."; }
            drivers.Add(new Driver(
                key: "rain",
                label: "Precipitation",
                value: p6h,
                unit: "mm/6h",
                weight: 0.15,
                confidence: 0.85,
                direction: dir,
                band: null,
                rationale: rainRationale));
        }

        // ---- Wind above treeline --------------------------------------
        if (weather is not null && input.GeoContext is not null)
        {
            var aboveFrac = input.GeoContext.AboveTreelineFractionM ?? 0.0;
            var w = weather.WindSpeedMs;
            // Score combines wind with how much of the route is exposed.
            string dir;
            string windRationale;
            if (w > 14 && aboveFrac > 0.2)
            {
                dir = "harsh exposed";
                windRationale = $"{w:F0} m/s wind with {(aboveFrac * 100):F0}% of the route above treeline — punishing on exposed sections.";
                warnings.Add(new Warning(
                    code: "WIND_ABOVE_TREELINE",
                    severity: Severity.Caution,
                    title: "Wind on exposed sections",
                    body: "Above-treeline sections will see the full forecast wind. Consider an alternative or reverse direction.",
                    sourceUrl: null));
            }
            else if (w > 14)
            {
                dir = "strong but sheltered";
                windRationale = $"{w:F0} m/s wind — most of the route is below treeline.";
            }
            else
            {
                dir = "manageable";
                windRationale = $"{w:F0} m/s wind.";
            }
            drivers.Add(new Driver(
                key: "wind_above_treeline",
                label: "Wind exposure",
                value: w,
                unit: "m/s",
                weight: 0.15,
                confidence: 0.6,
                direction: dir,
                band: null,
                rationale: windRationale));
        }
        else if (weather is not null)
        {
            // No geo context — fall back to plain wind.
            drivers.Add(new Driver(
                key: "wind_above_treeline",
                label: "Wind",
                value: weather.WindSpeedMs,
                unit: "m/s",
                weight: 0.10,
                confidence: 0.4,
                direction: weather.WindSpeedMs > 14 ? "strong" : "manageable",
                band: null,
                rationale: $"Wind {weather.WindSpeedMs:F0} m/s; route exposure unknown."));
        }

        // ---- Daylight feasibility -------------------------------------
        var feasibility = BuildDaylightFeasibilityDriver(details, input.Activity.Route, input.QueryContext.At);
        drivers.Add(feasibility);

        // ---- Observations ---------------------------------------------
        var obsDriver = BuildObservationDriver(input);
        if (obsDriver is not null) drivers.Add(obsDriver);

        var (score, confidence) = CompositeScoreAndConfidence(drivers);
        var rationale = BuildRationale(drivers);
        var windows = Array.Empty<TimeWindow>();

        // KindSlice: section-by-section profile is too heavy for v1;
        // surface a compact summary of the elevation profile + treeline
        // exposure pulled from geo context.
        var kindSlices = new Dictionary<string, JsonElement>();
        kindSlices["hiking"] = JsonSerializer.SerializeToElement(new
        {
            ascentMDerived = input.GeoContext?.AscentM,
            descentMDerived = input.GeoContext?.DescentM,
            lengthMDerived = input.GeoContext?.LengthM,
            aboveTreelineFractionM = input.GeoContext?.AboveTreelineFractionM,
            estimatedHoursStored = details.EstimatedHours,
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
            suggestedWindows: windows,
            kindSlices: kindSlices,
            provenance: input.ToProvenance(sw.ElapsedMilliseconds));
    }

    private static Driver BuildDaylightFeasibilityDriver(
        HikingDetails details, NetTopologySuite.Geometries.LineString route, DateTimeOffset at)
    {
        // Rough hours of civil twilight at the route's midpoint
        // latitude for the calendar day. Without a proper astronomy
        // library we approximate with a sinusoidal model — accurate to
        // within ~30 min for 55-70°N, good enough as a tripwire driver.
        var midLat = route.Coordinates[route.NumPoints / 2].Y;
        var doy = at.UtcDateTime.DayOfYear;
        var hoursOfDaylight = ApproxDaylightHours(midLat, doy);
        var estimated = (double)(details.EstimatedHours ?? 0);
        if (estimated <= 0)
        {
            // Fall back to a coarse pace estimate: 4 km/h on the flat +
            // 10 min per 100 m of ascent. Works for trail-grade hikes.
            var km = details.DistanceMeters / 1000.0;
            var ascentBonusH = details.AscentMeters / 100.0 * (10.0 / 60);
            estimated = km / 4.0 + ascentBonusH;
        }

        string dir;
        string rationale;
        var slack = hoursOfDaylight - estimated;
        if (slack < 0)
        {
            dir = "infeasible-today";
            rationale = $"Estimated {estimated:F1}h vs {hoursOfDaylight:F1}h of daylight — finish in the dark.";
        }
        else if (slack < 1.5)
        {
            dir = "tight";
            rationale = $"Estimated {estimated:F1}h vs {hoursOfDaylight:F1}h of daylight — start early.";
        }
        else
        {
            dir = "comfortable";
            rationale = $"Estimated {estimated:F1}h vs {hoursOfDaylight:F1}h of daylight — plenty of slack.";
        }

        return new Driver(
            key: "daylight_feasibility",
            label: "Daylight",
            value: hoursOfDaylight,
            unit: "h",
            weight: 0.20,
            confidence: 0.65,
            direction: dir,
            band: null,
            rationale: rationale);
    }

    /// <summary>Sinusoidal approximation of civil-daylight hours for a
    /// given latitude and day-of-year. Tuned to roughly match
    /// Norwegian latitudes; not a substitute for a real ephemeris.</summary>
    private static double ApproxDaylightHours(double latitudeDeg, int dayOfYear)
    {
        var doyRad = (dayOfYear - 81) / 365.0 * 2 * Math.PI;
        var declination = 23.44 * Math.Sin(doyRad) * Math.PI / 180.0;
        var latRad = latitudeDeg * Math.PI / 180.0;
        // Sunrise hour-angle formula. Clamped at ±1 for polar day/night.
        var cosH = -Math.Tan(latRad) * Math.Tan(declination);
        if (cosH > 1) return 0;     // polar night
        if (cosH < -1) return 24;   // polar day
        var hAngle = Math.Acos(cosH);
        return hAngle * 2 * 24 / (2 * Math.PI);
    }

    private static Driver? BuildObservationDriver(SynthesisInput<HikingActivity> input)
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
                : $"{input.RecentObservations.Count} recent observation(s) on this trail.");
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
        "thermal" => d.Direction switch
        {
            "warm" => 0.95,
            "mild" => 0.85,
            "hot" => 0.45,
            "cold" => 0.60,
            "very cold" => 0.30,
            _ => 0.5,
        },
        "rain" => d.Direction switch
        {
            "dry" => 0.95,
            "showers" => 0.70,
            "light" => 0.55,
            "heavy" => 0.20,
            _ => 0.6,
        },
        "wind_above_treeline" => d.Direction switch
        {
            "manageable" => 0.90,
            "strong but sheltered" => 0.65,
            "harsh exposed" => 0.25,
            "strong" => 0.45,
            _ => 0.6,
        },
        "daylight_feasibility" => d.Direction switch
        {
            "comfortable" => 0.95,
            "tight" => 0.55,
            "infeasible-today" => 0.05,
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
