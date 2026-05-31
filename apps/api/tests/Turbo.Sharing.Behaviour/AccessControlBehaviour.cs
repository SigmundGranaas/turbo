using FluentAssertions;
using Turboapi.Sharing.data;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing;
using Turboapi.Sharing.value;
using Xunit;

namespace Turbo.Sharing.Behaviour;

[Collection("SharingDatabase")]
public sealed class AccessControlBehaviour : IAsyncLifetime
{
    private readonly SharingDatabaseFixture _db;

    public AccessControlBehaviour(SharingDatabaseFixture db) => _db = db;

    public Task InitializeAsync() => _db.ResetAsync();
    public Task DisposeAsync() => Task.CompletedTask;

    [Fact]
    public async Task owner_has_full_access()
    {
        var owner = Guid.NewGuid();
        var rid = await CreateResource(owner);
        var ac = NewControl();

        (await ac.CanReadAsync(owner, rid)).Should().BeTrue();
        (await ac.CanWriteAsync(owner, rid)).Should().BeTrue();
        (await ac.EffectiveRoleAsync(owner, rid)).Should().Be(EffectiveRole.Owner);
    }

    [Fact]
    public async Task non_owner_with_no_grant_has_no_access()
    {
        var owner = Guid.NewGuid();
        var stranger = Guid.NewGuid();
        var rid = await CreateResource(owner);
        var ac = NewControl();

        (await ac.CanReadAsync(stranger, rid)).Should().BeFalse();
        (await ac.CanWriteAsync(stranger, rid)).Should().BeFalse();
        (await ac.EffectiveRoleAsync(stranger, rid)).Should().BeNull();
    }

    [Fact]
    public async Task viewer_grant_confers_read_but_not_write()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var rid = await CreateResource(owner);
        await GrantToUser(rid, friend, Role.Viewer, owner);
        var ac = NewControl();

        (await ac.CanReadAsync(friend, rid)).Should().BeTrue();
        (await ac.CanWriteAsync(friend, rid)).Should().BeFalse();
        (await ac.EffectiveRoleAsync(friend, rid)).Should().Be(EffectiveRole.Viewer);
    }

    [Fact]
    public async Task editor_grant_confers_read_and_write()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var rid = await CreateResource(owner);
        await GrantToUser(rid, friend, Role.Editor, owner);
        var ac = NewControl();

        (await ac.CanReadAsync(friend, rid)).Should().BeTrue();
        (await ac.CanWriteAsync(friend, rid)).Should().BeTrue();
        (await ac.EffectiveRoleAsync(friend, rid)).Should().Be(EffectiveRole.Editor);
    }

    [Fact]
    public async Task expired_grant_is_ignored()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var rid = await CreateResource(owner);
        await GrantToUser(rid, friend, Role.Editor, owner, expiresAt: DateTime.UtcNow.AddDays(-1));
        var ac = NewControl();

        (await ac.CanReadAsync(friend, rid)).Should().BeFalse();
        (await ac.EffectiveRoleAsync(friend, rid)).Should().BeNull();
    }

    [Fact]
    public async Task group_grant_reaches_every_member()
    {
        var owner = Guid.NewGuid();
        var memberA = Guid.NewGuid();
        var memberB = Guid.NewGuid();
        var stranger = Guid.NewGuid();

        var rid = await CreateResource(owner);
        var groupId = await CreateGroup(owner, "Ski crew", new[] { memberA, memberB });
        await GrantToGroup(rid, groupId, Role.Editor, owner);

        var ac = NewControl();
        (await ac.CanWriteAsync(memberA, rid)).Should().BeTrue();
        (await ac.CanWriteAsync(memberB, rid)).Should().BeTrue();
        (await ac.CanReadAsync(stranger, rid)).Should().BeFalse();
    }

    [Fact]
    public async Task most_permissive_role_wins_when_user_and_group_grant_overlap()
    {
        var owner = Guid.NewGuid();
        var member = Guid.NewGuid();
        var rid = await CreateResource(owner);
        var groupId = await CreateGroup(owner, "Buddies", new[] { member });
        await GrantToUser(rid, member, Role.Viewer, owner);
        await GrantToGroup(rid, groupId, Role.Editor, owner);

        var ac = NewControl();
        (await ac.EffectiveRoleAsync(member, rid)).Should().Be(EffectiveRole.Editor);
    }

    [Fact]
    public async Task public_visibility_confers_viewer_access_to_anyone()
    {
        var owner = Guid.NewGuid();
        var anyone = Guid.NewGuid();
        var rid = await CreateResource(owner, Visibility.Public);

        var ac = NewControl();
        (await ac.CanReadAsync(anyone, rid)).Should().BeTrue();
        (await ac.CanWriteAsync(anyone, rid)).Should().BeFalse();
        (await ac.EffectiveRoleAsync(anyone, rid)).Should().Be(EffectiveRole.Viewer);
    }

    [Fact]
    public async Task soft_deleted_resource_is_invisible_to_everyone_including_owner()
    {
        var owner = Guid.NewGuid();
        var rid = await CreateResource(owner);
        await SoftDelete(rid);

        var ac = NewControl();
        (await ac.CanReadAsync(owner, rid)).Should().BeFalse();
        (await ac.EffectiveRoleAsync(owner, rid)).Should().BeNull();
    }

    [Fact]
    public async Task link_grants_do_not_confer_user_access_through_resolver()
    {
        // Link grants are resolved at the HTTP boundary, not via this query.
        var owner = Guid.NewGuid();
        var someUser = Guid.NewGuid();
        var rid = await CreateResource(owner);
        await using (var ctx = _db.CreateContext())
        {
            ctx.Grants.Add(new GrantEntity
            {
                ResourceId = rid,
                SubjectType = SubjectType.Link.ToWire(),
                SubjectId = Guid.NewGuid(),
                Role = Role.Viewer.ToWire(),
                GrantedBy = owner,
                GrantedAt = DateTime.UtcNow,
                LinkToken = "any-token",
            });
            await ctx.SaveChangesAsync();
        }

        var ac = NewControl();
        (await ac.CanReadAsync(someUser, rid)).Should().BeFalse();
    }

    [Fact]
    public async Task require_write_throws_when_denied()
    {
        var owner = Guid.NewGuid();
        var stranger = Guid.NewGuid();
        var rid = await CreateResource(owner);
        var ac = NewControl();

        var act = () => ac.RequireWriteAsync(stranger, rid);
        await act.Should().ThrowAsync<AccessDeniedException>();
    }

    [Fact]
    public async Task require_read_succeeds_for_viewer()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var rid = await CreateResource(owner);
        await GrantToUser(rid, friend, Role.Viewer, owner);

        var ac = NewControl();
        var act = () => ac.RequireReadAsync(friend, rid);
        await act.Should().NotThrowAsync();
    }

    private EfAccessControl NewControl() => new(_db.CreateContext());

    private async Task<Guid> CreateResource(Guid owner, Visibility visibility = Visibility.Private)
    {
        var id = Guid.NewGuid();
        await using var ctx = _db.CreateContext();
        ctx.Resources.Add(new ResourceEntity
        {
            Id = id,
            Type = ResourceType.Collection,
            OwnerId = owner,
            Visibility = visibility.ToWire(),
            Version = 1,
            UpdatedAt = DateTime.UtcNow,
        });
        await ctx.SaveChangesAsync();
        return id;
    }

    private async Task GrantToUser(Guid resourceId, Guid userId, Role role, Guid grantedBy, DateTime? expiresAt = null)
    {
        await using var ctx = _db.CreateContext();
        ctx.Grants.Add(new GrantEntity
        {
            ResourceId = resourceId,
            SubjectType = SubjectType.User.ToWire(),
            SubjectId = userId,
            Role = role.ToWire(),
            GrantedBy = grantedBy,
            GrantedAt = DateTime.UtcNow,
            ExpiresAt = expiresAt,
        });
        await ctx.SaveChangesAsync();
    }

    private async Task GrantToGroup(Guid resourceId, Guid groupId, Role role, Guid grantedBy)
    {
        await using var ctx = _db.CreateContext();
        ctx.Grants.Add(new GrantEntity
        {
            ResourceId = resourceId,
            SubjectType = SubjectType.Group.ToWire(),
            SubjectId = groupId,
            Role = role.ToWire(),
            GrantedBy = grantedBy,
            GrantedAt = DateTime.UtcNow,
        });
        await ctx.SaveChangesAsync();
    }

    private async Task<Guid> CreateGroup(Guid owner, string name, IEnumerable<Guid> memberIds)
    {
        var id = Guid.NewGuid();
        await using var ctx = _db.CreateContext();
        ctx.Groups.Add(new GroupEntity
        {
            Id = id,
            OwnerId = owner,
            Name = name,
            UpdatedAt = DateTime.UtcNow,
        });
        foreach (var member in memberIds)
        {
            ctx.GroupMembers.Add(new GroupMemberEntity
            {
                GroupId = id,
                UserId = member,
                Role = "member",
                JoinedAt = DateTime.UtcNow,
            });
        }
        await ctx.SaveChangesAsync();
        return id;
    }

    private async Task SoftDelete(Guid resourceId)
    {
        await using var ctx = _db.CreateContext();
        var row = await ctx.Resources.FindAsync(resourceId);
        row!.DeletedAt = DateTime.UtcNow;
        await ctx.SaveChangesAsync();
    }
}
