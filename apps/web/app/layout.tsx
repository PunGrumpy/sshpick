import { GeistMono } from "geist/font/mono";
import type { Metadata, Viewport } from "next";
import Link from "next/link";

import { Mark } from "@/components/mark";
import { Motion } from "@/components/motion";
import { blobUrl, license, readme, repoUrl, version } from "@/lib/repo";

import "./globals.css";

export const metadata: Metadata = {
  title: { default: "keyway", template: "%s · keyway" },
  description: readme().intro.replaceAll("`", ""),
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#fff" },
    { media: "(prefers-color-scheme: dark)", color: "#0a0a0a" },
  ],
};

const links = [
  { href: "/docs/", label: "Docs" },
  { href: blobUrl("crates/keyway/CHANGELOG.md"), label: "Changelog" },
  { href: repoUrl, label: "GitHub" },
];

const Nav = () => (
  <nav className="flex items-center gap-1">
    {links.map(({ href, label }) => {
      const className =
        "rounded-full px-3 py-1.5 text-[13px] text-muted transition-colors hover:text-fg";

      return href.startsWith("/") ? (
        <Link className={className} href={href} key={label}>
          {label}
        </Link>
      ) : (
        <a className={className} href={href} key={label} rel="noreferrer">
          {label}
        </a>
      );
    })}
  </nav>
);

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html className={GeistMono.variable} lang="en">
      <body className="min-h-dvh bg-bg font-mono text-[14px] text-fg antialiased">
        <Motion />
        <header className="fixed inset-x-0 top-0 z-50">
          <div className="absolute inset-0 -z-10 bg-bg/85 backdrop-blur-[12px]" />
          <div className="mx-auto flex h-14 max-w-[64rem] items-center justify-between px-[var(--frame)]">
            <Link
              className="-ml-1 flex items-center gap-2 rounded-full px-1 py-1 font-medium tracking-[-0.02em]"
              href="/"
            >
              <Mark className="size-[18px]" />
              keyway
            </Link>
            <div className="-mr-3">
              <Nav />
            </div>
          </div>
        </header>
        <div className="pt-14">{children}</div>
        <footer className="mt-24 border-t border-hair">
          <div className="mx-auto flex max-w-[64rem] flex-col gap-4 px-[var(--frame)] py-10 text-[13px] text-muted sm:flex-row sm:items-center sm:justify-between">
            <span className="tabular-nums">
              keyway v{version} · {license()}
            </span>
            <div className="-mx-3">
              <Nav />
            </div>
          </div>
        </footer>
      </body>
    </html>
  );
}
