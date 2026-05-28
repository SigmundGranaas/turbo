namespace Turboapi.Auth.Application.UseCases.Commands.ChangePassword
{
    public record ChangePasswordCommand(
        Guid AccountId,
        string CurrentPassword,
        string NewPassword);
}
