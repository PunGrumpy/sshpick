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
      <ul className="flex flex-wrap gap-x-4 gap-y-2 text-[13px] md:flex-col">
        {pages.map(({ slug, title }) => {
          const href = slug ? `/docs/${slug}/` : "/docs/";
          const current = pathname === href || pathname === href.slice(0, -1);

          return (
            <li key={slug || "overview"}>
              <Link
                href={href}
                aria-current={current ? "page" : undefined}
                className={current ? "text-fg" : "text-muted hover:text-fg"}
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
