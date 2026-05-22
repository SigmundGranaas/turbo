using Turbo.Messaging;

namespace Turboapi.Tracks;

/// <summary>
/// Module marker for the Tracks commit boundary. See
/// <see cref="Turboapi.Geo.GeoScope"/> for the rationale.
/// </summary>
public sealed class TracksScope : IModuleScope
{
    public const string Name = "tracks";
    public static string SourceName => Name;
}
