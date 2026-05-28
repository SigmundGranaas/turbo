namespace Turboapi.Auth.Application.UseCases.Commands.UpdateProfile
{
    public record UpdateProfileCommand(
        Guid AccountId,
        string? DisplayName);
}
