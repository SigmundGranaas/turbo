using FluentAssertions;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Domain.Aggregates;
using Turboapi.Auth.Domain.Interfaces;
using Xunit;

namespace Turbo.Auth.Behaviour;

/// <summary>
/// Domain-level coverage for the password-change rules that can't be set up
/// over HTTP — chiefly the OAuth-only guard, since the test host has no way
/// to mint a Google account through the public API.
/// </summary>
public sealed class AccountChangePasswordTests
{
    // Reversible "hash" that lets the test assert which password is stored
    // without depending on the real PBKDF2 implementation.
    private sealed class FakeHasher : IPasswordHasher
    {
        public string HashPassword(string password) => $"hashed:{password}";
        public bool VerifyPassword(string password, string hashedPassword)
            => hashedPassword == $"hashed:{password}";
    }

    private static Account NewPasswordAccount(string password)
    {
        var account = Account.Create(Guid.NewGuid(), "user@example.com", new[] { "User" });
        account.AddPasswordAuthMethod(password, new FakeHasher());
        return account;
    }

    [Fact]
    public void changing_with_the_correct_current_password_succeeds_and_updates_the_hash()
    {
        var hasher = new FakeHasher();
        var account = NewPasswordAccount("old-password");

        var result = account.ChangePassword("old-password", "new-password", hasher);

        result.IsSuccess.Should().BeTrue();
        var method = (PasswordAuthMethod)account.AuthenticationMethods.First();
        method.PasswordHash.Should().Be("hashed:new-password");
    }

    [Fact]
    public void changing_with_a_wrong_current_password_returns_invalid_current_password()
    {
        var hasher = new FakeHasher();
        var account = NewPasswordAccount("old-password");

        var result = account.ChangePassword("wrong", "new-password", hasher);

        result.IsFailure.Should().BeTrue();
        result.Error.Should().Be(ChangePasswordError.InvalidCurrentPassword);
    }

    [Fact]
    public void changing_the_password_on_an_oauth_only_account_is_rejected()
    {
        var hasher = new FakeHasher();
        var account = Account.Create(Guid.NewGuid(), "google-user@example.com", new[] { "User" });
        account.AddOAuthAuthMethod("Google", "google-external-id");

        var result = account.ChangePassword("anything", "new-password", hasher);

        result.IsFailure.Should().BeTrue();
        result.Error.Should().Be(ChangePasswordError.OAuthOnlyAccount);
        account.HasPasswordAuthMethod().Should().BeFalse();
    }
}
