FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS base
USER $APP_UID
WORKDIR /app
EXPOSE 8080

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

COPY ["Directory.Packages.props", "./"]
COPY ["hosts/Turbo.Host.Activities/Turbo.Host.Activities.csproj", "hosts/Turbo.Host.Activities/"]

# Shared activities module + every kind's four assemblies (Api/Core/Contracts/Infrastructure).
COPY ["src/Activities/Turbo.Activities.Shared.Api/Turbo.Activities.Shared.Api.csproj", "src/Activities/Turbo.Activities.Shared.Api/"]
COPY ["src/Activities/Turbo.Activities.Shared.Core/Turbo.Activities.Shared.Core.csproj", "src/Activities/Turbo.Activities.Shared.Core/"]
COPY ["src/Activities/Turbo.Activities.Shared.Contracts/Turbo.Activities.Shared.Contracts.csproj", "src/Activities/Turbo.Activities.Shared.Contracts/"]
COPY ["src/Activities/Turbo.Activities.Shared.Infrastructure/Turbo.Activities.Shared.Infrastructure.csproj", "src/Activities/Turbo.Activities.Shared.Infrastructure/"]
COPY ["src/Activities/Turbo.Activities.Fishing.Api/Turbo.Activities.Fishing.Api.csproj", "src/Activities/Turbo.Activities.Fishing.Api/"]
COPY ["src/Activities/Turbo.Activities.Fishing.Core/Turbo.Activities.Fishing.Core.csproj", "src/Activities/Turbo.Activities.Fishing.Core/"]
COPY ["src/Activities/Turbo.Activities.Fishing.Contracts/Turbo.Activities.Fishing.Contracts.csproj", "src/Activities/Turbo.Activities.Fishing.Contracts/"]
COPY ["src/Activities/Turbo.Activities.Fishing.Infrastructure/Turbo.Activities.Fishing.Infrastructure.csproj", "src/Activities/Turbo.Activities.Fishing.Infrastructure/"]
COPY ["src/Activities/Turbo.Activities.BackcountrySki.Api/Turbo.Activities.BackcountrySki.Api.csproj", "src/Activities/Turbo.Activities.BackcountrySki.Api/"]
COPY ["src/Activities/Turbo.Activities.BackcountrySki.Core/Turbo.Activities.BackcountrySki.Core.csproj", "src/Activities/Turbo.Activities.BackcountrySki.Core/"]
COPY ["src/Activities/Turbo.Activities.BackcountrySki.Contracts/Turbo.Activities.BackcountrySki.Contracts.csproj", "src/Activities/Turbo.Activities.BackcountrySki.Contracts/"]
COPY ["src/Activities/Turbo.Activities.BackcountrySki.Infrastructure/Turbo.Activities.BackcountrySki.Infrastructure.csproj", "src/Activities/Turbo.Activities.BackcountrySki.Infrastructure/"]
COPY ["src/Activities/Turbo.Activities.Hiking.Api/Turbo.Activities.Hiking.Api.csproj", "src/Activities/Turbo.Activities.Hiking.Api/"]
COPY ["src/Activities/Turbo.Activities.Hiking.Core/Turbo.Activities.Hiking.Core.csproj", "src/Activities/Turbo.Activities.Hiking.Core/"]
COPY ["src/Activities/Turbo.Activities.Hiking.Contracts/Turbo.Activities.Hiking.Contracts.csproj", "src/Activities/Turbo.Activities.Hiking.Contracts/"]
COPY ["src/Activities/Turbo.Activities.Hiking.Infrastructure/Turbo.Activities.Hiking.Infrastructure.csproj", "src/Activities/Turbo.Activities.Hiking.Infrastructure/"]
COPY ["src/Activities/Turbo.Activities.XcSki.Api/Turbo.Activities.XcSki.Api.csproj", "src/Activities/Turbo.Activities.XcSki.Api/"]
COPY ["src/Activities/Turbo.Activities.XcSki.Core/Turbo.Activities.XcSki.Core.csproj", "src/Activities/Turbo.Activities.XcSki.Core/"]
COPY ["src/Activities/Turbo.Activities.XcSki.Contracts/Turbo.Activities.XcSki.Contracts.csproj", "src/Activities/Turbo.Activities.XcSki.Contracts/"]
COPY ["src/Activities/Turbo.Activities.XcSki.Infrastructure/Turbo.Activities.XcSki.Infrastructure.csproj", "src/Activities/Turbo.Activities.XcSki.Infrastructure/"]
COPY ["src/Activities/Turbo.Activities.Packrafting.Api/Turbo.Activities.Packrafting.Api.csproj", "src/Activities/Turbo.Activities.Packrafting.Api/"]
COPY ["src/Activities/Turbo.Activities.Packrafting.Core/Turbo.Activities.Packrafting.Core.csproj", "src/Activities/Turbo.Activities.Packrafting.Core/"]
COPY ["src/Activities/Turbo.Activities.Packrafting.Contracts/Turbo.Activities.Packrafting.Contracts.csproj", "src/Activities/Turbo.Activities.Packrafting.Contracts/"]
COPY ["src/Activities/Turbo.Activities.Packrafting.Infrastructure/Turbo.Activities.Packrafting.Infrastructure.csproj", "src/Activities/Turbo.Activities.Packrafting.Infrastructure/"]
COPY ["src/Activities/Turbo.Activities.Freediving.Api/Turbo.Activities.Freediving.Api.csproj", "src/Activities/Turbo.Activities.Freediving.Api/"]
COPY ["src/Activities/Turbo.Activities.Freediving.Core/Turbo.Activities.Freediving.Core.csproj", "src/Activities/Turbo.Activities.Freediving.Core/"]
COPY ["src/Activities/Turbo.Activities.Freediving.Contracts/Turbo.Activities.Freediving.Contracts.csproj", "src/Activities/Turbo.Activities.Freediving.Contracts/"]
COPY ["src/Activities/Turbo.Activities.Freediving.Infrastructure/Turbo.Activities.Freediving.Infrastructure.csproj", "src/Activities/Turbo.Activities.Freediving.Infrastructure/"]

COPY ["TurboAuthentication/TurboAuthentication.csproj", "TurboAuthentication/"]
COPY ["src/Shared/Turbo.Messaging.Abstractions/Turbo.Messaging.Abstractions.csproj", "src/Shared/Turbo.Messaging.Abstractions/"]
COPY ["src/Shared/Turbo.Messaging.Nats/Turbo.Messaging.Nats.csproj", "src/Shared/Turbo.Messaging.Nats/"]
COPY ["src/Shared/Turbo.Outbox.Abstractions/Turbo.Outbox.Abstractions.csproj", "src/Shared/Turbo.Outbox.Abstractions/"]
COPY ["src/Shared/Turbo.Outbox.Postgres/Turbo.Outbox.Postgres.csproj", "src/Shared/Turbo.Outbox.Postgres/"]
COPY ["src/Shared/Turbo.Hosting.Postgres/Turbo.Hosting.Postgres.csproj", "src/Shared/Turbo.Hosting.Postgres/"]
RUN dotnet restore "hosts/Turbo.Host.Activities/Turbo.Host.Activities.csproj"

COPY . .
WORKDIR /src/hosts/Turbo.Host.Activities
RUN dotnet publish "Turbo.Host.Activities.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
ENTRYPOINT ["dotnet", "Turbo.Host.Activities.dll"]
