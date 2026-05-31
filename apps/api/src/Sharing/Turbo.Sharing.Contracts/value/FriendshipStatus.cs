namespace Turboapi.Sharing.value;

public enum FriendshipStatus
{
    Pending = 0,
    Accepted = 1,
    Blocked = 2,
}

public static class FriendshipStatusExtensions
{
    public static string ToWire(this FriendshipStatus status) => status switch
    {
        FriendshipStatus.Pending => "pending",
        FriendshipStatus.Accepted => "accepted",
        FriendshipStatus.Blocked => "blocked",
        _ => throw new ArgumentOutOfRangeException(nameof(status), status, null),
    };

    public static FriendshipStatus ParseFriendshipStatus(string raw) => raw switch
    {
        "pending" => FriendshipStatus.Pending,
        "accepted" => FriendshipStatus.Accepted,
        "blocked" => FriendshipStatus.Blocked,
        _ => throw new ArgumentException($"Unknown friendship status '{raw}'", nameof(raw)),
    };
}
