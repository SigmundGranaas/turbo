using Turbo.Messaging;

namespace Turboapi.Activities.Freediving;

public sealed class FreedivingScope : IModuleScope
{
    public const string Name = "activities.freediving";
    public static string SourceName => Name;
}
