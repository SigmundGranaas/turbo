# Sharing Model Design

Status: proposal
Branch: `claude/sharing-model-design-Z3qdN`

## Goals

- A single sharing primitive that every shareable entity participates in by construction — not bolted onto each type.
- Offline / anonymous use is unaffected. The sharing layer is server-side only.
- Supports direct user shares, group shares, and link shares on day one without parallel code paths.
- View and edit roles from the start. Conflict resolution uses the existing last-writer-wins on `(version, updated_at)`.
- Removing the feature is bounded: one module client-side, one schema server-side.

## Non-goals

- Real-time co-editing. Sharing rides the existing delta sync.
- Cross-instance / federated sharing.
- Granular field-level ACLs.

## The primitive

Separate "what it is" from "who can touch it". Introduce a `Resource` envelope that owns identity, ownership, visibility, and grants. Domain entities become payloads attached to a resource.

```
┌────────────────────── Sharing Service (new) ───────────────────────────┐
│  resources  ◄────────  grants  ◄──── friendships ◄──── invites         │
│                              │                                         │
│                              └── groups + group_members                │
│                                                                        │
│  IAccessControl.canRead/canWrite(userId, resourceId)                   │
└──────────────▲─────────────────────────────────────────────────────────┘
               │ every read/write is gated here, regardless of payload
    ┌──────────┼──────────────────┬─────────────────┐
    ▼          ▼                  ▼                 ▼
 Collections  Markers            Paths           Future-X
 (payload)   (payload)         (payload)      (declare type)
```

The sharing service has no knowledge of collections, markers, or paths. Adding a new shareable type means: declare a `type` string, create a payload table keyed on `resource_id`, register a handler. Zero code in the sharing module changes.

## Schema

```sql
-- The universal shareable envelope. Every shareable thing has one.
resources(
  id           uuid primary key,
  type         text not null,           -- 'collection' | 'marker' | 'path' | ...
  owner_id     uuid not null,
  visibility   text not null,           -- 'private' | 'friends' | 'unlisted_link' | 'public'
  version      bigint not null,         -- monotonic, used by delta sync
  created_at   timestamptz not null,
  updated_at   timestamptz not null,
  deleted_at   timestamptz              -- tombstone for sync
);
create index on resources(owner_id, type);
create index on resources(type, visibility);

-- One table covers user, group, and link grants. Extensible to new subject types.
grants(
  resource_id   uuid not null references resources(id) on delete cascade,
  subject_type  text not null,          -- 'user' | 'group' | 'link'
  subject_id    uuid,                   -- null only for link grants
  role          text not null,          -- 'viewer' | 'editor'
  granted_by    uuid not null,
  granted_at    timestamptz not null,
  expires_at    timestamptz,
  link_token    text unique,            -- populated only when subject_type='link'
  primary key (resource_id, subject_type, subject_id)
);
create index on grants(subject_id) where subject_type = 'user';
create index on grants(subject_id) where subject_type = 'group';

-- Friend graph (canonical order: lower_user_id < higher_user_id)
friendships(
  lower_user_id   uuid not null,
  higher_user_id  uuid not null,
  initiator_id    uuid not null,
  status          text not null,        -- 'pending' | 'accepted' | 'blocked'
  created_at      timestamptz not null,
  accepted_at     timestamptz,
  primary key (lower_user_id, higher_user_id)
);

-- Pending invites by email (resolved on signup or first-seen email)
share_invites(
  id             uuid primary key,
  inviter_id     uuid not null,
  invitee_email  citext not null,
  resource_id    uuid,                  -- null = pure friend invite
  role           text,
  expires_at     timestamptz,
  redeemed_at    timestamptz
);

-- Groups (named circles)
groups(
  id          uuid primary key,
  owner_id    uuid not null,
  name        text not null,
  created_at  timestamptz not null,
  updated_at  timestamptz not null
);

group_members(
  group_id   uuid not null references groups(id) on delete cascade,
  user_id    uuid not null,
  role       text not null,             -- 'admin' | 'member'
  joined_at  timestamptz not null,
  primary key (group_id, user_id)
);

-- Payloads: domain tables PK on resource_id. Domain fields only.
collections(resource_id uuid primary key references resources(id) on delete cascade, name text, ...);
markers    (resource_id uuid primary key references resources(id) on delete cascade, geom geography, title text, ...);
paths      (resource_id uuid primary key references resources(id) on delete cascade, points ..., distance ...);
```

Invariant: a payload row cannot exist without its resource. Ownership, version, visibility, and audit always live on the envelope.

## Access control

A single service used by every domain handler:

```csharp
public interface IAccessControl {
    Task<bool> CanRead(UserId actor, ResourceId rid);
    Task<bool> CanWrite(UserId actor, ResourceId rid);
    Task RequireWrite(UserId actor, ResourceId rid);   // throws on deny
}
```

Resolution query (canRead; canWrite adds `role = 'editor'` and excludes link grants by default):

```sql
select 1 from resources where id = :rid and owner_id = :me
union all
select 1 from grants
   where resource_id = :rid
     and subject_type = 'user' and subject_id = :me
     and (expires_at is null or expires_at > now())
union all
select 1 from grants g
   join group_members gm on g.subject_id = gm.group_id
   where g.resource_id = :rid and g.subject_type = 'group'
     and gm.user_id = :me
     and (g.expires_at is null or g.expires_at > now())
union all
select 1 from resources
   where id = :rid and visibility = 'public';      -- public read only
```

Link grants are resolved at the HTTP boundary, not in this query. A request carrying `?token=…` is upgraded to the link's role for that resource only.

## Sync semantics

One delta endpoint replaces per-feature sync:

```
GET /resources/sync?since=<cursor>&types=collection,marker,path
→ {
    cursor: "…",
    items: [
      {
        resource: { id, type, owner_id, version, updated_at,
                    visibility, my_role, deleted, access_revoked },
        payload: { ...type-specific... }
      }, ...
    ]
  }
```

Server-side filter:
```sql
where r.owner_id = :me
   or exists (select 1 from grants g
              where g.resource_id = r.id
                and ((g.subject_type='user' and g.subject_id=:me)
                  or (g.subject_type='group' and g.subject_id in (select group_id from group_members where user_id=:me))))
```

Client behavior:
- Iterate items; upsert the envelope into a small `role_cache`; dispatch the payload by `resource.type` to the typed repository.
- `access_revoked=true` → local tombstone, distinguishable from `deleted=true` for messaging.
- Writes from non-owner/non-editor return 403; client treats this as a revocation tombstone.

Group-membership changes propagate by bumping `resources.version` on all resources granted to the changed group via a server-side trigger. The standard `?since=` cursor picks them up on next sync.

## Module layout

### Server (.NET modulith)

```
apps/api/src/
  Sharing/                          ← NEW. Owns resources, grants, friendships, groups, invites.
    Turbo.Sharing.Core/             IAccessControl, command handlers, domain events
    Turbo.Sharing.Infrastructure/   EF Core for resources/grants/friendships/groups
    Turbo.Sharing.Api/              /resources, /resources/:id/grants, /friends, /groups, /invites

  Collections/                      ← refactor: payload only. Drops OwnerId. Injects IAccessControl.
  Geo/ (markers, paths)             ← same pattern.
  Auth/                             ← unchanged.
```

Domain handlers never authorize themselves:

```csharp
public async Task UpdateCollection(UserId actor, ResourceId rid, CollectionPatch patch) {
    await accessControl.RequireWrite(actor, rid);
    await collectionRepo.Apply(rid, patch);
    await resourceRepo.BumpVersion(rid);
}
```

### Client (Flutter)

```
lib/features/sharing/                ← absorbs existing link-share module + new social layer
  models/   resource.dart, grant.dart, friendship.dart, group.dart, share_invite.dart
  data/     role_cache_repository.dart, sharing_api_client.dart
  providers/
    resourceProvider(id)             → Resource envelope (cached from sync)
    visibleResourcesProvider(type)   → owned + shared, merged
    canEditProvider(resourceId)      → bool          ← used by every edit-capable widget
    friendsProvider                  → accepted friends
    groupsProvider                   → my groups
  widgets/
    ShareSheet(resourceId)           ← universal. Same widget for any payload type.
    FriendPicker, GroupPicker, IncomingShareBanner, FriendsPage, GroupsPage
  api.dart

lib/features/collections/            ← payload only
  models/collection.dart             (drops ownerId; gains resourceId == id)
  data/collection_repository.dart

lib/features/markers/                ← same shape
lib/features/saved_paths/            ← same shape
```

Usage in a feature screen:
```dart
final canEdit = ref.watch(canEditProvider(collection.id));
...
ShareSheet(resourceId: collection.id);
```

## Offline / anonymous compatibility

The sharing layer must not interfere with users running the app without signing in.

- The client local DB never has a `resources` table. Domain entities keep their UUID PKs and work identically online or offline.
- The same UUID a client generates locally becomes the `resource.id` server-side on first sync. No remapping.
- On first authenticated sync, the server lazily creates `resources` rows (`owner_id = syncing user`, `visibility = 'private'`) for any UUIDs it hasn't seen.
- `canEditProvider`:
  - Unauthenticated: short-circuits to `true`. Anonymous users own everything they can see locally.
  - Authenticated: consults `role_cache`. Cache miss = treat as owner.
- UI affordances (`ShareSheet`, `FriendsPage`, `GroupsPage`) are gated on **authentication status**, not a feature flag. Anonymous users never see them.
- `/sharing/*` and `/resources/*/grants` endpoints are simply not called when offline. No network attempts means no interference.

Net effect: an offline/anonymous user runs the app exactly as today. Sharing is invisible until sign-in *and* a sharing action.

## Sharing modes on one primitive

Every new sharing mode is a new `subject_type` (and possibly a resolution table). The access check is the only place that knows how to resolve each subject type. Everything else is generic.

| Mode                | `subject_type`         | Resolution                              | Status      |
|---------------------|------------------------|-----------------------------------------|-------------|
| Direct friend       | `user`                 | `subject_id = me`                       | Day 1       |
| Group / circle      | `group`                | join `group_members`                    | Day 1       |
| Link share          | `link`                 | match `link_token` query param          | Day 1       |
| Public (read)       | (on `visibility`)      | `visibility = 'public'`                 | Day 1       |
| Expiring share      | any                    | `expires_at` filter                     | Day 1       |
| Followers           | `subscribers_of:uid`   | join `subscriptions`                    | Future      |
| Org / team          | `org`                  | join `org_members`                      | Future      |

"Type of sharing" is data, not code.

## Migration

Each step is independently shippable and reversible.

1. **Introduce `resources`** + `IAccessControl` as a no-op (only owner checks).
   Backfill: one resource per existing collection/marker/path, `visibility='private'`.
   Rename `collections.id` → `collections.resource_id` (FK to resources). Same for markers, paths.
   No user-visible change.

2. **Ship `grants`, `friendships`, `/friends`** endpoints.
   Client: `FriendsPage` mounted, accessible only when authenticated.
   No entity sharing yet.

3. **Ship `ShareSheet`** + user-grant creation + invite-by-email.
   Editor role enabled in same release.

4. **Ship `groups` + `group_members`** + `GroupsPage`.
   Sharing to groups now possible via the same `ShareSheet`.

5. **Promote stateless link-share to tracked link grants.**
   Existing `/share/m`, `/share/p` URLs continue to decode locally indefinitely.
   New URLs route through `subject_type='link'` grants so the owner can revoke.

## Open questions

- Identity handle for friend lookup: friend code, email, or both?
- Notification surface for incoming friend requests and share notifications (in-app only, or also push/email)?
- Quota: max number of grants per resource? Max friends/group size?
- Tombstone retention: how long do `deleted_at`/`access_revoked` markers live before garbage collection?
