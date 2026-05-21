namespace Turboapi.Auth.Application.UseCases.Commands.RegisterUserWithPassword
{
    public record RegisterUserWithPasswordCommand(
        string Email,
        string Password);
}