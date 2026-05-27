namespace Turboapi.Sharing.value;

/// <summary>
/// Subject of a grant: who or what is the access being granted to. New
/// sharing modes are added here, with a corresponding resolution rule in
/// IAccessControl. The grant table itself is agnostic.
/// </summary>
public enum SubjectType
{
    User = 0,
    Group = 1,
    Link = 2,
}

public static class SubjectTypeExtensions
{
    public static string ToWire(this SubjectType subject) => subject switch
    {
        SubjectType.User => "user",
        SubjectType.Group => "group",
        SubjectType.Link => "link",
        _ => throw new ArgumentOutOfRangeException(nameof(subject), subject, null),
    };

    public static SubjectType ParseSubjectType(string raw) => raw switch
    {
        "user" => SubjectType.User,
        "group" => SubjectType.Group,
        "link" => SubjectType.Link,
        _ => throw new ArgumentException($"Unknown subject type '{raw}'", nameof(raw)),
    };
}
