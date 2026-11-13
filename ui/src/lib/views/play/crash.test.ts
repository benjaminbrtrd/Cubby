// Crash diagnosis on real game logs. Run with `npm test` (built by Vite, run by node:test).
/// <reference types="node" />
import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { ModFile } from '../../types'
import { diagnose } from './crash'

function mod(fileName: string, name: string, id: string, version: string): ModFile {
  return { fileName, name, enabled: true, size: 1, modified: null, version, description: null, authors: [], loader: 'fabric', id } as unknown as ModFile
}

const MODS = [
  mod('sodium-fabric-0.9.3-beta.4+mc26.3.jar', 'Sodium', 'sodium', '0.9.3-beta.4+mc26.3'),
  mod('iris-fabric-1.11.7+mc26.3.jar', 'Iris', 'iris', '1.11.7+mc26.3'),
  mod('fabric-api-0.140.0.jar', 'Fabric API', 'fabric-api', '0.140.0'),
]

// Fabric Loader refusing to start after Sodium was updated past what Iris supports.
const SODIUM_IRIS = [
  '[12:01:02] [main/INFO]: Loading Minecraft 26.3 with Fabric Loader 0.18.4',
  '[12:01:02] [main/ERROR]: Incompatible mods found!',
  'net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!',
  'A potential solution has been determined, this may resolve your problem:',
  '\t - Replace mod \'Iris\' (iris) 1.11.7+mc26.3 with any version that is compatible with:',
  '\t   - sodium 0.9.3-beta.4+mc26.3',
  'More details:',
  '\t - Mod \'Sodium\' (sodium) 0.9.3-beta.4+mc26.3 is incompatible with version 1.11.7 or earlier of mod \'Iris\' (iris), yet a conflicting version is present: 1.11.7+mc26.3!',
  '\tat net.fabricmc.loader.impl.FormattedException.ofLocalized(FormattedException.java:51)',
  '\tat net.fabricmc.loader.impl.FabricLoaderImpl.load(FabricLoaderImpl.java:200)',
  '\tat net.fabricmc.loader.impl.launch.knot.Knot.init(Knot.java:146)',
  '\tat net.fabricmc.loader.impl.launch.knot.Knot.launch(Knot.java:68)',
  '\tat net.fabricmc.loader.impl.launch.knot.KnotClient.main(KnotClient.java:23)',
]

test('Fabric: a mod declared incompatible with another is a conflict naming both', () => {
  const d = diagnose(SODIUM_IRIS, MODS, 1)
  assert.equal(d.kind, 'conflict')
  assert.equal(d.culprit?.mod?.name, 'Sodium')
  assert.equal(d.other?.mod?.name, 'Iris')
  assert.deepEqual(d.declared, { range: '1.11.7 or earlier', present: '1.11.7+mc26.3' })
  assert.ok(d.evidence.some((e) => e.culprit && e.text.includes('is incompatible with')))
})

test('Fabric: the same error inside one multi-line log entry', () => {
  // The loader's error list sometimes arrives as one entry: its lines are still read one by one.
  const d = diagnose(['[12:01:02] [main/INFO]: Loading Minecraft 26.3', SODIUM_IRIS.slice(1).join('\n')], MODS, 1)
  assert.equal(d.kind, 'conflict')
  assert.equal(d.other?.ref, 'iris')
  assert.ok(d.checkedLines >= SODIUM_IRIS.length)
})

test('Fabric: "any version" and a mod that isn\'t installed', () => {
  const d = diagnose(
    ['Incompatible mods found!', " - Mod 'Embeddium' (embeddium) 1.0 is incompatible with any version of mod 'Sodium' (sodium), yet a conflicting version is present: 0.6.0!"],
    MODS,
    1,
  )
  assert.equal(d.kind, 'conflict')
  assert.equal(d.culprit?.mod, null)
  assert.equal(d.culprit?.name, 'Embeddium')
  assert.equal(d.declared?.range, 'any version')
})

test('Fabric: wrong and missing dependencies stay "missing"', () => {
  const wrong = diagnose(
    ['Incompatible mods found!', " - Mod 'Iris' (iris) 1.11.7 requires version 0.9.0 or later of mod 'Sodium' (sodium), but only the wrong version is present: 0.8.0!"],
    MODS,
    1,
  )
  assert.equal(wrong.kind, 'missing')
  assert.equal(wrong.missing[0].wrong, true)
  const absent = diagnose(["Incompatible mods found!", " - Mod 'Iris' (iris) 1.11.7 requires any version of mod 'Sodium' (sodium), which is missing!"], [], 1)
  assert.equal(absent.kind, 'missing')
  assert.equal(absent.missing[0].wrong, false)
})
