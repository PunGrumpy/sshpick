import type { Metadata } from "next";

import { renderMarkdown } from "@/lib/markdown";
import { blobUrl, docPages, docSource } from "@/lib/repo";

interface Props {
  params: Promise<{ slug: string }>;
}

export const dynamicParams = false;

export const generateStaticParams = () =>
  docPages().map(({ slug }) => ({ slug }));

export const generateMetadata = async ({ params }: Props) => {
  const { slug } = await params;
  const page = docPages().find((candidate) => candidate.slug === slug);

  return { title: page?.title, description: page?.summary } satisfies Metadata;
};

export default async function DocPage({ params }: Props) {
  const { slug } = await params;
  const { html } = renderMarkdown(docSource(slug));

  return (
    <>
      <article
        className="prose-docs"
        dangerouslySetInnerHTML={{ __html: html }}
      />
      <p className="mt-14 border-t border-border pt-6 text-[13px] text-muted">
        <a
          href={blobUrl(`docs/${slug}.md`)}
          rel="noreferrer"
          className="hover:text-fg"
        >
          Edit docs/{slug}.md on GitHub
        </a>
      </p>
    </>
  );
}
