using Turbo.Messaging;

namespace Turboapi.Sharing;

/// <summary>
/// Module marker for the Sharing commit boundary. Pins the outbox /
/// unit-of-work / idempotency-store generic parameters to this service so
/// other modules cannot bind to its scope by accident.
/// </summary>
public sealed class SharingScope : IModuleScope
{
    public const string Name = "sharing";
    public static string SourceName => Name;
}
