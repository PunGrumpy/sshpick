/** README prose keeps its Markdown backticks; this draws them as code, the one inline style it uses. */
export const Inline = ({ text }: { text: string }) => (
  <>
    {text.split(/`(?<code>[^`]+)`/u).map((part, i) =>
      i % 2 ? (
        <code
          key={i}
          className="rounded border border-border bg-card px-1 text-[0.92em] text-fg"
        >
          {part}
        </code>
      ) : (
        part
      )
    )}
  </>
);
