using Turbo.Hosting.Postgres;
using Turbo.Messaging.Nats;
using Turboapi.Auth;
using Turboapi.Auth.Infrastructure.Persistence;
using Turboapi.Auth.Presentation.Middleware;

var builder = WebApplication.CreateBuilder(args);
var webAppPolicy = "WebAppPolicy";

builder.Configuration.AddEnvironmentVariables();
builder.Services.AddEndpointsApiExplorer();

builder.Services.AddCors(options =>
{
    options.AddPolicy(name: webAppPolicy, policy =>
    {
        policy.WithOrigins(
                "http://localhost:3000",
                "http://localhost:8080",
                "http://localhost:5173",
                "https://kart.sandring.no"
            )
            .AllowAnyHeader()
            .AllowAnyMethod()
            .AllowCredentials();
    });
});

builder.Services.AddAuthModule(builder.Configuration);

// AuthModule registers an OutboxDispatcherHostedService that needs an
// IMessageTransport. The modulith satisfies it with in-process
// transport; the microservices-topology host uses NATS JetStream.
// Auth publishes only — no subscribers (other modules don't react to
// auth events).
builder.Services.AddNatsMessaging(o =>
{
    o.Url = builder.Configuration["Nats:Url"] ?? "nats://localhost:4222";
    o.StreamName = "TURBO_AUTH";
    o.Subjects = ["turbo.auth.>"];
    o.SubjectPrefix = "turbo.auth";
});

var app = builder.Build();
await app.Services.MigrateModuleDatabaseAsync<AuthDbContext>(
    builder.Configuration.GetConnectionString("Auth")
        ?? throw new InvalidOperationException("ConnectionStrings:Auth is not configured"));
app.UseMiddleware<GlobalExceptionMiddleware>();
app.UseRouting();
app.UseCors(webAppPolicy);
app.UseAuthentication();
app.UseAuthorization();
app.MapControllers();
app.MapGet("/healthz", () => Results.Ok("ok")).AllowAnonymous();
app.Run();

namespace Turbo.Host.Auth
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class AuthHostProgram;
}
