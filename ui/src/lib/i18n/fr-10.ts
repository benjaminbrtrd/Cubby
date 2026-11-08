// French: automatic memory, world datapacks, undoing updates, exports, imports from other launchers,
// shared game options, desktop shortcuts, world maps and Discord. See fr.ts.
import type { Entry } from '../i18n.svelte'

export const FR_10: Record<string, Entry> = {
  // Automatic memory
  'Set memory automatically': 'Régler la mémoire automatiquement',
  'Each instance gets what its mods need: 2 GB for Vanilla, up to 8 GB for big modpacks, never more than half of your {total}.':
    'Chaque instance reçoit ce dont ses mods ont besoin : 2 Go pour Vanilla, jusqu’à 8 Go pour les gros modpacks, jamais plus de la moitié de vos {total}.',
  memory: 'mémoire',
  'Every instance that doesn’t set its own gets the memory below.': 'Les instances qui n’ont pas leur propre réglage reçoivent la mémoire ci-dessous.',
  '{n}, set from its mods': '{n}, selon ses mods',
  ' (automatic)': ' (automatique)',
}
