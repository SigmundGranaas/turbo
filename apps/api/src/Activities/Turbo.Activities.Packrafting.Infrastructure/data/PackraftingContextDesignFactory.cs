using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Activities.Packrafting.data;

public sealed class PackraftingContextDesignFactory : IDesignTimeDbContextFactory<PackraftingContext>
{
    public PackraftingContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<PackraftingContext>()
            .UseNpgsql(
                "Host=localhost;Port=5444;Database=packrafting;Username=postgres;Password=postgres",
                npgsql => npgsql.UseNetTopologySuite())
            .Options;
        return new PackraftingContext(options);
    }
}
