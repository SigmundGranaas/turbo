using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Activities.Fishing.data;

/// <summary>Design-time only. Mirror of Tracks' design factory.</summary>
public sealed class FishingContextDesignFactory : IDesignTimeDbContextFactory<FishingContext>
{
    public FishingContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<FishingContext>()
            .UseNpgsql(
                "Host=localhost;Port=5440;Database=fishing;Username=postgres;Password=postgres",
                npgsql => npgsql.UseNetTopologySuite())
            .Options;
        return new FishingContext(options);
    }
}
