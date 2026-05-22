using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Activities.Freediving.data;

public sealed class FreedivingContextDesignFactory : IDesignTimeDbContextFactory<FreedivingContext>
{
    public FreedivingContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<FreedivingContext>()
            .UseNpgsql(
                "Host=localhost;Port=5445;Database=freediving;Username=postgres;Password=postgres",
                npgsql => npgsql.UseNetTopologySuite())
            .Options;
        return new FreedivingContext(options);
    }
}
