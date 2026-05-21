using FluentAssertions;
using Turboapi.Activities.domain.exception;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class IfMatchHeaderTests
{
    [Theory]
    [InlineData("\"42\"", 42L)]
    [InlineData("W/\"42\"", 42L)]
    [InlineData("42", 42L)]
    [InlineData("  \"7\"  ", 7L)]
    [InlineData("\"0\"", 0L)]
    public void Parses_valid_etag_forms(string header, long expected)
    {
        IfMatchHeader.Parse(header).Should().Be(expected);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("\"abc\"")]
    [InlineData("not-a-number")]
    [InlineData("\"\"")]
    public void Returns_null_for_missing_or_unparseable(string? header)
    {
        IfMatchHeader.Parse(header).Should().BeNull();
    }
}
