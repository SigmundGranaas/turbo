using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Collections.data;

public sealed class CollectionsReadContextDesignFactory : IDesignTimeDbContextFactory<CollectionsReadContext>
{
    public CollectionsReadContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<CollectionsReadContext>()
            .UseNpgsql("Host=localhost;Port=5438;Database=collections;Username=postgres;Password=postgres")
            .Options;
        return new CollectionsReadContext(options);
    }
}
