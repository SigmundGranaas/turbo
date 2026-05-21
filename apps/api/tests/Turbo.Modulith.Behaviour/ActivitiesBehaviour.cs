using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Activities.BackcountrySki.controller;
using Turboapi.Activities.BackcountrySki.controller.request;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.controller;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.Fishing.controller;
using Turboapi.Activities.Fishing.controller.request;
using Turboapi.Activities.Fishing.value;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Xunit;

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// End-to-end coverage of the activities pipeline against the modulith
/// deploy: create → in-process projection → cross-kind summaries
/// read model → bbox read with cursor pagination. We cover both a
/// Point-kind (fishing) and a LineString-kind (backcountry ski) so the
/// composition guarantees hold across geometry shapes.
///
/// Skipped automatically if no Docker daemon is reachable (the Postgres
/// Testcontainer can't start). This keeps the same xunit project usable
/// in local dev (where Docker is on) and CI (where Testcontainers is
/// pre-provisioned).
/// </summary>
[Collection("ModulithHost")]
public sealed class ActivitiesBehaviour
{
    private readonly ModulithHostFixture _host;
    public ActivitiesBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task creating_a_fishing_activity_lands_in_the_cross_kind_bbox_query()
    {
        var client = await RegisterAsync();

        var request = new CreateFishingActivityRequest
        {
            Name = "Test pond",
            Description = "behavior test",
            Longitude = 10.752,
            Latitude = 59.913,
            Details = new FishingDetailsDto
            {
                WaterKind = WaterKind.Lake,
                ShoreOrBoat = ShoreOrBoat.Shore,
                AccessNotes = "easy path from car park",
            },
        };

        var create = await client.PostAsJsonAsync("/api/activities/fishing", request);
        create.StatusCode.Should().Be(HttpStatusCode.Created,
            $"create failed: {await create.Content.ReadAsStringAsync()}");
        var created = await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>();
        created!.Id.Should().NotBeEmpty();

        var summary = await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=10.7&minLat=59.9&maxLon=10.8&maxLat=59.95");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Items.Any(i => i.Id == created.Id) == true ? body : null;
        }, description: "summary projection picks up the fishing create");

        var item = summary.Items.Single(i => i.Id == created.Id);
        item.Kind.Should().Be("fishing");
        item.Name.Should().Be("Test pond");
        item.GeometryKind.Should().Be("Point");
        summary.Truncated.Should().BeFalse();
        summary.NextCursor.Should().BeNull();
    }

    [Fact]
    public async Task updating_with_stale_etag_returns_412()
    {
        var client = await RegisterAsync();

        var create = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "Optimistic concurrency check",
            Longitude = 10.0,
            Latitude = 60.0,
            Details = new FishingDetailsDto { WaterKind = WaterKind.River, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        // Wait for the create to project — the UPDATE handler reads the
        // current version via the typed reader; without the projected row
        // it would 404 (ActivityNotFoundException) instead of returning 412.
        await Eventually.UntilAsync(async () =>
        {
            var r = await client.GetAsync($"/api/activities/fishing/{id}");
            return r.StatusCode == HttpStatusCode.OK;
        }, description: "create projects before stale-ETag update attempt");

        // A version that doesn't exist yet — the row's actual version is 1 after create.
        var staleUpdate = new HttpRequestMessage(HttpMethod.Put, $"/api/activities/fishing/{id}")
        {
            Content = JsonContent.Create(new UpdateFishingActivityRequest { Name = "Renamed" }),
        };
        staleUpdate.Headers.IfMatch.Add(new EntityTagHeaderValue("\"999\""));
        var response = await client.SendAsync(staleUpdate);

        response.StatusCode.Should().Be(HttpStatusCode.PreconditionFailed);
        var body = await response.Content.ReadFromJsonAsync<ConcurrencyErrorResponse>();
        body.Should().NotBeNull();
        body!.ExpectedVersion.Should().Be(999);
        body.ActualVersion.Should().BeGreaterThan(0);
    }

    [Fact]
    public async Task bbox_pagination_returns_cursor_when_truncated()
    {
        var client = await RegisterAsync();

        // Create a small batch and ask for a tiny page so we trigger truncation.
        for (var i = 0; i < 4; i++)
        {
            var r = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
            {
                Name = $"Pager {i}",
                Longitude = 11.0 + i * 0.001,
                Latitude = 60.0,
                Details = new FishingDetailsDto { WaterKind = WaterKind.Lake, ShoreOrBoat = ShoreOrBoat.Shore },
            });
            r.StatusCode.Should().Be(HttpStatusCode.Created);
        }

        var first = await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=10.9&minLat=59.95&maxLon=11.1&maxLat=60.05&limit=2");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Truncated == true ? body : null;
        }, description: "first bbox page is marked truncated when batch exceeds the limit");

        first.Items.Should().HaveCount(2);
        first.NextCursor.Should().NotBeNullOrEmpty();

        var second = await client.GetAsync(
            $"/api/activities/summaries/bbox?minLon=10.9&minLat=59.95&maxLon=11.1&maxLat=60.05&limit=2&cursor={Uri.EscapeDataString(first.NextCursor!)}");
        second.StatusCode.Should().Be(HttpStatusCode.OK);
        var secondBody = await second.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
        secondBody!.Items.Should().NotBeEmpty();
        secondBody.Items.Select(i => i.Id).Should().NotIntersectWith(first.Items.Select(i => i.Id));
    }

    [Fact]
    public async Task creating_a_backcountry_ski_route_lands_in_bbox_query_with_linestring_geometry()
    {
        var client = await RegisterAsync();

        var request = new CreateBackcountrySkiActivityRequest
        {
            Name = "Test ridge",
            Description = "behavior test",
            RouteWkt = "LINESTRING(8.50 61.20, 8.51 61.21, 8.52 61.22)",
            Details = new BackcountrySkiDetailsDto
            {
                AscentMeters = 800,
                DescentMeters = 800,
                DistanceMeters = 6000,
                ElevationMinMeters = 1100,
                ElevationMaxMeters = 1900,
                AtesRating = AtesRating.Challenging,
                DominantAspect = Aspect.N,
                VarsomRegionId = 3014,
            },
        };

        var create = await client.PostAsJsonAsync("/api/activities/backcountry-ski", request);
        create.StatusCode.Should().Be(HttpStatusCode.Created,
            $"create failed: {await create.Content.ReadAsStringAsync()}");
        var created = await create.Content.ReadFromJsonAsync<CreateBackcountrySkiActivityResponse>();
        created!.Id.Should().NotBeEmpty();

        var summary = await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=8.4&minLat=61.1&maxLon=8.6&maxLat=61.3");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Items.Any(i => i.Id == created.Id) == true ? body : null;
        }, description: "summary projection picks up the bcski create");

        var item = summary.Items.Single(i => i.Id == created.Id);
        item.Kind.Should().Be("backcountry_ski");
        item.GeometryKind.Should().Be("LineString");
    }

    [Fact]
    public async Task kind_filter_excludes_other_kinds_from_bbox_result()
    {
        var client = await RegisterAsync();

        var fishingCreate = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "Kind filter fishing",
            Longitude = 12.0,
            Latitude = 62.0,
            Details = new FishingDetailsDto { WaterKind = WaterKind.Lake, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        fishingCreate.StatusCode.Should().Be(HttpStatusCode.Created);
        var fishingId = (await fishingCreate.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        var bcskiCreate = await client.PostAsJsonAsync("/api/activities/backcountry-ski", new CreateBackcountrySkiActivityRequest
        {
            Name = "Kind filter bcski",
            RouteWkt = "LINESTRING(12.00 62.00, 12.01 62.01)",
            Details = new BackcountrySkiDetailsDto
            {
                AscentMeters = 300, DescentMeters = 300, DistanceMeters = 2000,
                ElevationMinMeters = 800, ElevationMaxMeters = 1100,
                AtesRating = AtesRating.Simple, DominantAspect = Aspect.S,
            },
        });
        bcskiCreate.StatusCode.Should().Be(HttpStatusCode.Created);
        var bcskiId = (await bcskiCreate.Content.ReadFromJsonAsync<CreateBackcountrySkiActivityResponse>())!.Id;

        var fishOnly = await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=11.9&minLat=61.9&maxLon=12.1&maxLat=62.1&kinds=fishing");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Items.Any(i => i.Id == fishingId) == true ? body : null;
        }, description: "kind=fishing filter returns the fishing summary");

        fishOnly.Items.Should().Contain(i => i.Id == fishingId);
        fishOnly.Items.Should().NotContain(i => i.Id == bcskiId);
    }

    [Fact]
    public async Task deleting_a_fishing_activity_removes_it_from_bbox_and_returns_404_on_get()
    {
        var client = await RegisterAsync();

        var create = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "To be deleted",
            Longitude = 9.0, Latitude = 58.0,
            Details = new FishingDetailsDto { WaterKind = WaterKind.Lake, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        // The bbox projection picked it up.
        await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=8.9&minLat=57.9&maxLon=9.1&maxLat=58.1");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Items.Any(i => i.Id == id) == true ? body : null;
        }, description: "create propagates to bbox before delete");

        // Delete.
        var del = await client.DeleteAsync($"/api/activities/fishing/{id}");
        del.StatusCode.Should().Be(HttpStatusCode.NoContent);

        // Detail endpoint now returns 404. Wrapped in Eventually because the
        // kind's typed projector consumes FishingActivityDeleted asynchronously
        // off the outbox — a GET right after the DELETE call can still see the
        // pre-tombstone row depending on dispatcher timing.
        await Eventually.UntilAsync(async () =>
        {
            var r = await client.GetAsync($"/api/activities/fishing/{id}");
            return r.StatusCode == HttpStatusCode.NotFound;
        }, description: "delete propagates to per-kind detail endpoint");

        // Bbox projection also drops the item.
        await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=8.9&minLat=57.9&maxLon=9.1&maxLat=58.1");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            // Return the body only once it confirms the item is gone — null
            // otherwise keeps the probe polling until the tombstone projects.
            return body!.Items.All(i => i.Id != id) ? body : null;
        }, description: "delete propagates to the bbox projection");
    }

    [Fact]
    public async Task deleting_with_stale_if_match_returns_412()
    {
        var client = await RegisterAsync();

        var create = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "Delete-concurrency check",
            Longitude = 7.5, Latitude = 57.5,
            Details = new FishingDetailsDto { WaterKind = WaterKind.River, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        // Wait for the create to project before issuing the 412 attempt —
        // the DELETE handler reads via the typed reader and a not-yet-
        // projected row would return 404 (ActivityNotFoundException) instead
        // of 412.
        await Eventually.UntilAsync(async () =>
        {
            var r = await client.GetAsync($"/api/activities/fishing/{id}");
            return r.StatusCode == HttpStatusCode.OK;
        }, description: "create projects before stale-ETag delete attempt");

        var request = new HttpRequestMessage(HttpMethod.Delete, $"/api/activities/fishing/{id}");
        request.Headers.IfMatch.Add(new EntityTagHeaderValue("\"99\""));
        var response = await client.SendAsync(request);

        response.StatusCode.Should().Be(HttpStatusCode.PreconditionFailed);

        // The actual row should still exist — delete must not have happened.
        var get = await client.GetAsync($"/api/activities/fishing/{id}");
        get.StatusCode.Should().Be(HttpStatusCode.OK);
    }

    [Fact]
    public async Task get_by_id_returns_the_typed_detail_payload_after_create()
    {
        var client = await RegisterAsync();

        var create = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "Detail roundtrip",
            Description = "with description",
            Longitude = 6.5, Latitude = 57.0,
            Details = new FishingDetailsDto
            {
                WaterKind = WaterKind.Sea,
                ShoreOrBoat = ShoreOrBoat.Boat,
                AccessNotes = "harbour access",
            },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        // Wait for the typed projector to land the row before reading detail.
        var get = await Eventually.Returns<HttpResponseMessage>(async () =>
        {
            var r = await client.GetAsync($"/api/activities/fishing/{id}");
            return r.StatusCode == HttpStatusCode.OK ? r : null;
        }, description: "fishing detail projection");
        get.StatusCode.Should().Be(HttpStatusCode.OK);

        // ETag header carries the version — clients use this for If-Match.
        get.Headers.ETag.Should().NotBeNull();
        get.Headers.ETag!.Tag.Should().Be("\"1\"");

        var body = await get.Content.ReadFromJsonAsync<FishingActivityResponse>();
        body.Should().NotBeNull();
        body!.Id.Should().Be(id);
        body.Name.Should().Be("Detail roundtrip");
        body.Description.Should().Be("with description");
        body.Longitude.Should().BeApproximately(6.5, 0.0001);
        body.Latitude.Should().BeApproximately(57.0, 0.0001);
        body.Details.WaterKind.Should().Be(WaterKind.Sea);
        body.Details.ShoreOrBoat.Should().Be(ShoreOrBoat.Boat);
        body.Details.AccessNotes.Should().Be("harbour access");
        body.Version.Should().Be(1);
    }

    [Fact]
    public async Task get_by_id_returns_404_for_an_id_owned_by_another_user()
    {
        var alice = await RegisterAsync();
        var create = await alice.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "Alice's secret",
            Longitude = 5.0, Latitude = 60.0,
            Details = new FishingDetailsDto { WaterKind = WaterKind.Lake, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        var bob = await RegisterAsync();
        var get = await bob.GetAsync($"/api/activities/fishing/{id}");
        get.StatusCode.Should().Be(HttpStatusCode.NotFound,
            "an activity owned by another user must look like it does not exist " +
            "to a different caller — no enumeration leak through 403");
    }

    [Fact]
    public async Task conditions_endpoint_returns_a_typed_report_for_an_owned_activity()
    {
        var client = await RegisterAsync();

        var create = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "Conditions check",
            Longitude = 5.5, Latitude = 60.5,
            Details = new FishingDetailsDto { WaterKind = WaterKind.Lake, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        // Conditions reads the typed activity via IFishingActivityReader, which
        // depends on the projector consuming FishingActivityCreated. Poll.
        var conditions = await Eventually.Returns<HttpResponseMessage>(async () =>
        {
            var r = await client.GetAsync($"/api/activities/fishing/{id}/conditions");
            return r.StatusCode == HttpStatusCode.OK ? r : null;
        }, description: "fishing conditions endpoint reaches OK");

        var report = await conditions.Content.ReadFromJsonAsync<FishingConditionsReport>();
        report.Should().NotBeNull();
        report!.ActivityId.Should().Be(id);
        report.Weather.Should().NotBeNull();
        // The synthetic weather provider always returns deterministic values
        // so we can assert the shape, not the magnitude.
        report.Weather.WindSpeedMs.Should().BeGreaterThanOrEqualTo(0);
        report.Rationale.Should().NotBeNullOrEmpty();
    }

    [Fact]
    public async Task conditions_endpoint_returns_404_for_someone_elses_activity()
    {
        var alice = await RegisterAsync();
        var create = await alice.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "Alice's pond",
            Longitude = 4.0, Latitude = 62.0,
            Details = new FishingDetailsDto { WaterKind = WaterKind.Lake, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        var bob = await RegisterAsync();
        var get = await bob.GetAsync($"/api/activities/fishing/{id}/conditions");
        get.StatusCode.Should().Be(HttpStatusCode.NotFound);
    }

    [Fact]
    public async Task update_chain_bumps_version_monotonically_and_projects_each_revision()
    {
        var client = await RegisterAsync();

        var create = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "v1",
            Longitude = 3.0, Latitude = 63.0,
            Details = new FishingDetailsDto { WaterKind = WaterKind.River, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        // Wait for the create to project — the UPDATE handler reads the
        // current version via the typed reader and would 404 if the row
        // isn't there yet.
        await Eventually.UntilAsync(async () =>
        {
            var r = await client.GetAsync($"/api/activities/fishing/{id}");
            return r.StatusCode == HttpStatusCode.OK;
        }, description: "create projects before first update");

        // Update once with no precondition.
        var u1 = await client.PutAsJsonAsync($"/api/activities/fishing/{id}",
            new UpdateFishingActivityRequest { Name = "v2" });
        u1.StatusCode.Should().Be(HttpStatusCode.NoContent);

        // Wait for the v2 projection to land before reading.
        var body1 = await Eventually.Returns<FishingActivityResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/activities/fishing/{id}");
            if (r.StatusCode != HttpStatusCode.OK) return null;
            var body = await r.Content.ReadFromJsonAsync<FishingActivityResponse>();
            return body?.Version == 2 ? body : null;
        }, description: "v2 projection lands");
        body1.Name.Should().Be("v2");

        // Update again with a matching If-Match.
        var u2 = new HttpRequestMessage(HttpMethod.Put, $"/api/activities/fishing/{id}")
        {
            Content = JsonContent.Create(new UpdateFishingActivityRequest { Name = "v3" }),
        };
        u2.Headers.IfMatch.Add(new EntityTagHeaderValue("\"2\""));
        var r2 = await client.SendAsync(u2);
        r2.StatusCode.Should().Be(HttpStatusCode.NoContent);

        var body2 = await Eventually.Returns<FishingActivityResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/activities/fishing/{id}");
            if (r.StatusCode != HttpStatusCode.OK) return null;
            var body = await r.Content.ReadFromJsonAsync<FishingActivityResponse>();
            return body?.Version == 3 ? body : null;
        }, description: "v3 projection lands");
        body2.Name.Should().Be("v3");

        // Projection has the latest name.
        await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=2.9&minLat=62.9&maxLon=3.1&maxLat=63.1");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            var hit = body?.Items.FirstOrDefault(i => i.Id == id);
            return hit?.Name == "v3" ? body : null;
        }, description: "summary projection mirrors the latest name");
    }

    private async Task<HttpClient> RegisterAsync()
    {
        var client = _host.CreateClient();
        var email = $"activities-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";
        var register = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, password, password));
        register.IsSuccessStatusCode.Should().BeTrue(
            $"register precondition failed: {register.StatusCode}: {await register.Content.ReadAsStringAsync()}");
        var tokens = (await register.Content.ReadFromJsonAsync<AuthTokenResponse>())!;
        var authed = _host.CreateClient();
        authed.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", tokens.AccessToken);
        return authed;
    }
}
