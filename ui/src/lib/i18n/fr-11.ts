// French: installing from CurseForge (mods, shader packs, resource packs and datapacks) and its filters in
// Discover. See fr.ts.
import type { Entry } from '../i18n.svelte'

export const FR_11: Record<string, Entry> = {
  // Install dialog
  '{title} is in {name}. {n} still need a download on CurseForge.': '{title} est dans {name}. {n} restent à télécharger sur CurseForge.',
  '{title} is in {name}.': '{title} est dans {name}.',
  'Create an instance first: {many} go into one.': 'Créez d’abord une instance : les {many} s’installent dedans.',
  'Couldn’t load the files': 'Impossible de charger les fichiers',
  'Loading files…': 'Chargement des fichiers…',
  '{title} has no file Cubby can install.': '{title} n’a aucun fichier que Cubby peut installer.',
  '{title} has no file for Minecraft {mc}.': '{title} n’a aucun fichier pour Minecraft {mc}.',
  'Show files for every version': 'Voir les fichiers de toutes les versions',
  'This file isn’t made for {name}: the game may not start with it.': 'Ce fichier n’est pas fait pour {name} : le jeu risque de ne pas démarrer avec.',
  'This file isn’t made for Minecraft {mc}. It may not load.': 'Ce fichier n’est pas fait pour Minecraft {mc}. Il risque de ne pas se charger.',
  'Only on CurseForge': 'Uniquement sur CurseForge',
  'Its author only lets it be downloaded on curseforge.com. Download it there, then add it to {name}.':
    'Son auteur ne permet de le télécharger que sur curseforge.com. Téléchargez-le là-bas, puis ajoutez-le à {name}.',
  'Download on CurseForge': 'Télécharger sur CurseForge',
  '{name} has another version:': '{name} a une autre version :',
  'Also needed': 'Également nécessaire',
  'Only on CurseForge: download it by hand': 'Uniquement sur CurseForge : à télécharger à la main',
  'Added with it · {size}': 'Ajouté avec · {size}',
  'No file for {name}': 'Aucun fichier pour {name}',
  'Its page': 'Sa page',
  'Nothing else needed: everything it requires is already in {name}.': 'Rien d’autre à ajouter : tout ce qu’il lui faut est déjà dans {name}.',
  ', and {n} to get from CurseForge.': ', et {n} à récupérer sur CurseForge.',
  'Close the game first.': 'Fermez d’abord le jeu.',
  'See it on CurseForge': 'Voir sur CurseForge',

  // Filters
  'Pick other versions': 'Choisir d’autres versions',
  'Show fewer versions': 'Moins de versions',
  'Show all versions ({n})': 'Toutes les versions ({n})',
  'CurseForge searches up to {n} versions at once.': 'CurseForge cherche dans {n} versions au plus à la fois.',
  'CurseForge searches up to {n} categories at once.': 'CurseForge cherche dans {n} catégories au plus à la fois.',
  'Showing {many} for {what}.': 'Affichage des {many} pour {what}.',
  'More than {n}': 'Plus de {n}',

  // Versions of installed content
  'Local file': 'Fichier local',
  'Not found on Modrinth or CurseForge': 'Introuvable sur Modrinth et CurseForge',
  'Versions of {title}': 'Versions de {title}',
  'Pick another version to go back to an older one, or to move to a newer one. Cubby replaces the file that is there.':
    'Choisissez une autre version pour revenir à une plus ancienne, ou passer à une plus récente. Cubby remplace le fichier en place.',
  'Couldn’t load {title}': 'Impossible de charger {title}',

  // Modpack versions
  '{name} now runs {pack} {version}.': '{name} utilise maintenant {pack} {version}.',
  'Switch version': 'Changer de version',
  'Installed:': 'Installée :',
  installed: 'installée',
  'Your worlds, screenshots, servers and the mods you added yourself stay.': 'Vos mondes, captures, serveurs et les mods que vous avez ajoutés vous-même restent.',
  'The pack’s mods and settings are replaced by the ones of this version. Mods it no longer has go to the instance’s trash folder.':
    'Les mods et réglages du pack sont remplacés par ceux de cette version. Les mods qu’elle n’a plus vont dans la corbeille de l’instance.',
  'This version runs Minecraft {mc}. Back up your worlds first: a world opened in a newer version may not open in an older one.':
    'Cette version utilise Minecraft {mc}. Sauvegardez d’abord vos mondes : un monde ouvert dans une version plus récente peut ne plus s’ouvrir dans une plus ancienne.',

  // Updates at start
  '{n} can be updated in {name}.': '{n} peuvent être mis à jour dans {name}.',
  '{n} can be updated in {count}.': '{n} peuvent être mis à jour dans {count}.',
  'See them': 'Voir',
  'See {name}': 'Voir {name}',
  'Check mod updates when Cubby starts': 'Vérifier les mises à jour des mods au démarrage de Cubby',
  'Looks for new versions of every instance’s mods and packs on Modrinth, at start and when you open an instance. Cubby tells you, and always asks before updating.':
    'Cherche de nouvelles versions des mods et packs de chaque instance sur Modrinth, au démarrage et à l’ouverture d’une instance. Cubby vous prévient, et demande toujours avant de mettre à jour.',
  'Source: CurseForge': 'Source : CurseForge',
  'Open on CurseForge': 'Ouvrir sur CurseForge',
  'Page on CurseForge': 'Page sur CurseForge',
}
