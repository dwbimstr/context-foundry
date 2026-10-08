// Checks every relative Markdown link and heading anchor in README.md, docs/
// and specs/ (code spans and fenced blocks excluded). Run from the repository
// root before each commit: `node scripts/check-links.mjs`; exit 1 on any error.
import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const files = ['README.md'];
for (const base of ['docs', 'specs']) {
  const walk = (dir) => {
    for (const entry of fs.readdirSync(path.join(root, dir), { withFileTypes: true })) {
      const name = path.posix.join(dir, entry.name);
      if (entry.isDirectory()) walk(name);
      else if (entry.isFile() && name.endsWith('.md')) files.push(name);
    }
  };
  walk(base);
}
const prose = (text) => {
  let fence = null;
  return text.split('\n').map((line) => {
    const m = line.match(/^\s*(`{3,}|~{3,})/);
    if (m) { if (!fence) fence = m[1][0]; else if (m[1][0] === fence) fence = null; return ''; }
    return fence ? '' : line;
  });
};
const cache = new Map();
const anchors = (file) => {
  if (cache.has(file)) return cache.get(file);
  const set = new Set(); const seen = new Map();
  for (const line of prose(fs.readFileSync(file, 'utf8'))) {
    const h = line.match(/^ {0,3}#{1,6}\s+(.+?)\s*#*\s*$/);
    if (!h) continue;
    const base = h[1].replace(/<[^>]*>/g, '').replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
      .toLowerCase().replace(/[^\p{L}\p{N}\p{M}\s_-]/gu, '').replace(/\s/g, '-');
    const n = seen.get(base) ?? 0; seen.set(base, n + 1);
    set.add(n ? `${base}-${n}` : base);
  }
  cache.set(file, set); return set;
};
const errors = []; let links = 0, headings = 0;
for (const file of files) {
  prose(fs.readFileSync(path.join(root, file), 'utf8')).forEach((raw, i) => {
    const line = raw.replace(/(`+)[\s\S]*?\1/g, '');
    for (const m of line.matchAll(/\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g)) {
      const url = m[1];
      if (/^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(url)) continue;
      const [p, frag] = url.split('#');
      const target = p ? path.resolve(path.dirname(path.join(root, file)), decodeURIComponent(p)) : path.join(root, file);
      links++;
      if (!fs.existsSync(target)) { errors.push(`${file}:${i + 1}: missing ${url}`); continue; }
      if (frag && target.endsWith('.md')) { headings++; if (!anchors(target).has(decodeURIComponent(frag))) errors.push(`${file}:${i + 1}: missing heading ${url}`); }
    }
  });
}
console.log(JSON.stringify({ files: files.length, links, headings, errors }, null, 2));
process.exitCode = errors.length ? 1 : 0;
