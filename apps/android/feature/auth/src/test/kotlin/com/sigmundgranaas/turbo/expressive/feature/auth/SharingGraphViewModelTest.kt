package com.sigmundgranaas.turbo.expressive.feature.auth

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.sync.FriendshipDto
import com.sigmundgranaas.turbo.expressive.core.sync.GroupDto
import com.sigmundgranaas.turbo.expressive.core.sync.GroupMemberDto
import com.sigmundgranaas.turbo.expressive.core.sync.ResourceEnvelopeDto
import com.sigmundgranaas.turbo.expressive.core.sync.ResourceSyncPageDto
import com.sigmundgranaas.turbo.expressive.core.sync.SharingRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SharingGraphViewModelTest {

    private val dispatcher = StandardTestDispatcher()

    private class FakeSharing : SharingRepository {
        var friends = mutableListOf(
            FriendshipDto(otherUserId = "u-accepted", initiatorId = "me", status = "accepted"),
            FriendshipDto(otherUserId = "u-pending", initiatorId = "u-pending", status = "pending"),
        )
        var groupList = mutableListOf(GroupDto(id = "g-1", ownerId = "me", name = "Tindetur"))
        var lookupResult: Outcome<String> = Outcome.Success("u-new")
        val requested = mutableListOf<String>()
        val accepted = mutableListOf<String>()
        val removed = mutableListOf<String>()
        val createdGroups = mutableListOf<String>()
        val addedMembers = mutableListOf<Pair<String, String>>()

        override suspend fun friendCode() = Outcome.Success("turbo-ABC")
        override suspend fun createLink(resourceId: String, role: String) = Outcome.Success("u")
        override suspend fun redeemLink(token: String) =
            Outcome.Success(com.sigmundgranaas.turbo.expressive.core.sync.LinkRedemption("r", "path", "viewer"))
        override suspend fun sharedResources(since: String?) =
            Outcome.Success(ResourceSyncPageDto(items = listOf(ResourceEnvelopeDto(id = "r-1", type = "track", myRole = "viewer"))))

        override suspend fun friendships() = Outcome.Success(friends.toList())
        override suspend fun requestFriendship(otherUserId: String): Outcome<Unit> {
            requested += otherUserId; return Outcome.Success(Unit)
        }
        override suspend fun acceptFriendship(otherUserId: String): Outcome<Unit> {
            accepted += otherUserId
            friends.replaceAll { if (it.otherUserId == otherUserId) it.copy(status = "accepted") else it }
            return Outcome.Success(Unit)
        }
        override suspend fun removeFriendship(otherUserId: String): Outcome<Unit> {
            removed += otherUserId; friends.removeAll { it.otherUserId == otherUserId }; return Outcome.Success(Unit)
        }
        override suspend fun lookupUser(code: String) = lookupResult
        override suspend fun groups() = Outcome.Success(groupList.toList())
        override suspend fun createGroup(name: String): Outcome<GroupDto> {
            createdGroups += name
            val g = GroupDto(id = "g-${groupList.size + 1}", ownerId = "me", name = name)
            groupList += g
            return Outcome.Success(g)
        }
        override suspend fun addGroupMember(groupId: String, userId: String): Outcome<Unit> {
            addedMembers += groupId to userId
            groupList.replaceAll {
                if (it.id == groupId) it.copy(members = it.members + GroupMemberDto(userId, "member")) else it
            }
            return Outcome.Success(Unit)
        }
        override suspend fun removeGroupMember(groupId: String, userId: String) = Outcome.Success(Unit)
    }

    @Before fun setUp() = Dispatchers.setMain(dispatcher)
    @After fun tearDown() = Dispatchers.resetMain()

    @Test
    fun `splits friendships into accepted and pending and loads groups + shared`() = runTest(dispatcher) {
        val vm = SharingGraphViewModel(FakeSharing())
        advanceUntilIdle()
        val s = vm.state.value
        assertEquals(listOf("u-accepted"), s.accepted.map { it.otherUserId })
        assertEquals(listOf("u-pending"), s.pending.map { it.otherUserId })
        assertEquals(1, s.groups.size)
        assertEquals(listOf("r-1"), s.shared.map { it.id })
    }

    @Test
    fun `addFriend looks up the code then sends a request`() = runTest(dispatcher) {
        val repo = FakeSharing()
        val vm = SharingGraphViewModel(repo)
        advanceUntilIdle()
        vm.addFriend("turbo-XYZ")
        advanceUntilIdle()
        assertEquals(listOf("u-new"), repo.requested)
        assertEquals("Friend request sent", vm.state.value.message)
    }

    @Test
    fun `addFriend reports when the code is unknown`() = runTest(dispatcher) {
        val repo = FakeSharing().apply { lookupResult = Outcome.Failure(RuntimeException("404")) }
        val vm = SharingGraphViewModel(repo)
        advanceUntilIdle()
        vm.addFriend("nope")
        advanceUntilIdle()
        assertTrue(repo.requested.isEmpty())
        assertEquals("No user found for that code", vm.state.value.message)
    }

    @Test
    fun `accept moves a pending friend into accepted`() = runTest(dispatcher) {
        val repo = FakeSharing()
        val vm = SharingGraphViewModel(repo)
        advanceUntilIdle()
        vm.accept("u-pending")
        advanceUntilIdle()
        assertEquals(listOf("u-pending"), repo.accepted)
        assertTrue(vm.state.value.pending.isEmpty())
        assertEquals(setOf("u-accepted", "u-pending"), vm.state.value.accepted.map { it.otherUserId }.toSet())
    }

    @Test
    fun `createGroup adds a group and refreshes`() = runTest(dispatcher) {
        val repo = FakeSharing()
        val vm = SharingGraphViewModel(repo)
        advanceUntilIdle()
        vm.createGroup("Skitur")
        advanceUntilIdle()
        assertEquals(listOf("Skitur"), repo.createdGroups)
        assertTrue(vm.state.value.groups.any { it.name == "Skitur" })
    }

    @Test
    fun `addGroupMember resolves a code before adding`() = runTest(dispatcher) {
        val repo = FakeSharing()
        val vm = SharingGraphViewModel(repo)
        advanceUntilIdle()
        vm.addGroupMember("g-1", "turbo-XYZ")
        advanceUntilIdle()
        assertEquals(listOf("g-1" to "u-new"), repo.addedMembers)
    }
}
