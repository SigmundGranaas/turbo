namespace Turboapi.Places.Core;

/// <summary>
/// Server-side configuration for the Nasjonal Turbase (ut.no / DNT) proxy. The
/// <see cref="ApiKey"/> is a secret held only by the backend so mobile clients
/// never embed it. Bound from the <c>Turbasen</c> configuration section
/// (appsettings / environment, e.g. <c>Turbasen__ApiKey</c>).
/// </summary>
public sealed class TurbasenConfig
{
    public string ApiKey { get; set; } = "";
    public string BaseUrl { get; set; } = "https://api.nasjonalturbase.no";
    public string ApiVersion { get; set; } = "v3";
}
