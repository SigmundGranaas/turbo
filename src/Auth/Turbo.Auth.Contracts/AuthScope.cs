using Turbo.Messaging;

namespace Turboapi.Auth;

/// <summary>
/// Module marker for the Auth commit boundary.
/// </summary>
public sealed class AuthScope : IModuleScope
{
    public const string Name = "auth";
    public static string SourceName => Name;
}
