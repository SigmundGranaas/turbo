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
                .WithOrigins("http://localhost:8080", "https://kartapi.sandring.no")
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

app.UseHttpsRedirection();
app.UseCors("Default");

app.Run();

namespace Turbo.Gateway
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class GatewayProgram;
}
