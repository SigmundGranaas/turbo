using System.Text;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Authentication.Cookies;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.EntityFrameworkCore;
using Microsoft.IdentityModel.Tokens;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Auth.Application.Behaviors;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Application.UseCases.Commands.AuthenticateWithOAuth;
using Turboapi.Auth.Application.UseCases.Commands.LoginUserWithPassword;
using Turboapi.Auth.Application.UseCases.Commands.RefreshToken;
using Turboapi.Auth.Application.UseCases.Commands.RegisterUserWithPassword;
using Turboapi.Auth.Application.UseCases.Commands.RevokeRefreshToken;
using Turboapi.Auth.Application.UseCases.Queries.ValidateSession;
using Turboapi.Auth.Domain.Interfaces;
using Turboapi.Auth.Infrastructure.Auth;
using Turboapi.Auth.Infrastructure.Auth.OAuthProviders;
using Turboapi.Auth.Infrastructure.Persistence;
using Turboapi.Auth.Infrastructure.Persistence.Repositories;
using Turboapi.Auth.Presentation.Controllers;
using Turboapi.Auth.Presentation.Cookies;
using Turboapi.Auth.Presentation.Security;

namespace Turboapi.Auth;

/// <summary>
/// Composition entry point for the Auth module. Wires persistence,
/// the UnitOfWork + outbox path, the JWT issuance pipeline, OAuth
/// adapters, and the controllers. Owns the shared auth scheme
/// (Cookie + JwtBearer) because Auth is the module that issues the
/// tokens the other modules validate.
/// </summary>
public static class AuthModule
{
    public const string ConnectionStringName = "Auth";

    public static IServiceCollection AddAuthModule(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var connectionString = ResolveConnectionString(configuration);

        services.AddDbContext<AuthDbContext>(options =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.EnableRetryOnFailure();
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        services.AddScoped<IAccountRepository, AccountRepository>();
        services.AddScoped<IRefreshTokenRepository, RefreshTokenRepository>();
        services.AddScoped<IOutbox<AuthScope>, PgOutbox<AuthDbContext, AuthScope>>();
        // Auth's UoW does the aggregate-event drain in addition to the
        // execution-strategy SaveChanges that PgUnitOfWork would do
        // generically; it owns the IUnitOfWork<AuthScope> binding.
        services.AddScoped<IUnitOfWork<AuthScope>, AuthUnitOfWork>();
        services.AddScoped<IIdempotencyStore<AuthDbContext>, PgIdempotencyStore<AuthDbContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<AuthDbContext>>();

        services.AddCommandHandler<RegisterUserWithPasswordCommand, Result<AuthTokenResponse, RegistrationError>, RegisterUserWithPasswordCommandHandler>();
        services.AddCommandHandler<LoginUserWithPasswordCommand, Result<AuthTokenResponse, LoginError>, LoginUserWithPasswordCommandHandler>();
        services.AddCommandHandler<RefreshTokenCommand, Result<AuthTokenResponse, RefreshTokenError>, RefreshTokenCommandHandler>();
        services.AddCommandHandler<AuthenticateWithOAuthCommand, Result<AuthTokenResponse, OAuthLoginError>, AuthenticateWithOAuthCommandHandler>();
        services.AddCommandHandler<RevokeRefreshTokenCommand, Result<RefreshTokenError>, RevokeRefreshTokenCommandHandler>();
        services.AddScoped<ValidateSessionQueryHandler>();

        services.AddHttpClient();
        services.AddScoped<IPasswordHasher, PasswordHasher>();
        services.AddScoped<IAuthTokenService, JwtService>();
        services.Configure<JwtConfig>(configuration.GetSection("Jwt"));
        services.Configure<CookieSettings>(configuration.GetSection("Cookie"));
        services.AddScoped<Turboapi.Auth.Presentation.Cookies.ICookieManager, CookieManager>();
        services.Configure<GoogleAuthSettings>(configuration.GetSection("Authentication:Google"));
        services.AddHttpClient<GoogleOAuthAdapter>();
        services.AddScoped<IOAuthProviderAdapter, GoogleOAuthAdapter>();

        services.AddHttpContextAccessor();
        services.AddControllers().AddApplicationPart(typeof(AuthController).Assembly);

        services.AddTurboSharedAuthentication(configuration);

        return services;
    }

    /// <summary>
    /// Registers the Cookie + JwtBearer authentication scheme that the
    /// other modules also validate against. Idempotent across modules
    /// — only the first call sets up the scheme; subsequent calls in
    /// the same DI container are no-ops via TryAdd.
    /// </summary>
    public static IServiceCollection AddTurboSharedAuthentication(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var jwtConfig = configuration.GetSection("Jwt").Get<JwtConfig>()
                        ?? throw new InvalidOperationException("Jwt configuration section is missing");

        services.AddSingleton(jwtConfig);
        services.AddSingleton<ISecureDataFormat<AuthenticationTicket>, JwtDataFormat>();

        // JwtBearer is the default for both authenticate and challenge so
        // [Authorize] on cross-module controllers (Activity, Geo) works with
        // the Bearer header issued by Auth. The Cookie scheme remains
        // registered for the SessionController which opts into it
        // explicitly via [Authorize(AuthenticationSchemes = "...")].
        var authBuilder = services
            .AddAuthentication(opt =>
            {
                opt.DefaultScheme = JwtBearerDefaults.AuthenticationScheme;
                opt.DefaultAuthenticateScheme = JwtBearerDefaults.AuthenticationScheme;
                opt.DefaultChallengeScheme = JwtBearerDefaults.AuthenticationScheme;
            })
            .AddCookie(options =>
            {
                options.Cookie.Name = CookieManager.AccessTokenCookieName;
                options.Cookie.HttpOnly = true;
                options.Cookie.SameSite = SameSiteMode.Lax;
                options.Cookie.SecurePolicy = CookieSecurePolicy.Always;
                // The login/OAuth flow stores a RAW JWT in this cookie (not an
                // encrypted ASP.NET ticket), so the cookie handler must decode it
                // with our JWT validator instead of the default data protector.
                // Without this the Cookie scheme can't read the cookie and every
                // cookie-authed request (the web app) 401s — the bearer-header
                // JwtBearer scheme that the native apps use is unaffected.
                options.TicketDataFormat = new JwtDataFormat(jwtConfig);
                options.Events.OnRedirectToLogin = context =>
                {
                    context.Response.StatusCode = StatusCodes.Status401Unauthorized;
                    return Task.CompletedTask;
                };
                options.Events.OnRedirectToAccessDenied = context =>
                {
                    context.Response.StatusCode = StatusCodes.Status403Forbidden;
                    return Task.CompletedTask;
                };
            })
            .AddJwtBearer(options =>
            {
                options.TokenValidationParameters = new TokenValidationParameters
                {
                    ValidateIssuerSigningKey = true,
                    IssuerSigningKey = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(jwtConfig.Key)),
                    ValidateIssuer = true,
                    ValidIssuer = jwtConfig.Issuer,
                    ValidateAudience = true,
                    ValidAudience = jwtConfig.Audience,
                    ValidateLifetime = true,
                    ClockSkew = TimeSpan.Zero
                };
            });

        services.AddAuthorization();
        return services;
    }

    private static string ResolveConnectionString(IConfiguration configuration) =>
        configuration.GetConnectionString(ConnectionStringName)
            ?? throw new InvalidOperationException(
                $"ConnectionStrings:{ConnectionStringName} is not configured");
}

public static class CommandHandlerServiceCollectionExtensions
{
    public static IServiceCollection AddCommandHandler<TCommand, TResponse, THandler>(this IServiceCollection services)
        where THandler : class, ICommandHandler<TCommand, TResponse>
    {
        services.AddScoped<THandler>();
        services.AddScoped<ICommandHandler<TCommand, TResponse>>(provider =>
            new UnitOfWorkCommandHandlerDecorator<TCommand, TResponse>(
                provider.GetRequiredService<THandler>(),
                provider.GetRequiredService<IUnitOfWork<AuthScope>>())
        );
        return services;
    }
}
