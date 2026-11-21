import { strict as assert } from 'node:assert'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { checkLinks } from './check-links.mjs'

test('local links check targets, references, images, and heading fragments', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'arcadejanitor-links-'))
  try {
    mkdirSync(path.join(root, 'docs'))
    writeFileSync(path.join(root, 'docs', 'target.md'), '# Target\n\n## A heading\n')
    writeFileSync(path.join(root, 'docs', 'index.md'),
      '# Index\n\n[Valid](./target.md#a-heading)\n\n[Missing][target]\n\n' +
      '[target]: ./absent.md\n\n![Missing image](absent.png)\n\n' +
      '[Missing heading](./target.md#absent)\n\n[External](https://example.com/)\n')
    assert.deepEqual(checkLinks(root, ['docs/index.md']), [
      'docs/index.md: missing local link ./absent.md',
      'docs/index.md: missing local link absent.png',
      'docs/index.md: missing local heading ./target.md#absent',
    ])
  } finally {
    rmSync(root, { recursive: true })
  }
})

test('the pinned style checker rejects malformed Markdown', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'arcadejanitor-markdown-'))
  try {
    const file = path.join(root, 'invalid.md')
    writeFileSync(file, '# Title\n\n### Skipped heading level\n')
    const checker = fileURLToPath(new URL('../node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs', import.meta.url))
    const result = spawnSync(process.execPath, [checker, file], { cwd: root, encoding: 'utf8' })
    assert.equal(result.status, 1)
    assert.match(result.stdout + result.stderr, /MD001/)
  } finally {
    rmSync(root, { recursive: true })
  }
})

test('site directory links require index pages and valid fragments', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'arcadejanitor-index-links-'))
  try {
    mkdirSync(path.join(root, 'docs', 'page'), { recursive: true })
    mkdirSync(path.join(root, 'docs', 'empty'))
    mkdirSync(path.join(root, 'tests', 'fixtures'), { recursive: true })
    writeFileSync(path.join(root, 'docs', 'page', 'index.md'), '# Page\n')
    writeFileSync(path.join(root, 'docs', 'page.md'), '# Sibling\n\n## Missing\n')
    writeFileSync(path.join(root, 'README.md'), '# Project\n\n[Fixtures](tests/fixtures/)\n')
    writeFileSync(path.join(root, 'docs', 'index.md'),
      '# Index\n\n[Page](./page/)\n\n[Heading](./page/#page)\n\n[Sibling](./page#missing)\n\n' +
      '[Missing heading](./page/#missing)\n\n[Missing page](./empty/)\n\n' +
      '[Current heading](./#index)\n\n[Missing current heading](./#missing)\n')
    assert.deepEqual(checkLinks(root, ['README.md', 'docs/index.md']), [
      'docs/index.md: missing local heading ./page/#missing',
      'docs/index.md: missing local link ./empty/',
      'docs/index.md: missing local heading ./#missing',
    ])
  } finally {
    rmSync(root, { recursive: true })
  }
})
