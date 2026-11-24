import type { Metadata } from "next";

import { renderMarkdown } from "@/lib/markdown";
import { docSource } from "@/lib/repo";

export const metadata: Metadata = { title: "Documentation" };

export default function DocsIndex() {
  const { html } = renderMarkdown(docSource(""));

  return (
    <article
      className="prose-docs"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
