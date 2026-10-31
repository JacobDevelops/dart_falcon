import { Marked } from 'marked'

export interface Heading {
  id: string
  title: string
  level: 2 | 3
}

export function configKeyId(path: string): string {
  return `key-${path.replace(/[^a-z0-9]+/gi, '-')}`
}

export function createHeadingSlugger(): (renderedHtml: string) => string {
  const used = new Set<string>()
  return (html) => {
    const base = html
      .toLowerCase()
      .replace(/<[^>]+>|&[a-z]+;/g, '')
      .replace(/[^\w\s-]/g, '')
      .trim()
      .replace(/\s/g, '-')
    let id = base
    let suffix = 0
    while (used.has(id)) id = `${base}-${++suffix}`
    used.add(id)
    return id
  }
}

export function extractHeadings(source: string): Heading[] {
  const headings: Heading[] = []
  const slug = createHeadingSlugger()
  const parser = new Marked({
    renderer: {
      heading({ tokens, depth }) {
        const html = this.parser.parseInline(tokens)
        const id = slug(html)
        if (depth === 2 || depth === 3) {
          headings.push({
            id,
            title: html.replace(/<[^>]+>/g, '').replace(/&(?:amp|lt|gt|quot|#39);/g, (entity) => {
              const entities: Record<string, string> = { '&amp;': '&', '&lt;': '<', '&gt;': '>', '&quot;': '"', '&#39;': "'" }
              return entities[entity]
            }),
            level: depth,
          })
        }
        return ''
      },
    },
  })
  parser.parse(source, { async: false })
  return headings
}
