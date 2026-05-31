namespace Turboapi.Sharing.controller.request;

public sealed record CreateGroupRequest(string Name);
public sealed record RenameGroupRequest(string Name);
public sealed record GroupMemberRequest(Guid UserId);
