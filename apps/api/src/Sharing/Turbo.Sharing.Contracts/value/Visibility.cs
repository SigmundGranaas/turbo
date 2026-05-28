namespace Turboapi.Sharing.value;

/// <summary>
/// Coarse-grained visibility of a resource. Sits alongside grants — a
/// resource with visibility=public is readable by anyone regardless of
/// grants; private/friends/unlisted_link rely on the grant graph.
/// </summary>
public enum Visibility
{
    Private = 0,
    Friends = 1,
    UnlistedLink = 2,
    Public = 3,
}

public static class VisibilityExtensions
{
    public static string ToWire(this Visibility visibility) => visibility switch
    {
        Visibility.Private => "private",
        Visibility.Friends => "friends",
        Visibility.UnlistedLink => "unlisted_link",
        Visibility.Public => "public",
        _ => throw new ArgumentOutOfRangeException(nameof(visibility), visibility, null),
    };

    public static Visibility ParseVisibility(string raw) => raw switch
    {
        "private" => Visibility.Private,
        "friends" => Visibility.Friends,
        "unlisted_link" => Visibility.UnlistedLink,
        "public" => Visibility.Public,
        _ => throw new ArgumentException($"Unknown visibility '{raw}'", nameof(raw)),
    };
}
