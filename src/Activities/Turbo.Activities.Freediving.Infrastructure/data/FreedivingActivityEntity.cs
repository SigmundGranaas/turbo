using NetTopologySuite.Geometries;

namespace Turboapi.Activities.Freediving.data.model;

public class FreedivingActivityEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }
    public required string Name { get; set; }
    public string? Description { get; set; }
    public required Point Geometry { get; set; }

    public short WaterBody { get; set; }
    public short BottomType { get; set; }
    public float MaxDepthMeters { get; set; }
    public float? TypicalVisibilityMeters { get; set; }
    public bool HarpoonAllowed { get; set; }
    public bool ShoreEntry { get; set; }
    public string? AccessNotes { get; set; }

    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
    public long Version { get; set; }

    public List<TargetSpeciesEntity> TargetSpecies { get; set; } = new();
}

public class TargetSpeciesEntity
{
    public Guid ActivityId { get; set; }
    public required string SpeciesCode { get; set; }
    public string? Notes { get; set; }
}
