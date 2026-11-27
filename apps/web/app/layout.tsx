import { GeistMono } from "geist/font/mono";
import type { Metadata, Viewport } from "next";
import { Instrument_Serif } from "next/font/google";
import Link from "next/link";

import { Motion } from "@/components/motion";
import { blobUrl, readme, repoUrl } from "@/lib/repo";

import "./globals.css";

const serif = Instrument_Serif({
  weight: "400",
  subsets: ["latin"],
  variable: "--font-instrument-serif",
});

export const metadata: Metadata = {
  title: { default: "keyway", template: "%s · keyway" },
  description: readme().intro.replaceAll("`", ""),
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#fff" },
    { media: "(prefers-color-scheme: dark)", color: "#000" },
  ],
};

const links = [
  { href: "/docs/", label: "docs" },
  { href: blobUrl("crates/keyway/CHANGELOG.md"), label: "changelog" },
  { href: repoUrl, label: "source" },
];

const Nav = ({ className }: { className?: string }) => (
  <nav className={`flex gap-5 text-muted ${className ?? ""}`}>
    {links.map(({ href, label }) =>
      href.startsWith("/") ? (
        <Link key={label} href={href} className="hover:text-fg">
          {label}
        </Link>
      ) : (
        <a key={label} href={href} rel="noreferrer" className="hover:text-fg">
          {label}
        </a>
      )
    )}
  </nav>
);

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className={`${GeistMono.variable} ${serif.variable}`}>
      <body className="min-h-dvh bg-bg font-mono text-[14px] text-fg antialiased">
        <Motion />
        <header className="sticky top-0 z-10 border-b border-border bg-bg/85 backdrop-blur">
          <div className="mx-auto flex h-14 max-w-[56rem] items-center justify-between px-4 sm:px-6">
            <Link href="/" className="font-medium">
              keyway
            </Link>
            <Nav className="text-[13px]" />
          </div>
        </header>
        {children}
        <footer className="border-t border-border">
          <div className="mx-auto flex max-w-[56rem] flex-col gap-3 px-4 py-10 text-[13px] text-muted sm:flex-row sm:items-center sm:justify-between sm:px-6">
            <span>keyway</span>
            <Nav />
          </div>
        </footer>
      </body>
    </html>
  );
}
