using Turbo.Hosting.Postgres;
using Turbo.Messaging.Nats;
using Turboapi.Collections;
using Turboapi.Collections.data;
using TurboAuthentication.Extensions;

var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

builder.Services.AddEndpointsApiExplorer();
builder.Services.AddAuthorization();

builder.Services.AddTurboAuth(builder.Configuration);
builder.Services.AddCollectionsModule(builder.Configuration);

builder.Services.AddNatsMessaging(o =>
{
    o.Url = builder.Configuration["Nats:Url"] ?? "nats://localhost:4222";
    o.StreamName = "TURBO_COLLECTIONS";
    o.Subjects = ["turbo.collections.>"];
    o.SubjectPrefix = "turbo.collections";
});
builder.Services.AddCollectionsNatsSubscribers();

var app = builder.Build();
await app.Services.MigrateModuleDatabaseAsync<CollectionsReadContext>(
    builder.Configuration.GetConnectionString("Collections")
        ?? throw new InvalidOperationException("ConnectionStrings:Collections is not configured"));
app.UseHttpsRedirection();
app.UseAuthentication();
app.UseAuthorization();
app.MapControllers();
app.MapGet("/healthz", () => Results.Ok("ok")).AllowAnonymous();
app.Run();

namespace Turbo.Host.Collections
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class CollectionsHostProgram;
}
