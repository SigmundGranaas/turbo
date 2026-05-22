namespace Turboapi.Activities.XcSki.value;

/// <summary>Which technique(s) the trail supports.</summary>
public enum XcSkiTechnique
{
    Classic = 0,
    Skate = 1,
    Both = 2,
    Backcountry = 3,   // off-piste / not groomed
}

/// <summary>How recently the trail was groomed — server-side defaults
/// to <see cref="Unknown"/>; an external feed (e.g. Skisporet) would
/// overwrite this in a follow-up conditions provider.</summary>
public enum GroomingStatus
{
    Unknown = 0,
    Today = 1,
    Yesterday = 2,
    OlderThanTwoDays = 3,
    NeverGroomed = 4,
}
