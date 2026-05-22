namespace Turboapi.Activities.BackcountrySki.value;

/// <summary>
/// ATES (Avalanche Terrain Exposure Scale) rating. Used to filter routes
/// against the current avalanche danger level on a kind-specific scale.
/// </summary>
public enum AtesRating
{
    Unrated = 0,
    Simple = 1,
    Challenging = 2,
    Complex = 3,
}

/// <summary>
/// 8-point cardinal aspect. The route stores its dominant aspect on the
/// header row and the full distribution in the per-leg aspects table.
/// </summary>
public enum Aspect
{
    N = 0, NE = 1, E = 2, SE = 3,
    S = 4, SW = 5, W = 6, NW = 7,
}

/// <summary>Direction of motion for a single leg of the route.</summary>
public enum LegKind
{
    Ascent = 0,
    Descent = 1,
    Traverse = 2,
}
