using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;
using Turboapi.Geo.domain.query.model;

namespace Turboapi.Geo.data;

/// <summary>
/// Lets `dotnet ef` build the context without bootstrapping the host.
/// Design-time only; production wiring is owned by AddGeoModule.
/// </summary>
public sealed class LocationReadContextDesignFactory : IDesignTimeDbContextFactory<LocationReadContext>
{
    public LocationReadContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<LocationReadContext>()
            .UseNpgsql(
                "Host=localhost;Port=5435;Database=geo;Username=postgres;Password=postgres",
                npgsql => npgsql.UseNetTopologySuite())
            .Options;
        return new LocationReadContext(options);
    }
}
