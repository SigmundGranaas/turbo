using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Sharing.data;

public sealed class SharingReadContextDesignFactory : IDesignTimeDbContextFactory<SharingReadContext>
{
    public SharingReadContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<SharingReadContext>()
            .UseNpgsql("Host=localhost;Port=5438;Database=sharing;Username=postgres;Password=postgres")
            .Options;
        return new SharingReadContext(options);
    }
}
