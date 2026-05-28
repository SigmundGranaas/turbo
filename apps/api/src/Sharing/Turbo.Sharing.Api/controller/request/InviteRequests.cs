namespace Turboapi.Sharing.controller.request;

public sealed record CreateFriendInviteRequest(string Email, int? LifetimeDays);
public sealed record CreateResourceInviteRequest(string Email, Guid ResourceId, string Role, int? LifetimeDays);
public sealed record RedeemInvitesRequest(string Email);
