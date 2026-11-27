/** README prose keeps its Markdown backticks; this draws them as code, the one inline style it uses. */
export const Inline = ({ text }: { text: string }) => (
  <>
    {text.split(/`(?<code>[^`]+)`/u).map((part, i) =>
      i % 2 ? (
        <code
          className="rounded border border-hair bg-card px-1 text-[0.92em] whitespace-nowrap text-fg"
          key={i}
        >
          {part}
        </code>
      ) : (
        part
      )
    )}
  </>
);
