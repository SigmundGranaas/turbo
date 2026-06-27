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
import { useToast } from '../../store/toast';

const KEY = ['collections'];

export function useCollections() {
  return useQuery({ queryKey: KEY, queryFn: listCollections, staleTime: 30_000 });
}

export function useCollectionMutations() {
  const qc = useQueryClient();
  const invalidate = () => qc.invalidateQueries({ queryKey: KEY });
  const fail = (msg: string) => () => useToast.getState().show(msg);
  return {
    create: useMutation({ mutationFn: (v: { name: string; colorHex?: string }) => createCollection(v.name, v.colorHex), onSuccess: invalidate, onError: fail('Couldn’t create the collection. Sign in and try again.') }),
    update: useMutation({ mutationFn: (v: { c: Collection; changes: { name?: string; colorHex?: string } }) => updateCollection(v.c, v.changes), onSuccess: invalidate, onError: fail('Couldn’t save the collection.') }),
    remove: useMutation({ mutationFn: (c: Collection) => deleteCollection(c), onSuccess: invalidate, onError: fail('Couldn’t delete the collection.') }),
    addItem: useMutation({ mutationFn: (v: { c: Collection; item: CollectionItem }) => addItem(v.c, v.item), onSuccess: invalidate, onError: fail('Couldn’t add to the collection.') }),
    removeItem: useMutation({ mutationFn: (v: { c: Collection; item: CollectionItem }) => removeItem(v.c, v.item), onSuccess: invalidate, onError: fail('Couldn’t remove from the collection.') }),
  };
}
