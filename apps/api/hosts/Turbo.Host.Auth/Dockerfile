FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS base
USER $APP_UID
WORKDIR /app
EXPOSE 8080

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

# Copy only the csprojs first so the restore layer caches when source changes.
COPY ["Directory.Packages.props", "./"]
COPY ["hosts/Turbo.Host.Auth/Turbo.Host.Auth.csproj", "hosts/Turbo.Host.Auth/"]
COPY ["src/Auth/Turbo.Auth.Api/Turbo.Auth.Api.csproj", "src/Auth/Turbo.Auth.Api/"]
COPY ["src/Auth/Turbo.Auth.Core/Turbo.Auth.Core.csproj", "src/Auth/Turbo.Auth.Core/"]
COPY ["src/Auth/Turbo.Auth.Contracts/Turbo.Auth.Contracts.csproj", "src/Auth/Turbo.Auth.Contracts/"]
COPY ["src/Auth/Turbo.Auth.Infrastructure/Turbo.Auth.Infrastructure.csproj", "src/Auth/Turbo.Auth.Infrastructure/"]
COPY ["src/Shared/Turbo.Messaging.Abstractions/Turbo.Messaging.Abstractions.csproj", "src/Shared/Turbo.Messaging.Abstractions/"]
COPY ["src/Shared/Turbo.Messaging.Nats/Turbo.Messaging.Nats.csproj", "src/Shared/Turbo.Messaging.Nats/"]
COPY ["src/Shared/Turbo.Outbox.Abstractions/Turbo.Outbox.Abstractions.csproj", "src/Shared/Turbo.Outbox.Abstractions/"]
COPY ["src/Shared/Turbo.Outbox.Postgres/Turbo.Outbox.Postgres.csproj", "src/Shared/Turbo.Outbox.Postgres/"]
COPY ["src/Shared/Turbo.Hosting.Postgres/Turbo.Hosting.Postgres.csproj", "src/Shared/Turbo.Hosting.Postgres/"]
COPY ["TurboAuthentication/TurboAuthentication.csproj", "TurboAuthentication/"]
RUN dotnet restore "hosts/Turbo.Host.Auth/Turbo.Host.Auth.csproj"

COPY . .
WORKDIR /src/hosts/Turbo.Host.Auth
RUN dotnet publish "Turbo.Host.Auth.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
ENTRYPOINT ["dotnet", "Turbo.Host.Auth.dll"]
