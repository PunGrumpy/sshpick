"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

export const DocsNav = ({
  pages,
}: {
  pages: { slug: string; title: string }[];
}) => {
  const pathname = usePathname();

  return (
    <nav aria-label="Documentation">
      <p className="mb-3 text-[13px] text-muted">Documentation</p>
      <ul className="-ml-3 flex flex-wrap gap-x-1 gap-y-0.5 text-[13px] md:flex-col">
        {pages.map(({ slug, title }) => {
          const href = slug ? `/docs/${slug}/` : "/docs/";
          const current = pathname === href || pathname === href.slice(0, -1);

          return (
            <li key={slug || "overview"}>
              <Link
                aria-current={current ? "page" : undefined}
                className={`block rounded-full px-3 py-1.5 transition-colors ${
                  current ? "bg-card text-fg" : "text-muted hover:text-fg"
                }`}
                href={href}
              >
                {title}
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  );
};
