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
    <div className="mx-auto grid max-w-[72rem] gap-10 px-4 py-10 sm:px-6 md:grid-cols-[12rem_minmax(0,1fr)] md:py-14">
      <aside className="md:sticky md:top-24 md:self-start">
        <DocsNav pages={pages} />
      </aside>
      <main className="max-w-[48rem] min-w-0">{children}</main>
    </div>
  );
}
