using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Activities.XcSki.data;

public sealed class XcSkiContextDesignFactory : IDesignTimeDbContextFactory<XcSkiContext>
{
    public XcSkiContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<XcSkiContext>()
            .UseNpgsql(
                "Host=localhost;Port=5443;Database=xc_ski;Username=postgres;Password=postgres",
                npgsql => npgsql.UseNetTopologySuite())
            .Options;
        return new XcSkiContext(options);
    }
}
