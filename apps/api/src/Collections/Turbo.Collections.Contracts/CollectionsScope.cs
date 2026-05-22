using Turbo.Messaging;

namespace Turboapi.Collections;

/// <summary>
/// Module marker for the Collections commit boundary. See
/// <see cref="Turboapi.Geo.GeoScope"/> for the rationale.
/// </summary>
public sealed class CollectionsScope : IModuleScope
{
    public const string Name = "collections";
    public static string SourceName => Name;
}
