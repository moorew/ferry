// Fails if UI code uses a literal colour instead of a design token (design/tokens).
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const colour = /#[0-9a-f]{3,8}\b|\b(?:rgba?|hsla?)\(/gi;
const skip = new Set(['src/lib/mock.ts']);

function* files(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* files(path);
    else if (/\.(svelte|css|ts)$/.test(name)) yield path;
  }
}

let found = 0;
for (const path of files('src')) {
  if (skip.has(path.replaceAll('\\', '/'))) continue;
  readFileSync(path, 'utf8')
    .split('\n')
    .forEach((line, i) => {
      if (line.match(colour)) {
        console.error(`${path}:${i + 1}: ${line.trim()}`);
        found++;
      }
    });
}
if (found) {
  console.error(`\n${found} literal colour(s); use a --tc-* token from design/tokens instead.`);
  process.exit(1);
}
