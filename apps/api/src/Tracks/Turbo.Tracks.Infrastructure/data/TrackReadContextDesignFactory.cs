using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Tracks.data;

/// <summary>
/// Lets `dotnet ef` build the context without bootstrapping the host.
/// Design-time only; production wiring is owned by AddTracksModule.
/// </summary>
public sealed class TrackReadContextDesignFactory : IDesignTimeDbContextFactory<TrackReadContext>
{
    public TrackReadContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<TrackReadContext>()
            .UseNpgsql(
                "Host=localhost;Port=5437;Database=tracks;Username=postgres;Password=postgres",
                npgsql => npgsql.UseNetTopologySuite())
            .Options;
        return new TrackReadContext(options);
    }
}
