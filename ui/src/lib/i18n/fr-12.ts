// French: the contents of a modpack version (its Mods tab or section) on the pack pages of Discover. See fr.ts.
import type { Entry } from '../i18n.svelte'

export const FR_12: Record<string, Entry> = {
  'This pack has no version to look into yet.': 'Ce pack n’a encore aucune version à consulter.',
  "Couldn't load the contents of this version. {error}": 'Impossible de charger le contenu de cette version. {error}',
  "Reading the pack's list of files…": 'Lecture de la liste des fichiers du pack…',
  'Loading the versions…': 'Chargement des versions…',
  "This pack is a single download that doesn't list its mods. You'll see them in the instance once it's installed.":
    'Ce pack est un seul téléchargement qui ne liste pas ses mods. Vous les verrez dans l’instance une fois installée.',
  "This version doesn't list any mods.": 'Cette version ne liste aucun mod.',
  'Search the contents': 'Rechercher dans le contenu',
  'Search {n}': 'Rechercher parmi {n}',
  'Nothing in this version matches “{query}”.': 'Rien dans cette version ne correspond à « {query} ».',
  'Clear the search': 'Effacer la recherche',
  'Open {name} in Cubby': 'Ouvrir {name} dans Cubby',
  'Open the page of {name} in your browser': 'Ouvrir la page de {name} dans votre navigateur',
  'From Modrinth': 'Depuis Modrinth',
  'From CurseForge': 'Depuis CurseForge',
  'Web page': 'Page web',
  'CurseForge project': 'Projet CurseForge',
  Contents: 'Contenu',
  "The versions didn't load, so neither did the content.": 'Les versions n’ont pas pu être chargées, le contenu non plus.',
  // counted words (tn)
  item: ['élément', 'éléments'],
  optional: ['facultatif', 'facultatifs'],
  'resource pack': ['pack de ressources', 'packs de ressources'],
  'shader pack': ['shader pack', 'shader packs'],
}
