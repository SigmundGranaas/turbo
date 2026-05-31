using System.Security.Cryptography;
using System.Text.Json;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using Turboapi.Activities.data;
using Turboapi.Activities.data.model;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;

namespace Turboapi.Activities.services;

/// <summary>
/// Real implementation of <see cref="IActivityGeoContextService"/>. Replaces
/// the placeholder stub:
///
/// <list type="bullet">
///   <item>Samples elevation along the geometry via
///         <see cref="IElevationProvider"/> (synthetic for now; Kartverket
///         Høydedata WCS later).</item>
///   <item>Derives ascent / descent / length / min-max elevation from
///         the sampled profile.</item>
///   <item>Builds an 8-bin aspect histogram + 5°-bin slope histogram by
///         walking the geometry as a sequence of segments. Each segment
///         contributes its <c>(bearing, rise/run)</c> to the relevant bin
///         weighted by its 3D length.</item>
///   <item>Hashes the geometry's WKB so recompute is skipped when the
///         geometry hasn't changed.</item>
///   <item>Persists the full result as jsonb on
///         <see cref="ActivityGeoContextEntity"/>.</item>
/// </list>
///
/// Soft-fails on DEM error: returns a minimal context that still encodes
/// length + zeroed climb. Orchestrators read the context and lower
/// confidence on aspect-derived drivers when the result is sparse.
///
/// Region polygons (Varsom, Mareano) and watershed lookup will be
/// layered in via <see cref="WatershedHrefId"/> /
/// <see cref="VarsomRegionId"/> in a follow-up — the schema already
/// carries them so callers don't need to change.
/// </summary>
public sealed class ActivityGeoContextService : IActivityGeoContextService
{
    private readonly ActivitySummariesContext _db;
    private readonly IElevationProvider _elevation;
    private readonly IRegionPolygonStore _regions;
    private readonly ILogger<ActivityGeoContextService> _logger;

    private const double SegmentSpacingM = 25.0;
    private const string VarsomSource = "varsom_region";
    private const string MareanoSource = "mareano_cell";
    private const string WatershedSource = "watershed";

    public ActivityGeoContextService(
        ActivitySummariesContext db,
        IElevationProvider elevation,
        IRegionPolygonStore regions,
        ILogger<ActivityGeoContextService> logger)
    {
        _db = db;
        _elevation = elevation;
        _regions = regions;
        _logger = logger;
    }

    public async Task<ActivityGeoContext> ComputeAndStoreAsync(
        Guid activityId, Geometry geometry, CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(geometry);
        var hash = GeometryHash(geometry);
        var existing = await _db.ActivityGeoContexts
            .FirstOrDefaultAsync(g => g.ActivityId == activityId, cancellationToken);
        if (existing is not null && existing.GeomHash == hash)
        {
            return DeserializePayload(existing);
        }

        var ctx = await ComputeInternalAsync(activityId, geometry, version: (existing?.Version ?? 0) + 1, cancellationToken);
        var payload = JsonSerializer.SerializeToDocument(ctx);
        if (existing is null)
        {
            _db.ActivityGeoContexts.Add(new ActivityGeoContextEntity
            {
                ActivityId = activityId,
                Version = ctx.Version,
                GeomHash = hash,
                Payload = payload,
                ComputedAt = ctx.ComputedAt,
            });
        }
        else
        {
            existing.Version = ctx.Version;
            existing.GeomHash = hash;
            existing.Payload = payload;
            existing.ComputedAt = ctx.ComputedAt;
        }
        await _db.SaveChangesAsync(cancellationToken);
        return ctx;
    }

    public async Task<ActivityGeoContext?> GetAsync(Guid activityId, CancellationToken cancellationToken)
    {
        var row = await _db.ActivityGeoContexts.AsNoTracking()
            .FirstOrDefaultAsync(g => g.ActivityId == activityId, cancellationToken);
        return row is null ? null : DeserializePayload(row);
    }

    public Task<ActivityGeoContext> ComputeTransientAsync(Geometry geometry, CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(geometry);
        return ComputeInternalAsync(Guid.Empty, geometry, version: 0, cancellationToken);
    }

    private async Task<ActivityGeoContext> ComputeInternalAsync(
        Guid activityId, Geometry geometry, int version, CancellationToken ct)
    {
        var lengthM = GeographicLengthM(geometry);
        var path = SampledPath(geometry);
        ElevationSlice? elevation = null;
        if (path.Count > 0)
        {
            try
            {
                elevation = await _elevation.GetAsync(path, SegmentSpacingM, ct);
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Elevation lookup failed for {ActivityId} — using minimal geo context", activityId);
            }
        }

        var profile = elevation?.Samples ?? Array.Empty<ElevationSample>();
        var (ascent, descent, elevMin, elevMax) = AscentDescent(profile);
        var aspectMix = BuildAspectMix(geometry, profile);
        var slopeHist = BuildSlopeHistogram(geometry, profile);

        // Region-polygon lookups. Each is soft-failing — when the
        // geo_regions table is empty for a source, the geo context just
        // carries a null id and the orchestrator degrades gracefully.
        var centroid = geometry.Centroid;
        int? varsomRegionId = null;
        string? varsomRegionName = null;
        string? watershedHrefId = null;
        int? mareanoCellId = null;
        try
        {
            var varsom = await _regions.FindContainingAsync(VarsomSource, centroid, ct).ConfigureAwait(false);
            if (varsom is not null && int.TryParse(varsom.RegionId, out var vid))
            {
                varsomRegionId = vid;
                varsomRegionName = varsom.Name;
            }
        }
        catch (Exception ex)
        {
            _logger.LogDebug(ex, "Varsom region lookup failed for {ActivityId}", activityId);
        }
        try
        {
            var ws = await _regions.FindContainingAsync(WatershedSource, centroid, ct).ConfigureAwait(false);
            watershedHrefId = ws?.RegionId;
        }
        catch (Exception ex)
        {
            _logger.LogDebug(ex, "Watershed lookup failed for {ActivityId}", activityId);
        }
        try
        {
            var ma = await _regions.FindContainingAsync(MareanoSource, centroid, ct).ConfigureAwait(false);
            if (ma is not null && int.TryParse(ma.RegionId, out var mid)) mareanoCellId = mid;
        }
        catch (Exception ex)
        {
            _logger.LogDebug(ex, "Mareano lookup failed for {ActivityId}", activityId);
        }

        return new ActivityGeoContext(
            activityId: activityId,
            version: version,
            elevationMinM: elevMin,
            elevationMaxM: elevMax,
            ascentM: ascent,
            descentM: descent,
            lengthM: lengthM,
            aspectMix: aspectMix,
            slopeHistogram: slopeHist,
            varsomRegionId: varsomRegionId,
            varsomRegionName: varsomRegionName,
            mareanoCellId: mareanoCellId,
            watershedHrefId: watershedHrefId,
            nveStations: Array.Empty<NearestStation>(), // populated in a follow-up
            treelineCrossings: null,
            aboveTreelineFractionM: null,
            touchesCoastline: false,
            distanceToCoastM: null,
            computedAt: DateTime.UtcNow);
    }

    /// <summary>SHA-256 of the geometry's WKB. Recompute is skipped when
    /// the hash matches the stored row.</summary>
    private static string GeometryHash(Geometry geometry)
    {
        var writer = new NetTopologySuite.IO.WKBWriter();
        var bytes = writer.Write(geometry);
        var sha = SHA256.HashData(bytes);
        return Convert.ToHexString(sha);
    }

    private static IReadOnlyList<(double Latitude, double Longitude)> SampledPath(Geometry geometry)
    {
        if (geometry is Point p)
        {
            return new[] { (p.Y, p.X) };
        }
        if (geometry is LineString ls)
        {
            var coords = ls.Coordinates;
            if (coords.Length == 0) return Array.Empty<(double, double)>();
            // Use the geometry's own vertex sequence — the elevation
            // provider re-samples internally to whatever spacing it
            // prefers. We don't densify here because over-sampling on
            // long routes blows up the upstream call cost.
            var result = new (double, double)[coords.Length];
            for (var i = 0; i < coords.Length; i++) result[i] = (coords[i].Y, coords[i].X);
            return result;
        }
        return new[] { (geometry.Centroid.Y, geometry.Centroid.X) };
    }

    private static double GeographicLengthM(Geometry geometry)
    {
        if (geometry is not LineString ls) return 0;
        const double earthRadiusM = 6_371_000;
        double total = 0;
        var coords = ls.Coordinates;
        for (var i = 1; i < coords.Length; i++)
        {
            var lat1 = coords[i - 1].Y * Math.PI / 180.0;
            var lat2 = coords[i].Y * Math.PI / 180.0;
            var dLat = lat2 - lat1;
            var dLon = (coords[i].X - coords[i - 1].X) * Math.PI / 180.0;
            var a = Math.Sin(dLat / 2) * Math.Sin(dLat / 2)
                    + Math.Cos(lat1) * Math.Cos(lat2) * Math.Sin(dLon / 2) * Math.Sin(dLon / 2);
            var c = 2 * Math.Atan2(Math.Sqrt(a), Math.Sqrt(1 - a));
            total += earthRadiusM * c;
        }
        return total;
    }

    private static (double Ascent, double Descent, double Min, double Max) AscentDescent(
        IReadOnlyList<ElevationSample> profile)
    {
        if (profile.Count == 0) return (0, 0, 0, 0);
        double ascent = 0;
        double descent = 0;
        var min = profile[0].ElevationM;
        var max = profile[0].ElevationM;
        for (var i = 1; i < profile.Count; i++)
        {
            var delta = profile[i].ElevationM - profile[i - 1].ElevationM;
            if (delta > 0) ascent += delta;
            else descent += -delta;
            if (profile[i].ElevationM < min) min = profile[i].ElevationM;
            if (profile[i].ElevationM > max) max = profile[i].ElevationM;
        }
        return (ascent, descent, min, max);
    }

    /// <summary>Build an 8-bin aspect histogram by walking each segment
    /// and binning its bearing weighted by the segment's horizontal
    /// length. Segments with negligible slope (no real "aspect") still
    /// contribute, weighted by length — for an XC trail with slight
    /// undulations this gives the trail-direction histogram, which is
    /// good enough as a proxy.</summary>
    private static IReadOnlyList<AspectShare> BuildAspectMix(
        Geometry geometry, IReadOnlyList<ElevationSample> profile)
    {
        if (geometry is not LineString ls || ls.Coordinates.Length < 2)
            return Array.Empty<AspectShare>();

        var coords = ls.Coordinates;
        var bins = new double[8];
        double totalWeight = 0;
        for (var i = 1; i < coords.Length; i++)
        {
            var bearing = Bearing(coords[i - 1], coords[i]);
            var weight = SegmentMeters(coords[i - 1], coords[i]);
            var bin = AspectBin(bearing);
            bins[bin] += weight;
            totalWeight += weight;
        }
        if (totalWeight <= 0) return Array.Empty<AspectShare>();
        var result = new AspectShare[8];
        for (var i = 0; i < 8; i++)
            result[i] = new AspectShare((Aspect)i, bins[i] / totalWeight);
        return result;
    }

    private static IReadOnlyList<SlopeBin> BuildSlopeHistogram(
        Geometry geometry, IReadOnlyList<ElevationSample> profile)
    {
        if (geometry is not LineString ls || ls.Coordinates.Length < 2 || profile.Count < 2)
            return Array.Empty<SlopeBin>();

        var coords = ls.Coordinates;
        // Build a per-segment slope by sampling elevation at the
        // segment's start + end distance. Profile distances are
        // monotonic + start at zero (see SyntheticElevationProvider /
        // future Kartverket impl).
        var bins = new double[11]; // 0-5, 5-10, …, 45-50, 50+
        double total = 0;
        double cumulative = 0;
        for (var i = 1; i < coords.Length; i++)
        {
            var segLen = SegmentMeters(coords[i - 1], coords[i]);
            if (segLen <= 0) continue;
            var startD = cumulative;
            var endD = cumulative + segLen;
            cumulative = endD;
            var startE = InterpolateElevation(profile, startD);
            var endE = InterpolateElevation(profile, endD);
            if (startE is null || endE is null) continue;
            var rise = Math.Abs(endE.Value - startE.Value);
            var slope = Math.Atan2(rise, segLen) * 180.0 / Math.PI;
            var binIdx = Math.Clamp((int)(slope / 5), 0, 10);
            bins[binIdx] += segLen;
            total += segLen;
        }
        if (total <= 0) return Array.Empty<SlopeBin>();
        var result = new SlopeBin[11];
        for (var i = 0; i < 11; i++)
        {
            var min = i * 5;
            var max = i == 10 ? 90 : (i + 1) * 5;
            result[i] = new SlopeBin(min, max, bins[i] / total);
        }
        return result;
    }

    private static double? InterpolateElevation(IReadOnlyList<ElevationSample> profile, double distanceM)
    {
        if (profile.Count == 0) return null;
        if (distanceM <= profile[0].DistanceM) return profile[0].ElevationM;
        for (var i = 1; i < profile.Count; i++)
        {
            if (distanceM <= profile[i].DistanceM)
            {
                var t = (distanceM - profile[i - 1].DistanceM) /
                        Math.Max(1e-6, profile[i].DistanceM - profile[i - 1].DistanceM);
                return profile[i - 1].ElevationM + t * (profile[i].ElevationM - profile[i - 1].ElevationM);
            }
        }
        return profile[^1].ElevationM;
    }

    private static int AspectBin(double bearingDegrees)
    {
        // N is 0°; bins are centred at 0, 45, 90, …, 315 with ±22.5°
        // half-widths.
        var shifted = (bearingDegrees + 22.5 + 360) % 360;
        return (int)(shifted / 45) % 8;
    }

    private static double Bearing(Coordinate from, Coordinate to)
    {
        var lat1 = from.Y * Math.PI / 180.0;
        var lat2 = to.Y * Math.PI / 180.0;
        var dLon = (to.X - from.X) * Math.PI / 180.0;
        var y = Math.Sin(dLon) * Math.Cos(lat2);
        var x = Math.Cos(lat1) * Math.Sin(lat2) - Math.Sin(lat1) * Math.Cos(lat2) * Math.Cos(dLon);
        var brng = Math.Atan2(y, x) * 180.0 / Math.PI;
        return (brng + 360) % 360;
    }

    private static double SegmentMeters(Coordinate from, Coordinate to)
    {
        const double earthRadiusM = 6_371_000;
        var lat1 = from.Y * Math.PI / 180.0;
        var lat2 = to.Y * Math.PI / 180.0;
        var dLat = lat2 - lat1;
        var dLon = (to.X - from.X) * Math.PI / 180.0;
        var a = Math.Sin(dLat / 2) * Math.Sin(dLat / 2)
                + Math.Cos(lat1) * Math.Cos(lat2) * Math.Sin(dLon / 2) * Math.Sin(dLon / 2);
        var c = 2 * Math.Atan2(Math.Sqrt(a), Math.Sqrt(1 - a));
        return earthRadiusM * c;
    }

    private static ActivityGeoContext DeserializePayload(ActivityGeoContextEntity row)
    {
        try
        {
            var ctx = row.Payload.RootElement.Deserialize<ActivityGeoContext>();
            if (ctx is not null) return ctx;
        }
        catch (JsonException) { /* fall through */ }

        // Last-resort minimal context if the stored payload can't be
        // parsed (schema drift on an old row, etc.).
        return new ActivityGeoContext(
            activityId: row.ActivityId, version: row.Version,
            elevationMinM: 0, elevationMaxM: 0,
            ascentM: 0, descentM: 0, lengthM: 0,
            aspectMix: Array.Empty<AspectShare>(),
            slopeHistogram: Array.Empty<SlopeBin>(),
            varsomRegionId: null, varsomRegionName: null,
            mareanoCellId: null, watershedHrefId: null,
            nveStations: Array.Empty<NearestStation>(),
            treelineCrossings: null, aboveTreelineFractionM: null,
            touchesCoastline: false, distanceToCoastM: null,
            computedAt: DateTime.SpecifyKind(row.ComputedAt, DateTimeKind.Utc));
    }
}
