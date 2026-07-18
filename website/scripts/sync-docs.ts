// Copies the repo's canonical docs markdown into the site so pages can import
// them with `?raw`. Runs automatically before dev/build (see package.json).
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const websiteRoot = join(here, '..')
const repoRoot = join(websiteRoot, '..')
const outDir = join(websiteRoot, 'src', 'content')

mkdirSync(outDir, { recursive: true })

for (const name of ['installation', 'configuration']) {
  const src = join(repoRoot, 'docs', `${name}.md`)
  const md = readFileSync(src, 'utf-8')
  writeFileSync(join(outDir, `${name}.md`), md)
  console.log(`synced docs/${name}.md`)
}
