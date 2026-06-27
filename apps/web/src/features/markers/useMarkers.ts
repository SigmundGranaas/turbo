import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createMarker,
  deleteMarker,
  listMarkers,
  updateMarker,
  type Marker,
  type MarkerInput,
} from './api';
import { useToast } from '../../store/toast';

const KEY = ['markers'];

export function useMarkers() {
  return useQuery({ queryKey: KEY, queryFn: listMarkers, staleTime: 30_000 });
}

// The geo/locations read-projection is eventually consistent (a few seconds
// behind a write), so invalidating right after a mutation refetches the STALE
// list — a deleted marker reappears, a new/edited one doesn't show — for
// seconds (and staleTime:30s delays the next refetch). Instead we patch the
// query cache optimistically from the authoritative mutation result; the next
// natural refetch reconciles once the projection has caught up. Mirrors the
// tracks fix (see useTracks.ts).

export function useCreateMarker() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: MarkerInput) => createMarker(input),
    onSuccess: (created) => {
      qc.setQueryData<Marker[]>(KEY, (old) => [created, ...(old ?? []).filter((m) => m.id !== created.id)]);
    },
  });
}

export function useUpdateMarker() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (m: Marker) => updateMarker(m),
    // The update response omits the bumped version, so merge the submitted
    // marker onto the cached one and bump the version locally (matches the
    // server increment) to keep the next If-Match write valid.
    onSuccess: (_res, m) => {
      qc.setQueryData<Marker[]>(KEY, (old) =>
        (old ?? []).map((x) => (x.id === m.id ? { ...x, ...m, version: x.version + 1 } : x)),
      );
    },
  });
}

export function useDeleteMarker() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (m: Marker) => deleteMarker(m),
    onSuccess: (_res, m) => {
      qc.setQueryData<Marker[]>(KEY, (old) => (old ?? []).filter((x) => x.id !== m.id));
    },
    onError: () => useToast.getState().show('Couldn’t delete the marker.'),
  });
}
