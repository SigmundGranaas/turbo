using FluentAssertions;
using Turboapi.Places.Ingestion;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P1a: name normalisation shared by the REST sampling path and the GPKG bulk
/// path, so both produce identical canonical rows. Encodes the rules the live
/// app applies: reject empty / "Ukjent" / "Unknown" names and bare Naturbase
/// codes; fold names for trigram/FTS while keeping æ/ø/å.
/// </summary>
public class NormalizationTests
{
    [Theory]
    [InlineData("Galdhøpiggen", true)]
    [InlineData("Reine", true)]
    [InlineData("", false)]
    [InlineData("   ", false)]
    [InlineData("Ukjent", false)]
    [InlineData("ukjent", false)]
    [InlineData("Unknown", false)]
    [InlineData("VV00002858", false)] // Naturbase area code
    [InlineData("VR123", false)]
    public void Accepts_real_names_rejects_placeholders_and_codes(string name, bool accepted)
    {
        Normalization.IsUsableName(name).Should().Be(accepted);
    }

    [Theory]
    [InlineData("Galdhøpiggen", "galdhøpiggen")]   // keep ø/å/æ
    [InlineData("Bødø", "bødø")]
    [InlineData("STORVATNET", "storvatnet")]
    public void Folds_for_search_preserving_norwegian_letters(string name, string expected)
    {
        Normalization.Fold(name).Should().Be(expected);
    }
}
