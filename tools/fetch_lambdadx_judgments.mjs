import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = fileURLToPath(new URL('../assets/extracted/lambdadx_replay_1600_master/judgement/', import.meta.url));
const base = 'https://maiwiki.moe/gameresources/sprites';
const response = await fetch(base + '/manifest.json');
if (!response.ok) throw new Error('manifest: ' + response.status);
const manifest = await response.json();
const families = ['judge/', 'slidejudge/', 'slidecirclejudge/', 'slidefanjudge/'];
const entries = Object.entries(manifest).filter(([key]) => families.some(prefix => key.startsWith(prefix)));
for (const [key] of entries) {
  const destination = path.join(root, key + '.png');
  try {
    await readFile(destination);
    continue;
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  const image = await fetch(base + '/' + key + '.png');
  if (!image.ok) throw new Error(key + ': ' + image.status);
  await mkdir(path.dirname(destination), { recursive: true });
  await writeFile(destination, new Uint8Array(await image.arrayBuffer()));
}
await writeFile(path.join(root, 'pad_sprite_manifest.json'), JSON.stringify(Object.fromEntries(entries), null, 2) + '\n');
console.log('LambdaDX judgments: ' + entries.length + ' sprites');
