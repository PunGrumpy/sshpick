"use client";

import Lenis from "lenis";
import { useEffect } from "react";

/**
 * Smooth scrolling, and reveals that only fade under prefers-reduced-motion. Content is visible
 * until this runs, so a page without script shows everything at once.
 */
export const Motion = () => {
  useEffect(() => {
    const reduced = window.matchMedia(
      "(prefers-reduced-motion: reduce)"
    ).matches;

    const lenis = reduced ? null : new Lenis({ autoRaf: true, anchors: true });
    const root = document.documentElement;
    root.classList.add("reveal-ready");

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) {
            continue;
          }

          entry.target.classList.add("revealed");
          observer.unobserve(entry.target);
        }
      },
      { rootMargin: "0px 0px -10% 0px" }
    );

    for (const element of document.querySelectorAll("[data-reveal]")) {
      observer.observe(element);
    }

    return () => {
      observer.disconnect();
      lenis?.destroy();
      root.classList.remove("reveal-ready");
    };
  }, []);

  return null;
};
