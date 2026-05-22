namespace Turboapi.Activities.Fishing.value;

/// <summary>
/// What kind of water the spot is in. Influences which conditions provider
/// the advisor consults (river → river flow + weather; sea → tides +
/// weather; lake → weather only).
/// </summary>
public enum WaterKind
{
    River = 0,
    Lake = 1,
    Sea = 2,
}

/// <summary>
/// How the spot is fished. Affects rendering hints and the conditions UI.
/// </summary>
public enum ShoreOrBoat
{
    Shore = 0,
    Boat = 1,
    Either = 2,
}
