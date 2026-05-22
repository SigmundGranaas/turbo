using Turbo.Messaging;

namespace Turboapi.Activities.Packrafting;

public sealed class PackraftingScope : IModuleScope
{
    public const string Name = "activities.packrafting";
    public static string SourceName => Name;
}
