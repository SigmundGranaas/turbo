/** `collections` feature slice — collections grouping markers + tracks: the
 *  "Saved → Collections" list, a collection's detail, and the add-to-collection
 *  picker (a modal, not the panel slot). Shares the host `saved` panel slot +
 *  the `pathsStore` sub-navigation with the `tracks` slice; the collections data
 *  layer stays shared in `api/collections`. */
export { CollectionsListPanel } from './CollectionsListPanel';
export { CollectionDetailPanel } from './CollectionDetailPanel';
export { CollectionPicker } from './CollectionPicker';
export { useCollections, useCollectionMutations } from './useCollections';
