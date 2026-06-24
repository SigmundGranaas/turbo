import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  addItem,
  createCollection,
  deleteCollection,
  listCollections,
  removeItem,
  updateCollection,
  type Collection,
  type CollectionItem,
} from '../../api/collections';

const KEY = ['collections'];

export function useCollections() {
  return useQuery({ queryKey: KEY, queryFn: listCollections, staleTime: 30_000 });
}

export function useCollectionMutations() {
  const qc = useQueryClient();
  const invalidate = () => qc.invalidateQueries({ queryKey: KEY });
  return {
    create: useMutation({ mutationFn: (v: { name: string; colorHex?: string }) => createCollection(v.name, v.colorHex), onSuccess: invalidate }),
    update: useMutation({ mutationFn: (v: { c: Collection; changes: { name?: string; colorHex?: string } }) => updateCollection(v.c, v.changes), onSuccess: invalidate }),
    remove: useMutation({ mutationFn: (c: Collection) => deleteCollection(c), onSuccess: invalidate }),
    addItem: useMutation({ mutationFn: (v: { c: Collection; item: CollectionItem }) => addItem(v.c, v.item), onSuccess: invalidate }),
    removeItem: useMutation({ mutationFn: (v: { c: Collection; item: CollectionItem }) => removeItem(v.c, v.item), onSuccess: invalidate }),
  };
}
