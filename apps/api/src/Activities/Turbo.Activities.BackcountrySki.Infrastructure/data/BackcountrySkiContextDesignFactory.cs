using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Activities.BackcountrySki.data;

public sealed class BackcountrySkiContextDesignFactory : IDesignTimeDbContextFactory<BackcountrySkiContext>
{
    public BackcountrySkiContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<BackcountrySkiContext>()
            .UseNpgsql(
                "Host=localhost;Port=5441;Database=backcountry_ski;Username=postgres;Password=postgres",
                npgsql => npgsql.UseNetTopologySuite())
            .Options;
        return new BackcountrySkiContext(options);
    }
}
