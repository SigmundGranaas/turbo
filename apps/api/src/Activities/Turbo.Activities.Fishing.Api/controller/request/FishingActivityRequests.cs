using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.controller.request;

public sealed class CreateFishingActivityRequest
{
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public double Longitude { get; set; }
    public double Latitude { get; set; }
    public FishingDetailsDto Details { get; set; } = new();
}

public sealed class UpdateFishingActivityRequest
{
    public string? Name { get; set; }
    public string? Description { get; set; }
    public double? Longitude { get; set; }
    public double? Latitude { get; set; }
    public FishingDetailsDto? Details { get; set; }
}

public sealed class FishingDetailsDto
{
    public WaterKind WaterKind { get; set; }
    public ShoreOrBoat ShoreOrBoat { get; set; }
    public string? AccessNotes { get; set; }
    public List<TargetSpeciesDto> TargetSpecies { get; set; } = new();
    public List<DepthSampleDto> KnownDepths { get; set; } = new();
    public PreferredConditionsDto? Preferred { get; set; }

    public FishingDetails ToValueObject() => new(
        WaterKind, ShoreOrBoat, AccessNotes,
        TargetSpecies.Select(t => new TargetSpecies(t.SpeciesCode, t.Notes)).ToList(),
        KnownDepths.Select(d => new DepthSample(d.Lat, d.Lon, d.DepthMeters)).ToList(),
        Preferred?.ToValueObject());
}

public sealed class TargetSpeciesDto
{
    public string SpeciesCode { get; set; } = string.Empty;
    public string? Notes { get; set; }
}

public sealed class DepthSampleDto
{
    public double Lat { get; set; }
    public double Lon { get; set; }
    public float DepthMeters { get; set; }
}

public sealed class PreferredConditionsDto
{
    public short? PressureMinHpa { get; set; }
    public short? PressureMaxHpa { get; set; }
    public float? WindMaxMs { get; set; }

    public PreferredConditions ToValueObject() => new(PressureMinHpa, PressureMaxHpa, WindMaxMs);
}
