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

  // Reviewing and undoing updates
  'Undo last update': 'Annuler la dernière mise à jour',
  'Update {n}': 'Mettre à jour {n}',
  file: ['fichier', 'fichiers'],
  'What’s new': 'Nouveautés',
  'Cubby keeps the old files, so you can undo the update if something breaks.': 'Cubby garde les anciens fichiers : vous pourrez annuler la mise à jour si quelque chose casse.',
  'No changelog for this version.': 'Pas de notes de version pour cette version.',
  'Undo update': 'Annuler la mise à jour',
  'Undo the update from {when}?': 'Annuler la mise à jour du {when} ?',
  'These files go back to the version you had before. The newer files move to the instance’s trash folder.':
    'Ces fichiers reviennent à la version d’avant. Les plus récents vont dans la corbeille de l’instance.',
  'Put back {n} from before the update.': '{n} remis comme avant la mise à jour.',

  // Export
  'Export as a modpack file': 'Exporter en fichier modpack',
  'Export {name}': 'Exporter {name}',
  Export: 'Exporter',
  'Show in folder': 'Afficher dans le dossier',
  'Saved to': 'Enregistré dans',
  '{listed} downloaded by the launcher that opens it, {stored} stored in the file.': '{listed} téléchargés par le launcher qui l’ouvre, {stored} stockés dans le fichier.',
  'Without a CurseForge API key, Cubby can’t tell which mods CurseForge has, so they are all stored in the file. It works, but the file is bigger.':
    'Sans clé d’API CurseForge, Cubby ne sait pas quels mods CurseForge propose : ils sont tous stockés dans le fichier. Ça marche, mais le fichier est plus lourd.',
  Format: 'Format',
  'Opens in Cubby, the Modrinth App, Prism Launcher and most launchers. Mods from Modrinth are downloaded from there, the others are stored in the file.':
    'S’ouvre dans Cubby, la Modrinth App, Prism Launcher et la plupart des launchers. Les mods de Modrinth sont téléchargés depuis Modrinth, les autres sont stockés dans le fichier.',
  'Opens in the CurseForge app, Cubby and Prism Launcher. Mods CurseForge has are downloaded from there, the others are stored in the file.':
    'S’ouvre dans l’app CurseForge, Cubby et Prism Launcher. Les mods présents sur CurseForge en sont téléchargés, les autres sont stockés dans le fichier.',
  'No CurseForge API key': 'Pas de clé d’API CurseForge',
  'Every mod will be stored in the file.': 'Tous les mods seront stockés dans le fichier.',
  'Pack name': 'Nom du pack',
  Version: 'Version',
  Summary: 'Résumé',
  Author: 'Auteur',
  'Also include': 'Inclure aussi',
  'Mods, resource packs and shader packs are always in. Turned-off mods are left out.': 'Les mods, resource packs et shader packs sont toujours inclus. Les mods désactivés sont exclus.',
  'Reading the instance…': 'Lecture de l’instance…',
  'Nothing else in this instance yet.': 'Rien d’autre dans cette instance pour l’instant.',
  'Writing the pack': 'Écriture du pack',

  // Importing from other launchers
  'Import from another launcher': 'Importer depuis un autre launcher',
  'Copies instances, the other launcher stays as it is': 'Copie les instances, l’autre launcher reste tel quel',
  'Imported {n}.': '{n} importée(s).',
  'Looking for other launchers…': 'Recherche des autres launchers…',
  'Couldn’t read the other launchers’ instances': 'Impossible de lire les instances des autres launchers',
  'No other launcher found': 'Aucun autre launcher trouvé',
  'Cubby looks for Prism Launcher, MultiMC, PolyMC, CurseForge, ATLauncher, GDLauncher, the Modrinth App and the Minecraft Launcher in their usual folders.':
    'Cubby cherche Prism Launcher, MultiMC, PolyMC, CurseForge, ATLauncher, GDLauncher, la Modrinth App et le Minecraft Launcher dans leurs dossiers habituels.',
  '{launcher} instances': 'Instances de {launcher}',
  'Version unknown: export it as a .mrpack from {launcher}, then import the file.': 'Version inconnue : exportez-la en .mrpack depuis {launcher}, puis importez le fichier.',
  'Safe: Cubby copies the mods, worlds, screenshots and settings. The other launchers stay as they are.':
    'Sans risque : Cubby copie les mods, mondes, captures et réglages. Les autres launchers restent tels quels.',

  // Copying options
  'Game options': 'Options du jeu',
  'Copy from another instance': 'Copier depuis une autre instance',
  'Bring over your key bindings, options or server list, so a new instance feels like home.':
    'Récupérez vos touches, options ou serveurs : une nouvelle instance est tout de suite à votre goût.',
  Copy: 'Copier',
  'Copy settings into {name}': 'Copier des réglages dans {name}',
  'There is no other instance to copy from yet.': 'Il n’y a pas encore d’autre instance depuis laquelle copier.',
  From: 'Depuis',
  'Key bindings': 'Touches',
  'Your controls: jump, sprint, and the keys mods add.': 'Vos contrôles : sauter, sprinter, et les touches ajoutées par les mods.',
  'Video, sound and other options': 'Vidéo, son et autres options',
  'Render distance, FOV, volume, language... Resource packs stay as they are.': 'Distance d’affichage, FOV, volume, langue… Les resource packs ne changent pas.',
  'Server list': 'Liste des serveurs',
  'Replaces the Multiplayer list of this instance.': 'Remplace la liste Multijoueur de cette instance.',
  'Close the game first: it writes its options when it quits.': 'Fermez d’abord le jeu : il enregistre ses options en quittant.',
  options: 'options',
  'key bindings': 'touches',
  servers: 'serveurs',
  'Copied the {parts} of {name}.': '{parts} de {name} copiés.',
  '{name} has nothing to copy yet. Play it once first.': '{name} n’a encore rien à copier. Lancez-la une fois d’abord.',

  // Desktop shortcuts
  'Add to desktop': 'Ajouter au bureau',
  'A shortcut to {name} is on your desktop. Double-click it to play.': 'Un raccourci vers {name} est sur votre bureau. Double-cliquez dessus pour jouer.',
  'That shortcut’s instance isn’t in Cubby anymore.': 'L’instance de ce raccourci n’est plus dans Cubby.',

  // World map
  Map: 'Carte',
  'Show the map of {name} bigger': 'Agrandir la carte de {name}',
  'Map of {name} around its spawn': 'Carte de {name} autour du point d’apparition',
  '{n} blocks around the spawn · {chunks} chunks explored': '{n} blocs autour du point d’apparition · {chunks} chunks explorés',
  'Drawing the map': 'Dessin de la carte',
  'See the land around the spawn from above.': 'Voyez les environs du point d’apparition vus du ciel.',
  'Draw again': 'Redessiner',
  'Draw the map': 'Dessiner la carte',
  'From {x}, {z} to {x2}, {z2}. The red dot is the spawn.': 'De {x}, {z} à {x2}, {z2}. Le point rouge est le point d’apparition.',
  'This world has no map yet: play in it once, then draw it.': 'Ce monde n’a pas encore de carte : jouez-y une fois, puis dessinez-la.',

  // Discord
  'Show on Discord': 'Afficher sur Discord',
  'Your Discord profile says which instance you’re playing, while you play. Discord has to be open on this computer.':
    'Votre profil Discord indique à quelle instance vous jouez, pendant la partie. Discord doit être ouvert sur cet ordinateur.',

  // Import footer
  'Coming from Prism, CurseForge, the Modrinth App or another launcher?': 'Vous venez de Prism, CurseForge, Modrinth App ou d’un autre launcher ?',
  'Import its instances': 'Importer ses instances',
}
