import { createFileRoute } from '@tanstack/react-router'
import { ruleMarkdown, textResponse } from '../lib/agent-docs'
import { resolveRule } from '../lib/rules.data'

export const Route = createFileRoute('/linter/rules/{$rule}.md')({
  server: {
    handlers: {
      GET: ({ params }) => {
        const rule = resolveRule(params.rule)
        if (rule && rule.name !== params.rule) {
          return new Response(null, { status: 301, headers: { Location: `/linter/rules/${rule.name}.md` } })
        }
        return rule
          ? textResponse(ruleMarkdown(rule), true)
          : textResponse('Rule not found\n', false, 404)
      },
    },
  },
})
