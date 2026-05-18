using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Activity.data;

/// <summary>
/// Lets `dotnet ef` build the context without bootstrapping the host.
/// Design-time only; production wiring is owned by AddActivityModule.
/// </summary>
public sealed class ActivityContextDesignFactory : IDesignTimeDbContextFactory<ActivityContext>
{
    public ActivityContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<ActivityContext>()
            .UseNpgsql("Host=localhost;Port=5436;Database=activity;Username=postgres;Password=postgres")
            .Options;
        return new ActivityContext(options);
    }
}
