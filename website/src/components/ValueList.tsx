export function ValueList({ values }: { values?: string[] }) {
  if (!values?.length) return null
  return (
    <span className="flag-tag">
      values:{' '}
      {values.map((value, i) => (
        <span key={value}>
          {i > 0 ? ' | ' : ''}
          <code>{value}</code>
        </span>
      ))}
    </span>
  )
}
