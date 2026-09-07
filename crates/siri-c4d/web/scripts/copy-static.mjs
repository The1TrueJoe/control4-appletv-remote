// Copy the non-TypeScript assets into dist/ so the whole UI lives in one
// directory for rust-embed to pack.
import { cp, mkdir, rm } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const web = resolve(here, '..');
const dist = resolve(web, 'dist');

await mkdir(dist, { recursive: true });
await rm(resolve(dist, 'css'), { recursive: true, force: true });
await cp(resolve(web, 'index.html'), resolve(dist, 'index.html'));
await cp(resolve(web, 'css'), resolve(dist, 'css'), { recursive: true });

console.log('copied index.html and css/ into dist/');
