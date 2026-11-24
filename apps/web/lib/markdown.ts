import { Marked } from "marked";
import type { Tokens } from "marked";

import { blobUrl } from "./repo";

const githubHeadingId = (text: string) =>
  text
    .toLowerCase()
    .replaceAll(/<[^>]+>/gu, "")
    .replaceAll(/[^\p{L}\p{N}\s-]/gu, "")
    .trim()
    .replaceAll(/\s/gu, "-");

/** A sibling page becomes its /docs route; anything else relative to docs/ goes to GitHub. */
export const rewriteHref = (href: string) => {
  if (/^[a-z]+:|^#|^\//iu.test(href)) {
    return href;
  }

  const [file, hash] = href.split("#");
  const anchor = hash ? `#${hash}` : "";
  const page = /^[a-z-]+(?=\.md$)/iu.exec(file)?.[0];

  if (page !== undefined) {
    return page === "README" ? `/docs/${anchor}` : `/docs/${page}/${anchor}`;
  }

  const resolved = new URL(file, "https://repo.invalid/docs/").pathname.slice(
    1
  );

  return `${blobUrl(resolved)}${anchor}`;
};

export interface Heading {
  depth: number;
  text: string;
  id: string;
}

export const renderMarkdown = (source: string) => {
  const headings: Heading[] = [];
  const counts = new Map<string, number>();

  const marked = new Marked({
    gfm: true,
    renderer: {
      heading({ tokens, depth, text }: Tokens.Heading) {
        const base = githubHeadingId(text);
        const seen = counts.get(base) ?? 0;
        counts.set(base, seen + 1);
        const id = seen ? `${base}-${seen}` : base;
        headings.push({ depth, id, text: text.replaceAll("`", "") });

        return `<h${depth} id="${id}"><a href="#${id}">${this.parser.parseInline(tokens)}</a></h${depth}>\n`;
      },
      link({ href, title, tokens }: Tokens.Link) {
        const target = rewriteHref(href);
        const external = /^https?:/u.test(target);
        const titleText = title ?? "";
        const titleAttr = titleText === "" ? "" : ` title="${titleText}"`;
        const rel = external ? ` rel="noreferrer"` : "";

        return `<a href="${target}"${titleAttr}${rel}>${this.parser.parseInline(tokens)}</a>`;
      },
    },
  });

  const html = marked.parse(source, { async: false });
  const title = headings.find((heading) => heading.depth === 1)?.text ?? "";

  return { html, title, headings };
};
