using System.Text.Json.Serialization;

namespace Turboapi.Activities.Packrafting.value;

public sealed record PackraftingDetails
{
    [JsonPropertyName("distanceMeters")] public int DistanceMeters { get; init; }
    [JsonPropertyName("paddleDistanceMeters")] public int PaddleDistanceMeters { get; init; }
    [JsonPropertyName("portageDistanceMeters")] public int PortageDistanceMeters { get; init; }

    [JsonPropertyName("maxGrade")] public WaterGrade MaxGrade { get; init; }
    [JsonPropertyName("typicalGrade")] public WaterGrade TypicalGrade { get; init; }

    [JsonPropertyName("putInLat")] public double PutInLat { get; init; }
    [JsonPropertyName("putInLon")] public double PutInLon { get; init; }
    [JsonPropertyName("takeOutLat")] public double TakeOutLat { get; init; }
    [JsonPropertyName("takeOutLon")] public double TakeOutLon { get; init; }

    [JsonPropertyName("nveStationCode")] public string? NveStationCode { get; init; }
    [JsonPropertyName("minFlowCumecs")] public float? MinFlowCumecs { get; init; }
    [JsonPropertyName("maxFlowCumecs")] public float? MaxFlowCumecs { get; init; }

    [JsonPropertyName("segments")] public IReadOnlyList<RouteSegment> Segments { get; init; } = Array.Empty<RouteSegment>();

    [JsonConstructor]
    public PackraftingDetails(
        int distanceMeters, int paddleDistanceMeters, int portageDistanceMeters,
        WaterGrade maxGrade, WaterGrade typicalGrade,
        double putInLat, double putInLon, double takeOutLat, double takeOutLon,
        string? nveStationCode, float? minFlowCumecs, float? maxFlowCumecs,
        IReadOnlyList<RouteSegment>? segments)
    {
        DistanceMeters = distanceMeters;
        PaddleDistanceMeters = paddleDistanceMeters;
        PortageDistanceMeters = portageDistanceMeters;
        MaxGrade = maxGrade;
        TypicalGrade = typicalGrade;
        PutInLat = putInLat; PutInLon = putInLon;
        TakeOutLat = takeOutLat; TakeOutLon = takeOutLon;
        NveStationCode = nveStationCode;
        MinFlowCumecs = minFlowCumecs;
        MaxFlowCumecs = maxFlowCumecs;
        Segments = segments ?? Array.Empty<RouteSegment>();
    }
}

public sealed record RouteSegment
{
    [JsonPropertyName("kind")] public SegmentKind Kind { get; init; }
    [JsonPropertyName("grade")] public WaterGrade? Grade { get; init; }
    [JsonPropertyName("distanceMeters")] public int DistanceMeters { get; init; }
    [JsonPropertyName("polylineWkt")] public string PolylineWkt { get; init; }
    [JsonPropertyName("notes")] public string? Notes { get; init; }

    [JsonConstructor]
    public RouteSegment(SegmentKind kind, WaterGrade? grade, int distanceMeters, string polylineWkt, string? notes)
    {
        Kind = kind;
        Grade = grade;
        DistanceMeters = distanceMeters;
        PolylineWkt = polylineWkt ?? throw new ArgumentNullException(nameof(polylineWkt));
        Notes = notes;
    }
}
