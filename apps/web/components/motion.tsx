"use client";

import Lenis from "lenis";
import { useEffect } from "react";

/**
 * Smooth scrolling and the scroll reveals. Content is visible until this runs, and stays visible
 * under prefers-reduced-motion, so a page without script, or for someone who asked for no motion,
 * shows everything at once.
 */
export const Motion = () => {
  useEffect(() => {
    const moves = !window.matchMedia("(prefers-reduced-motion: reduce)")
      .matches;

    const lenis = moves ? new Lenis({ autoRaf: true, anchors: true }) : null;
    const root = document.documentElement;

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

    if (moves) {
      root.classList.add("reveal-ready");

      for (const element of document.querySelectorAll("[data-reveal]")) {
        observer.observe(element);
      }
    }

    return () => {
      observer.disconnect();
      lenis?.destroy();
      root.classList.remove("reveal-ready");
    };
  }, []);

  return null;
};
