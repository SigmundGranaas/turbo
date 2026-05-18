using Microsoft.AspNetCore.Mvc;

namespace Turboapi.Geo.controller.request;

public record CreateLocationRequest
{
    public GeometryData Geometry { get; init; } = null!;
    public DisplayData Display { get; init; } = null!;
}

public record UpdateLocationRequest
{
    public GeometryData? Geometry { get; init; }
    public DisplayChangeset? Display { get; init; }
}

public record ExtentQuery
{
    public ExtentData Extent { get; init; } = null!;
}

public record GeometryData
{
    public double Longitude { get; init; }
    public double Latitude { get; init; }
}

public record DisplayData
{
    public string Name { get; init; } = null!;
    public string? Description { get; init; }
    public string? Icon { get; init; }
}

public record DisplayChangeset
{
    public string? Name { get; init; } = null!;
    public string? Description { get; init; }
    public string? Icon { get; init; }
}
public record ExtentData
{
    [FromQuery(Name = "minLon")]
    public double MinLongitude { get; init; }
    
    [FromQuery(Name = "minLat")]
    public double MinLatitude { get; init; }
    
    [FromQuery(Name = "maxLon")]
    public double MaxLongitude { get; init; }
    
    [FromQuery(Name = "maxLat")]
    public double MaxLatitude { get; init; }
}
