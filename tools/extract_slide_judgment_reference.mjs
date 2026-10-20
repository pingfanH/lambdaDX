import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import crypto from 'node:crypto';

const root = fileURLToPath(new URL('../assets/extracted/lambdadx_replay_1600_master/', import.meta.url));
const bundlePath = process.argv[2];
if (!bundlePath) throw new Error('usage: node tools/extract_slide_judgment_reference.mjs SOURCE_BUNDLE [--check]');
const source = fs.readFileSync(bundlePath, 'utf8');
if (!source.includes('var Ze=') || !source.includes('#z(e,n,r){')) {
  throw new Error('source bundle layout changed; review the geometry/result extraction');
}
const library = new Function(source.slice(0, source.indexOf('var Ze=')) + ';return {fe,Pe,Ve,Je,Be,I,t};')();
const paths = new Map();
for (const name of library.fe) {
  const file = path.join(root, 'slide_paths', name);
  if (!fs.existsSync(file)) {
    if (process.argv.includes('--check')) throw new Error('missing ' + file);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    const bytes = execFileSync('curl', ['-fsSL', '--retry', '5', '--retry-all-errors', '--connect-timeout', '15', '--max-time', '60', 'https://maiwiki.moe/gameresources/slide/' + name]);
    fs.writeFileSync(file, bytes);
  }
  paths.set(name, fs.readFileSync(file, 'utf8'));
}
const engine = library.Pe(paths);
const start = source.indexOf('#z(e,n,r){');
const end = source.indexOf('}};function rn', start);
if (end < start) throw new Error('source result method terminator changed');
const method = source.slice(start + '#z(e,n,r){'.length, end)
  .replaceAll('this.#r', 'settings').replaceAll('this.#t', 'sprites');
const render = new Function('t', 'Be', 'settings', 'sprites', 'e', 'n', 'r', method);
const quadSignature = '#i(e,t,n,r,i,a,o,s,c,l,u,d,f,p,m,h,g){';
const quadStart = source.indexOf(quadSignature);
const quadEnd = source.indexOf('}draw(', quadStart);
if (quadStart < 0 || quadEnd < quadStart) throw new Error('source sprite transform changed');
const drawQuad = new Function('Ft', 'Y', 'return function(e,t,n,r,i,a,o,s,c,l,u,d,f,p,m,h,g){' + source.slice(quadStart + quadSignature.length, quadEnd) + '}')(new Float64Array(8), new Float64Array(8));
const manifest = JSON.parse(fs.readFileSync(path.join(root, 'judgement/pad_sprite_manifest.json'), 'utf8'));
const poses = {};
for (let type = 1; type <= 13; type++) {
  for (let start = 0; start < 8; start++) {
    for (let end = 0; end < 8; end++) {
      if (!engine.checkSlideEnable(type, start, end)) continue;
      const item = type === library.I.Fan
        ? { kind: 'fan', fan: library.Je(engine, start, end) }
        : { kind: 'slide', chain: library.Ve(engine, [{ type, start, end }]) };
      const calls = [];
      render(library.t, library.Be, { judgePos: 5 }, { draw(...args) { calls.push(args); } }, null, item, 100);
      const [, key, x, y, { rot }] = calls[0];
      if (type !== library.I.Fan) {
        const chain = library.Ve(engine, [{type:1,start:(start+4)%8,end:start}, {type,start,end}]);
        const chained = [];
        render(library.t, library.Be, {judgePos:5}, {draw(...args) { chained.push(args); }}, null, {kind:'slide',chain}, 100);
        if (JSON.stringify(chained) !== JSON.stringify(calls)) throw new Error('chain final pose differs');
      }
      const meta = manifest[key];
      const px = meta.rect[0] + meta.pivot[0] * meta.rect[2];
      const py = meta.tex[1] - meta.rect[1] - meta.pivot[1] * meta.rect[3];
      let corners;
      drawQuad({quad(...args) { corners = Array.from(args[2]); }}, {tex:0}, x,y,rot,1,1,-px,-py,meta.tex[0]-px,meta.tex[1]-py,[0,0,1,1],1,1,false,null,null);
      poses[[type, start + 1, end + 1].join('/')] = { key: key.replace('_Critical', ''), x, y, rotation: rot, corners };
    }
  }
}
const animation = [];
const sample = { kind: 'slide', isBreak: true, chain: library.Ve(engine, [{type:1,start:0,end:4}]) };
for (const ms of [0, 16.666, 33.333, 41.665, 66.666, 83.33, 100, 150, 249.999, 250, 291.665, 333.33]) {
  const calls = [];
  render(library.t, library.Be, { judgePos: 5 }, { draw(...args) { calls.push(args); } }, null, sample, ms);
  animation.push({ms, alpha:calls[0][4].alpha, additive:calls.length > 1});
}
const data = { source:'https://maiwiki.moe/_app/immutable/chunks/ihKeLN9J.js', sha256:crypto.createHash('sha256').update(source).digest('hex'), poses, animation };
const destination = path.join(root, 'slide_judgment_reference.json');
if (process.argv.includes('--check')) {
  if (JSON.stringify(JSON.parse(fs.readFileSync(destination, 'utf8'))) !== JSON.stringify(data)) throw new Error('slide reference differs from source renderer');
} else fs.writeFileSync(destination, JSON.stringify(data, null, 2) + '\n');
console.log(Object.keys(poses).length + ' slide poses and ' + animation.length + ' animation samples matched the source renderer');
