var builder = WebApplication.CreateBuilder(args);

builder.Services.AddOpenApi();

// Topology=Microservices (default) routes per-service to auth/geo/activity;
// Topology=Modulith routes all three controller prefixes to a single host.
// Each topology lives under ReverseProxy:<Topology> in appsettings.json.
var topology = builder.Configuration["Topology"] ?? "Microservices";
builder.Services.AddReverseProxy()
    .LoadFromConfig(builder.Configuration.GetSection($"ReverseProxy:{topology}"));

builder.Services.AddCors(options =>
{
    options.AddPolicy(name: "Default",
        policy =>
        {
            policy
                .WithOrigins(
                    "http://localhost:8080",
                    "https://kartapi.sandring.no",
                    // The turbomap web app (kart.sandring.no) calls this API
                    // cross-origin with credentials; the Vite dev server (5173)
                    // does too. Both are same-site (sandring.no) so the Lax auth
                    // cookies flow; CORS just has to allow the origin to read.
                    "https://kart.sandring.no",
                    "http://localhost:5173")
                .AllowAnyMethod()
                .AllowAnyHeader()
                .AllowCredentials();
        });
});

var app = builder.Build();

app.MapGet("/healthz", () => Results.Ok("ok"));
app.MapReverseProxy();

if (app.Environment.IsDevelopment())
{
    app.MapOpenApi();
}
else
{
    // Only enforce HTTPS in non-Development. Local dev runs HTTP-only
    // behind compose; UseHttpsRedirection would log "Failed to
    // determine the https port for redirect" on every request.
    app.UseHttpsRedirection();
}

app.UseCors("Default");

app.Run();

namespace Turbo.Gateway
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class GatewayProgram;
}
