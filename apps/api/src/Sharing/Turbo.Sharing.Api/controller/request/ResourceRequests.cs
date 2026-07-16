namespace Turboapi.Sharing.controller.request;

/// <summary>Wire visibility value: private / friends / unlisted_link / public.</summary>
public sealed record SetVisibilityRequest(string Visibility);
