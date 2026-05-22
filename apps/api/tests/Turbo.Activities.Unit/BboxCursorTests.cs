using FluentAssertions;
using Turboapi.Activities.controller;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class BboxCursorTests
{
    [Fact]
    public void Encode_then_decode_round_trips()
    {
        var original = new BboxCursor(
            new DateTime(2026, 5, 20, 12, 34, 56, DateTimeKind.Utc),
            Guid.Parse("0190b0a2-0000-7000-8000-000000000001"));

        var encoded = original.Encode();
        var decoded = BboxCursor.TryParse(encoded);

        decoded.Should().NotBeNull();
        decoded!.Value.UpdatedAt.Should().Be(original.UpdatedAt);
        decoded!.Value.Id.Should().Be(original.Id);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("not-base64")]
    [InlineData("dGVzdA==")] // base64 of "test", missing the colon-separated format
    public void Invalid_cursor_returns_null(string? cursor)
    {
        BboxCursor.TryParse(cursor).Should().BeNull();
    }
}
