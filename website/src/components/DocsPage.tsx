import { useMemo } from 'react'
import { Marked, Parser, type Tokens } from 'marked'
import { DocsLayout } from './DocsLayout'
import { CollapsibleSection } from './CollapsibleSection'
import { Toc } from './Toc'
import { resolveDocHref } from '../lib/docs'
import { createHeadingSlugger, extractHeadings } from '../lib/headings'

function renderMarkdown(source: string) {
  const headingSlug = createHeadingSlugger()
  const markdown = new Marked({
    renderer: {
      heading({ tokens, depth }) {
        const html = this.parser.parseInline(tokens)
        return `<h${depth} id="${headingSlug(html)}">${html}</h${depth}>\n`
      },
      link({ href, tokens }) {
        return `<a href="${resolveDocHref(href)}">${this.parser.parseInline(tokens)}</a>`
      },
    },
  })
  const tokens = markdown.lexer(source)
  // Each render owns its renderer and parser; no shared parser binding or slug state.
  const parser = new Parser(markdown.defaults)
  const result: { intro: string; sections: { id: string; titleHtml: string; html: string }[] } = {
    intro: '',
    sections: [],
  }
  for (const token of tokens) {
    if (token.type === 'heading' && token.depth === 2) {
      const titleHtml = parser.parseInline((token as Tokens.Heading).tokens)
      result.sections.push({ id: headingSlug(titleHtml), titleHtml, html: '' })
    } else {
      const html = parser.parse(Object.assign([token], { links: tokens.links }))
      const section = result.sections[result.sections.length - 1]
      if (section) section.html += html
      else result.intro += html
    }
  }
  return result
}

export function MarkdownContent({ source }: { source: string }) {
  const { intro, sections } = useMemo(() => renderMarkdown(source), [source])
  return (
    <>
      {intro ? <div dangerouslySetInnerHTML={{ __html: intro }} /> : null}
      {sections.map(({ id, titleHtml, html }) => (
        <CollapsibleSection
          key={id}
          id={id}
          title={<span dangerouslySetInnerHTML={{ __html: titleHtml }} />}
        >
          <div dangerouslySetInnerHTML={{ __html: html }} />
        </CollapsibleSection>
      ))}
    </>
  )
}

export function DocsPage({ source }: { source: string }) {
  const headings = useMemo(() => extractHeadings(source), [source])
  return (
    <DocsLayout aside={headings.length ? <Toc items={headings} /> : undefined}>
      <article className="prose">
        <MarkdownContent source={source} />
      </article>
    </DocsLayout>
  )
}
