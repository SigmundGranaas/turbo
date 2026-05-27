namespace Turboapi.Sharing.value;

/// <summary>
/// Access roles a grant can confer on a subject. Owner is not a grant role
/// — it is implied by `resources.owner_id`.
/// </summary>
public enum Role
{
    Viewer = 0,
    Editor = 1,
}

public static class RoleExtensions
{
    public static string ToWire(this Role role) => role switch
    {
        Role.Viewer => "viewer",
        Role.Editor => "editor",
        _ => throw new ArgumentOutOfRangeException(nameof(role), role, null),
    };

    public static Role ParseRole(string raw) => raw switch
    {
        "viewer" => Role.Viewer,
        "editor" => Role.Editor,
        _ => throw new ArgumentException($"Unknown role '{raw}'", nameof(raw)),
    };

    public static bool AllowsWrite(this Role role) => role == Role.Editor;
}
