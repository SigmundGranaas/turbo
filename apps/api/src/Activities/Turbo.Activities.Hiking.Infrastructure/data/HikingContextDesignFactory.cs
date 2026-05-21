using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Activities.Hiking.data;

public sealed class HikingContextDesignFactory : IDesignTimeDbContextFactory<HikingContext>
{
    public HikingContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<HikingContext>()
            .UseNpgsql(
                "Host=localhost;Port=5442;Database=hiking;Username=postgres;Password=postgres",
                npgsql => npgsql.UseNetTopologySuite())
            .Options;
        return new HikingContext(options);
    }
}
