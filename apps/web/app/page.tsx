import Link from "next/link";

import { Inline } from "@/components/inline";
import { InstallLine } from "@/components/install-line";
import { PickerDemo } from "@/components/picker-demo";
import { hosts } from "@/lib/demo";
import {
  assertFixtureHosts,
  docPages,
  license,
  readme,
  version,
} from "@/lib/repo";

const Section = ({
  title,
  lead,
  children,
}: {
  title: string;
  lead?: string;
  children?: React.ReactNode;
}) => (
  <section className="border-t border-hair py-14" data-reveal>
    <div className="grid gap-x-10 gap-y-4 md:grid-cols-[18rem_minmax(0,1fr)]">
      <h2 className="self-start text-head">{title}</h2>
      {lead === undefined ? null : (
        <p className="max-w-[40rem] text-lead text-muted">
          <Inline text={lead} />
        </p>
      )}
    </div>
    {children}
  </section>
);

export default function Home() {
  assertFixtureHosts(Object.keys(hosts));
  const { intro, platforms, install, commands, picker, agents } = readme();
  const pages = docPages();

  return (
    <main className="mx-auto max-w-[64rem] px-[var(--frame)]">
      <section className="pt-16 pb-12 text-center sm:pt-24">
        <h1 className="text-hero">keyway</h1>
        <p className="mx-auto mt-7 max-w-[38rem] text-lead text-balance">
          <Inline text={intro} />
        </p>
        <div className="mx-auto mt-8 max-w-[50rem] text-left">
          <InstallLine command={install} />
        </div>
        <p className="mt-4 text-[13px] text-muted tabular-nums">
          v{version} · {platforms} · {license()}
        </p>
      </section>

      <section className="pb-16" data-reveal>
        <PickerDemo />
      </section>

      <Section lead={picker} title="Pick a host" />

      <Section title="Every command">
        <dl className="mt-8 border-t border-hair">
          {commands.map(({ command, note }) => (
            <div
              className="grid gap-x-10 gap-y-1 border-b border-hair py-3.5 md:grid-cols-[18rem_minmax(0,1fr)]"
              key={command}
            >
              <dt>
                <span className="text-muted select-none">$ </span>
                <code>{command}</code>
              </dt>
              <dd className="text-muted">{note}</dd>
            </div>
          ))}
        </dl>
      </Section>

      <Section lead={agents} title="For coding agents" />

      <Section title="Documentation">
        <ul className="mt-8 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {pages.map((page) => (
            <li key={page.slug}>
              <Link
                className="flex h-full flex-col gap-2 rounded-[12px] border border-hair p-5 transition-colors hover:bg-card"
                href={`/docs/${page.slug}/`}
              >
                <span className="font-medium tracking-[-0.02em]">
                  {page.title}
                </span>
                <span className="text-[13px] leading-relaxed text-muted">
                  <Inline text={page.summary} />
                </span>
              </Link>
            </li>
          ))}
        </ul>
      </Section>
    </main>
  );
}
