// The contents of pack versions, loaded once per version and shared by the list and the tab that counts it.
import { api } from '../../api'
import type { PackContents, ProviderId } from '../../types'

const MAX = 40
const cache = new Map<string, Promise<PackContents>>()

/** The contents of a version; `again` forgets a previous answer (after an error, to try again). */
export function loadPackContents(provider: ProviderId, packId: string, versionId: string, again = false): Promise<PackContents> {
  const key = `${provider}\n${packId}\n${versionId}`
  if (again) cache.delete(key)
  let pending = cache.get(key)
  if (!pending) {
    pending = api.packContents(provider, packId, versionId)
    pending.catch(() => cache.delete(key))
    if (cache.size >= MAX) cache.delete(cache.keys().next().value!)
    cache.set(key, pending)
  }
  return pending
}
