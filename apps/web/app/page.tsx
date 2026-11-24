import Link from "next/link";

import { Inline } from "@/components/inline";
import { InstallLine } from "@/components/install-line";
import { TerminalDemo } from "@/components/terminal-demo";
import { hosts } from "@/lib/demo";
import {
  assertFixtureHosts,
  docPages,
  license,
  readme,
  version,
} from "@/lib/repo";

export default function Home() {
  assertFixtureHosts(Object.keys(hosts));
  const { intro, platforms, install, commands, picker, agents } = readme();
  const pages = docPages();

  return (
    <main className="mx-auto max-w-[56rem] px-4 sm:px-6">
      <section className="pt-16 pb-12 sm:pt-24">
        <h1 className="font-serif text-display tracking-[-0.02em]">sshpick</h1>
        <p className="mt-8 max-w-[44rem] text-[15px] leading-relaxed sm:text-[17px]">
          <Inline text={intro} />
        </p>
        <div className="mt-8 max-w-[44rem]">
          <InstallLine command={install} />
        </div>
        <p className="mt-4 text-[13px] text-muted tabular-nums">
          v{version} · {platforms} · {license()}
        </p>
      </section>

      <section data-reveal className="pb-20">
        <TerminalDemo />
      </section>

      <section data-reveal className="border-t border-border py-14">
        <h2 className="font-serif text-headline">Pick a host.</h2>
        <p className="mt-5 max-w-[44rem] leading-relaxed text-muted">
          <Inline text={picker} />
        </p>
        <dl className="mt-8 grid gap-x-8 gap-y-3 text-[13px] sm:grid-cols-[auto_1fr]">
          {commands.map(({ command, note }) => (
            <div key={command} className="contents">
              <dt>
                <code>{command}</code>
              </dt>
              <dd className="text-muted">{note}</dd>
            </div>
          ))}
        </dl>
      </section>

      <section data-reveal className="border-t border-border py-14">
        <h2 className="font-serif text-headline">For coding agents.</h2>
        <p className="mt-5 max-w-[44rem] leading-relaxed text-muted">
          <Inline text={agents} />
        </p>
      </section>

      <section className="border-t border-border py-14">
        <h2 data-reveal className="font-serif text-headline">
          Documentation.
        </h2>
        <ul className="mt-8">
          {pages.map((page) => (
            <li
              key={page.slug}
              data-reveal
              className="border-t border-border first:border-t-0"
            >
              <Link
                href={`/docs/${page.slug}/`}
                className="group grid gap-2 py-6 sm:grid-cols-[14rem_1fr] sm:gap-8"
              >
                <span className="font-serif text-title decoration-1 underline-offset-4 group-hover:underline">
                  {page.title}
                </span>
                <span className="leading-relaxed text-muted">
                  {page.summary}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      </section>
    </main>
  );
}
