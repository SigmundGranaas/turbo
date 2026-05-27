namespace Turboapi.Sharing.value;

/// <summary>
/// Domain payload type carried by a Resource. Sharing has no compile-time
/// dependency on the payload modules; the values listed here are conventions
/// that callers register and consult. New shareable types add a string;
/// nothing else in this module changes.
/// </summary>
public static class ResourceType
{
    public const string Collection = "collection";
    public const string Marker = "marker";
    public const string Path = "path";
}
