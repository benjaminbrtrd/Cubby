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

  // Datapacks
  'Datapacks · {length}': 'Datapacks · {length}',
  'Reading datapacks…': 'Lecture des datapacks…',
  'No datapacks in this world. They add structures, biomes or rules to one world, without mods.':
    'Aucun datapack dans ce monde. Ils ajoutent des structures, des biomes ou des règles à un monde, sans mods.',
  Folder: 'Dossier',
  'Load {name} in this world': 'Charger {name} dans ce monde',
  'Remove {name}': 'Retirer {name}',
  'Find on Modrinth': 'Chercher sur Modrinth',
  'Add from file': 'Ajouter un fichier',
  'Close the game to change datapacks. They load when the world opens.': 'Fermez le jeu pour changer les datapacks. Ils se chargent à l’ouverture du monde.',
  datapack: ['datapack', 'datapacks'],
  'Added {n} to {world}.': '{n} ajouté(s) à {world}.',
  'Datapacks for {world}': 'Datapacks pour {world}',
  'Search datapacks': 'Chercher des datapacks',
  'Search datapacks for {minecraft}': 'Chercher des datapacks pour {minecraft}',
  'Searching Modrinth…': 'Recherche sur Modrinth…',
  'No datapacks for Minecraft {minecraft} match.': 'Aucun datapack pour Minecraft {minecraft} ne correspond.',
  '{author} · {downloads} downloads': '{author} · {downloads} téléchargements',
  Added: 'Ajouté',
  Add: 'Ajouter',
  '{title} is in {world}.': '{title} est dans {world}.',
  '{name} moved to the instance’s trash folder.': '{name} a été déplacé dans la corbeille de l’instance.',
}
