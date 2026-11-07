// The download queue: what the player chose in Discover waits here, with the dependencies it needs, until
// "Install all" sends everything to the backend in one go (one plan, one pool of parallel downloads).
import { api } from './api'
import { app } from './state.svelte'
import type { InstallPlan, ProjectHit, ProjectKind } from './types'

export type QueueState = 'staged' | 'running' | 'done' | 'failed'

export interface QueueItem {
  key: string
  projectId: string
  versionId: string
  versionNumber: string
  title: string
  iconUrl: string | null
  kind: ProjectKind
  instanceId: string
  bytes: number
  state: QueueState
  /** Set for something added because another item needs it. */
  neededBy?: string
  error?: string
}

class DownloadQueue {
  items = $state<QueueItem[]>([])

  private key(projectId: string, instanceId: string) {
    return `${instanceId}:${projectId}`
  }

  has(projectId: string, instanceId: string): QueueItem | undefined {
    return this.items.find((i) => i.key === this.key(projectId, instanceId))
  }

  forInstance(instanceId: string): QueueItem[] {
    return this.items.filter((i) => i.instanceId === instanceId)
  }

  /** Puts a project in the queue with the version and plan the install dialog showed: its dependencies that are
   *  not in the instance yet come with it. */
  addPlanned(hit: Pick<ProjectHit, 'id' | 'title' | 'iconUrl' | 'kind'>, instanceId: string, version: { id: string; versionNumber: string }, plan: InstallPlan) {
    const instance = app.instance(instanceId)
    const key = this.key(hit.id, instanceId)
    if (!instance || this.items.some((i) => i.key === key && i.state !== 'failed')) return
    this.items = this.items.filter((i) => i.key !== key)
    this.items.push({
      key,
      projectId: hit.id,
      versionId: version.id,
      versionNumber: version.versionNumber,
      title: hit.title,
      iconUrl: hit.iconUrl,
      kind: hit.kind,
      instanceId,
      bytes: plan.items[0]?.size ?? 0,
      state: 'staged',
    })
    for (const dep of plan.items.filter((i) => i.reason === 'dependency' && !i.installed)) {
      const depKey = this.key(dep.projectId, instanceId)
      if (this.items.some((i) => i.key === depKey)) continue
      this.items.push({
        key: depKey,
        projectId: dep.projectId,
        versionId: dep.versionId,
        versionNumber: dep.versionNumber,
        title: dep.title,
        iconUrl: null,
        kind: 'mod',
        instanceId,
        bytes: dep.size,
        state: 'staged',
        neededBy: hit.title,
      })
    }
  }

  remove(key: string) {
    const item = this.items.find((i) => i.key === key)
    if (!item || item.state === 'running') return
    this.items = this.items.filter((i) => i.key !== key)
    // a dependency nobody asks for any more goes too
    this.items = this.items.filter(
      (i) => !(i.neededBy && i.state === 'staged' && !this.items.some((o) => !o.neededBy && o.instanceId === i.instanceId && o.state === 'staged' && o.title === i.neededBy)),
    )
  }

  clearStaged(instanceId: string) {
    this.items = this.items.filter((i) => !(i.state === 'staged' && i.instanceId === instanceId))
  }

  clearFinished(instanceId?: string) {
    this.items = this.items.filter((i) => (instanceId && i.instanceId !== instanceId) || (i.state !== 'done' && i.state !== 'failed'))
  }

  /** Sends everything staged (or failed) for the instance to the backend, together. */
  async startAll(instanceId: string) {
    const batch = this.items.filter((i) => i.instanceId === instanceId && (i.state === 'staged' || i.state === 'failed'))
    if (!batch.length || this.items.some((i) => i.instanceId === instanceId && i.state === 'running')) return
    for (const i of batch) {
      i.state = 'running'
      i.error = undefined
    }
    // dependencies are planned again by the backend: only what was asked for by name goes in the list
    const named = batch.filter((i) => !i.neededBy).map((i) => i.versionId)
    try {
      await api.installVersions(instanceId, named.length ? named : batch.map((i) => i.versionId))
      for (const i of batch) i.state = 'done'
    } catch (e) {
      const message = typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
      for (const i of batch) {
        i.state = 'failed'
        i.error = message
      }
    }
  }
}

export const queue = new DownloadQueue()
