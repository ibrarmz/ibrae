// Writes the in-app updater's latest.json for a release.
// Usage: node latest-json.mjs <version> <dir with nsis.sig / msi.sig / appimage.sig>
// Each platform is listed only if its signature is there. The files are
// uploaded under the names the release workflow gives them.
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const [version, dir] = process.argv.slice(2);
const repo = process.env.GITHUB_REPOSITORY || 'ibrarmz/ibrae';
const base = `https://github.com/${repo}/releases/download/v${version}`;

const files = {
  'windows-x86_64-nsis': ['nsis.sig', `IBRAE_${version}_x64-setup.exe`],
  'windows-x86_64-msi': ['msi.sig', `IBRAE_${version}_x64.msi`],
  // Only AppImages update in place; .deb / .rpm copies just open the release page.
  'linux-x86_64-appimage': ['appimage.sig', `ibrae_${version}_amd64.AppImage`],
};
const platforms = {};
for (const [key, [sig, asset]] of Object.entries(files)) {
  const f = join(dir, sig);
  if (existsSync(f)) platforms[key] = { signature: readFileSync(f, 'utf8').trim(), url: `${base}/${asset}` };
}
if (platforms['linux-x86_64-appimage']) platforms['linux-x86_64'] = platforms['linux-x86_64-appimage'];

console.log(JSON.stringify({ version, notes: `IBRÆ ${version}`, pub_date: new Date().toISOString(), platforms }, null, 2));
