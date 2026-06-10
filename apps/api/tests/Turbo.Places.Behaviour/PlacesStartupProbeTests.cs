using FluentAssertions;
using Turboapi.Places.Core;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P0.3: the host runs a place-core call at boot and fails fast with an
/// actionable message if the native library is missing/broken — rather than
/// booting healthy and 500-ing on the first request. The core call is behind a
/// delegate so all three branches are deterministic in-process (forcing a real
/// DllNotFound mid-process is unreliable once the lib is loaded).
/// </summary>
public class PlacesStartupProbeTests
{
    [Fact]
    public void Throws_actionable_message_when_the_native_library_cannot_load()
    {
        var act = () => PlacesStartupProbe.Verify(_ => throw new DllNotFoundException("libplace_core"));

        act.Should().Throw<InvalidOperationException>()
            .WithMessage("*place-core*")
            .WithMessage("*PLACE_CORE_LIB*");
    }

    [Fact]
    public void Throws_when_the_core_returns_an_unexpected_verdict()
    {
        // Lib loads but is the wrong build / ruleset mismatch.
        var act = () => PlacesStartupProbe.Verify(_ => "{\"title\":\"Nope\",\"qualifier\":\"near\"}");

        act.Should().Throw<InvalidOperationException>();
    }

    [Fact]
    public void Passes_against_the_real_core()
    {
        PlacesHostFixtureLib.Ensure();
        var act = () => PlacesStartupProbe.Verify();
        act.Should().NotThrow();
    }
}

/// <summary>Points PLACE_CORE_LIB at the repo's built cdylib for the
/// real-core probe test (the behaviour fixture does the same for the host).</summary>
internal static class PlacesHostFixtureLib
{
    public static void Ensure() =>
        Environment.SetEnvironmentVariable("PLACE_CORE_LIB", PlacesHostFixture.FindPlaceCoreLibDir());
}
