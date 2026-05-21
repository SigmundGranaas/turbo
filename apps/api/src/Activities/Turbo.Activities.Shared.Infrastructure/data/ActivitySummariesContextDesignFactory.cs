using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Activities.data;

/// <summary>
/// Design-time only. Lets <c>dotnet ef</c> reach the context without
/// bootstrapping the host. Production wiring is owned by
/// <c>ActivitiesSharedModule.AddActivitiesSharedModule</c>.
/// </summary>
public sealed class ActivitySummariesContextDesignFactory : IDesignTimeDbContextFactory<ActivitySummariesContext>
{
    public ActivitySummariesContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<ActivitySummariesContext>()
            .UseNpgsql(
                "Host=localhost;Port=5439;Database=activities;Username=postgres;Password=postgres",
                npgsql => npgsql.UseNetTopologySuite())
            .Options;
        return new ActivitySummariesContext(options);
    }
}
