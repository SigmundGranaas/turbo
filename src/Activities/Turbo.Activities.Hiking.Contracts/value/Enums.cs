namespace Turboapi.Activities.Hiking.value;

/// <summary>Trail difficulty. Loosely mirrors DNT (Norwegian Trekking
/// Association) grades but kept generic so other regions fit.</summary>
public enum HikingDifficulty
{
    Easy = 0,
    Moderate = 1,
    Hard = 2,
    Expert = 3,
}

/// <summary>How well the trail is signposted / marked.</summary>
public enum TrailMarking
{
    Unmarked = 0,
    Cairns = 1,
    Paint = 2,
    Signposted = 3,
}

/// <summary>Dominant surface underfoot.</summary>
public enum TrailSurface
{
    Mixed = 0,
    Path = 1,
    Gravel = 2,
    Boardwalk = 3,
    Rock = 4,
    Scree = 5,
    Snow = 6,
}
