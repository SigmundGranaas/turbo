using Turboapi.Activities.Fishing.controller.request;
using Turboapi.Activities.Fishing.data.model;
using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.controller;

public sealed record CreateFishingActivityResponse(Guid Id);

public sealed class FishingActivityResponse
{
    public Guid Id { get; set; }
    public Guid OwnerId { get; set; }
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public double Longitude { get; set; }
    public double Latitude { get; set; }
    public FishingDetailsDto Details { get; set; } = new();
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public long Version { get; set; }

    public static FishingActivityResponse From(FishingActivityEntity e) => new()
    {
        Id = e.Id,
        OwnerId = e.OwnerId,
        Name = e.Name,
        Description = e.Description,
        Longitude = e.Geometry.X,
        Latitude = e.Geometry.Y,
        Details = new FishingDetailsDto
        {
            WaterKind = (WaterKind)e.WaterKind,
            ShoreOrBoat = (ShoreOrBoat)e.ShoreOrBoat,
            AccessNotes = e.AccessNotes,
            TargetSpecies = e.TargetSpecies
                .Select(t => new TargetSpeciesDto { SpeciesCode = t.SpeciesCode, Notes = t.Notes })
                .ToList(),
            KnownDepths = e.DepthSamples
                .OrderBy(d => d.Ordinal)
                .Select(d => new DepthSampleDto { Lat = d.Lat, Lon = d.Lon, DepthMeters = d.DepthMeters })
                .ToList(),
            Preferred = (e.PreferredPressureMinHpa is null && e.PreferredPressureMaxHpa is null && e.PreferredWindMaxMs is null)
                ? null
                : new PreferredConditionsDto
                {
                    PressureMinHpa = e.PreferredPressureMinHpa,
                    PressureMaxHpa = e.PreferredPressureMaxHpa,
                    WindMaxMs = e.PreferredWindMaxMs,
                },
        },
        CreatedAt = e.CreatedAt,
        UpdatedAt = e.UpdatedAt,
        Version = e.Version,
    };
}
