using Turbo.Messaging;

namespace Turboapi.Activities.Hiking;

public sealed class HikingScope : IModuleScope
{
    public const string Name = "activities.hiking";
    public static string SourceName => Name;
}
