using Turbo.Messaging;

namespace Turboapi.Activity;

/// <summary>
/// Module marker for the Activity commit boundary. Handlers depend on
/// <c>IOutbox&lt;ActivityScope&gt;</c> and
/// <c>IUnitOfWork&lt;ActivityScope&gt;</c>; the composition root binds those
/// to the EF Core implementations against <c>ActivityContext</c>. The
/// static <see cref="SourceName"/> is the value every event envelope this
/// module publishes carries, so handlers never have to repeat the string.
/// </summary>
public sealed class ActivityScope : IModuleScope
{
    public const string Name = "activity";
    public static string SourceName => Name;
}
