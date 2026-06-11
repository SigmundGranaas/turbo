FROM mcr.microsoft.com/dotnet/runtime:10.0 AS base
USER $APP_UID
WORKDIR /app

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

COPY ["Directory.Packages.props", "./"]
COPY ["src/Places/Turbo.Places.Ingestion/Turbo.Places.Ingestion.csproj", "src/Places/Turbo.Places.Ingestion/"]
COPY ["src/Places/Turbo.Places.Core/Turbo.Places.Core.csproj", "src/Places/Turbo.Places.Core/"]
COPY ["src/Places/Turbo.Places.Contracts/Turbo.Places.Contracts.csproj", "src/Places/Turbo.Places.Contracts/"]
COPY ["src/Places/Turbo.Places.Infrastructure/Turbo.Places.Infrastructure.csproj", "src/Places/Turbo.Places.Infrastructure/"]
RUN dotnet restore "src/Places/Turbo.Places.Ingestion/Turbo.Places.Ingestion.csproj"

COPY . .
WORKDIR /src/src/Places/Turbo.Places.Ingestion
RUN dotnet publish "Turbo.Places.Ingestion.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
# place-core cdylib for the post-swap reverse/search verification (staged into
# the context by api_image_build.yaml; native/.gitkeep keeps the COPY valid).
COPY native/ /app/native/
ENV PLACE_CORE_LIB=/app/native
# Args supplied by the CronJob (e.g. bulk-ssr fylke 18 Nordland). The job
# reads the DB connection from PLACES_DB and uses TMPDIR for the GML extract.
ENTRYPOINT ["dotnet", "Turbo.Places.Ingestion.dll"]
