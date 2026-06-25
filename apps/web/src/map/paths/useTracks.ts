import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createTrack, deleteTrack, listTracks, updateTrack, type Track, type TrackChanges, type TrackInput } from '../../api/tracks';

const KEY = ['tracks'];

export function useTracks() {
  return useQuery({ queryKey: KEY, queryFn: listTracks, staleTime: 30_000 });
}

// The tracks read-projection is eventually consistent (~3 s behind a write), so
// invalidating right after a mutation would refetch the STALE list and the new/
// edited/deleted track wouldn't reflect for seconds. Instead we patch the query
// cache optimistically from the (authoritative) mutation result; the next natural
// refetch reconciles once the projection has caught up.

export function useCreateTrack() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: TrackInput) => createTrack(input),
    onSuccess: (created) => {
      qc.setQueryData<Track[]>(KEY, (old) => [created, ...(old ?? []).filter((t) => t.id !== created.id)]);
    },
  });
}

export function useUpdateTrack() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ track, changes }: { track: Track; changes: TrackChanges }) => updateTrack(track, changes),
    // The update response omits version/timestamps, so merge the changes onto the
    // cached track and bump the version locally (matches the server increment).
    onSuccess: (_res, { track, changes }) => {
      qc.setQueryData<Track[]>(KEY, (old) =>
        (old ?? []).map((t) => (t.id === track.id ? { ...t, ...changes, version: t.version + 1 } : t)),
      );
    },
  });
}

export function useDeleteTrack() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (t: Track) => deleteTrack(t),
    onSuccess: (_res, t) => {
      qc.setQueryData<Track[]>(KEY, (old) => (old ?? []).filter((x) => x.id !== t.id));
    },
  });
}
