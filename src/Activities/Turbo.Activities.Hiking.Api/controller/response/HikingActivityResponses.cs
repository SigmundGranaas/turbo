using NetTopologySuite.IO;
using Turboapi.Activities.Hiking.controller.request;
using Turboapi.Activities.Hiking.data.model;
using Turboapi.Activities.Hiking.value;

namespace Turboapi.Activities.Hiking.controller;

public sealed record CreateHikingActivityResponse(Guid Id);

public sealed class HikingActivityResponse
{
    public Guid Id { get; set; }
    public Guid OwnerId { get; set; }
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public string RouteWkt { get; set; } = string.Empty;
    public HikingDetailsDto Details { get; set; } = new();
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public long Version { get; set; }

    public static HikingActivityResponse From(HikingActivityEntity e) => new()
    {
        Id = e.Id, OwnerId = e.OwnerId, Name = e.Name, Description = e.Description,
        RouteWkt = new WKTWriter().Write(e.Route),
        Details = new HikingDetailsDto
        {
            DistanceMeters = e.DistanceMeters,
            AscentMeters = e.AscentMeters,
            DescentMeters = e.DescentMeters,
            ElevationMinMeters = e.ElevationMinMeters,
            ElevationMaxMeters = e.ElevationMaxMeters,
            Difficulty = (HikingDifficulty)e.Difficulty,
            Surface = (TrailSurface)e.Surface,
            Marking = (TrailMarking)e.Marking,
            EstimatedHours = e.EstimatedHours,
            HasWaterSources = e.HasWaterSources,
            HasShelter = e.HasShelter,
            WaterSources = e.WaterSources.OrderBy(w => w.Ordinal)
                .Select(w => new WaterSourceDto { Lat = w.Lat, Lon = w.Lon, Kind = w.Kind, Notes = w.Notes })
                .ToList(),
        },
        CreatedAt = e.CreatedAt, UpdatedAt = e.UpdatedAt, Version = e.Version,
    };
}

public sealed record ErrorResponse(string Title, string Detail);
