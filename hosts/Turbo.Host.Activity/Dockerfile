FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS base
USER $APP_UID
WORKDIR /app
EXPOSE 8080

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

COPY ["Directory.Packages.props", "./"]
COPY ["hosts/Turbo.Host.Activity/Turbo.Host.Activity.csproj", "hosts/Turbo.Host.Activity/"]
COPY ["src/Activity/Turbo.Activity.Api/Turbo.Activity.Api.csproj", "src/Activity/Turbo.Activity.Api/"]
COPY ["src/Activity/Turbo.Activity.Core/Turbo.Activity.Core.csproj", "src/Activity/Turbo.Activity.Core/"]
COPY ["src/Activity/Turbo.Activity.Contracts/Turbo.Activity.Contracts.csproj", "src/Activity/Turbo.Activity.Contracts/"]
COPY ["src/Activity/Turbo.Activity.Infrastructure/Turbo.Activity.Infrastructure.csproj", "src/Activity/Turbo.Activity.Infrastructure/"]
COPY ["Turbo-pg-data/Turbo-pg-data.csproj", "Turbo-pg-data/"]
COPY ["src/Shared/Turbo.Messaging.Abstractions/Turbo.Messaging.Abstractions.csproj", "src/Shared/Turbo.Messaging.Abstractions/"]
COPY ["src/Shared/Turbo.Messaging.Nats/Turbo.Messaging.Nats.csproj", "src/Shared/Turbo.Messaging.Nats/"]
COPY ["src/Shared/Turbo.Outbox.Abstractions/Turbo.Outbox.Abstractions.csproj", "src/Shared/Turbo.Outbox.Abstractions/"]
COPY ["src/Shared/Turbo.Outbox.Postgres/Turbo.Outbox.Postgres.csproj", "src/Shared/Turbo.Outbox.Postgres/"]
RUN dotnet restore "hosts/Turbo.Host.Activity/Turbo.Host.Activity.csproj"

COPY . .
WORKDIR /src/hosts/Turbo.Host.Activity
RUN dotnet publish "Turbo.Host.Activity.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
ENTRYPOINT ["dotnet", "Turbo.Host.Activity.dll"]
