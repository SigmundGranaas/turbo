namespace Turboapi.Sharing.controller.request;

public sealed record GrantToUserRequest(Guid ResourceId, Guid UserId, string Role, DateTime? ExpiresAt);
public sealed record GrantToGroupRequest(Guid ResourceId, Guid GroupId, string Role, DateTime? ExpiresAt);
public sealed record GrantAsLinkRequest(Guid ResourceId, string Role, DateTime? ExpiresAt);
