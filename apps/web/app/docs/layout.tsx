import { DocsNav } from "@/components/docs-nav";
import { docPages } from "@/lib/repo";

const pages = [
  { slug: "", title: "Overview" },
  ...docPages().map(({ slug, title }) => ({ slug, title })),
];

export default function DocsLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <div className="mx-auto grid max-w-[64rem] gap-10 px-[var(--frame)] py-12 md:grid-cols-[13rem_minmax(0,1fr)] md:py-16">
      <aside className="md:sticky md:top-20 md:self-start">
        <DocsNav pages={pages} />
      </aside>
      <main className="max-w-[44rem] min-w-0">{children}</main>
    </div>
  );
}
