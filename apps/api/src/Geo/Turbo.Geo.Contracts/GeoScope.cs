using Turbo.Messaging;

namespace Turboapi.Geo;

/// <summary>
/// Module marker for the Geo commit boundary. See
/// <see cref="Turboapi.Activity.ActivityScope"/> for the rationale.
/// </summary>
public sealed class GeoScope : IModuleScope
{
    public const string Name = "geo";
    public static string SourceName => Name;
}
