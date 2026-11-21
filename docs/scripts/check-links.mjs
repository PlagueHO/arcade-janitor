import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import MarkdownIt from 'markdown-it'

const markdown = new MarkdownIt()
const root = path.resolve(fileURLToPath(new URL('../..', import.meta.url)))

export function checkLinks(directory, files) {
  const errors = []
  for (const file of files) {
    const tokens = markdown.parse(readFileSync(path.join(directory, file), 'utf8'), {})
    for (const token of tokens) {
      for (const child of token.children ?? []) {
        const url = child.type === 'image' ? child.attrGet('src') : child.attrGet('href')
        if (!url || /^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(url)) continue
        const [pathname, fragment] = url.split('#')
        const decoded = decodeURIComponent(pathname.split('?')[0])
        const source = path.join(directory, file)
        const target = !decoded ? source : decoded.startsWith('/')
          ? path.join(directory, 'docs', decoded)
          : path.resolve(path.dirname(source), decoded)
        const candidates = [target, ...(decoded.endsWith('/') ? [] : [`${target}.md`]), path.join(target, 'index.md')]
        if (file.startsWith('docs/') && decoded.startsWith('/')) {
          candidates.push(path.join(directory, 'docs', 'public', decoded))
        }
        const resolved = candidates.find(candidate => existsSync(candidate) && statSync(candidate).isFile())
          ?? (!file.startsWith('docs/') && !fragment && existsSync(target) && statSync(target).isDirectory()
            ? target : undefined)
        if (!resolved) {
          errors.push(`${file}: missing local link ${url}`)
          continue
        }
        if (fragment && resolved.endsWith('.md') && statSync(resolved).isFile()) {
          const headings = markdown.parse(readFileSync(resolved, 'utf8'), {})
          const anchors = new Set()
          const counts = new Map()
          for (let index = 0; index < headings.length; index++) {
            if (headings[index].type !== 'heading_open') continue
            const text = headings[index + 1].children
              .filter(item => ['text', 'code_inline'].includes(item.type))
              .map(item => item.content).join('')
            const explicit = text.match(/\{#([^}]+)\}$/)
            const slug = text.toLowerCase().replace(/[^\p{L}\p{N}_\-\s]/gu, '').replace(/\s/g, '-')
            const count = counts.get(slug) ?? 0
            counts.set(slug, count + 1)
            anchors.add(explicit ? explicit[1] : `${slug}${count ? `-${count}` : ''}`)
          }
          if (!anchors.has(decodeURIComponent(fragment))) {
            errors.push(`${file}: missing local heading ${url}`)
          }
        }
      }
    }
  }
  return errors
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const files = execFileSync('git', ['ls-files', '*.md'], { cwd: root, encoding: 'utf8' })
    .trim().split(/\r?\n/).filter(file => !file.includes('/') || file.startsWith('docs/'))
  const errors = checkLinks(root, files)
  if (errors.length) {
    console.error(errors.join('\n'))
    process.exitCode = 1
  } else {
    console.log('Local documentation links passed.')
  }
}
