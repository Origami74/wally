# React Frontend Rewrite Plan

## Overview
- Goal: replace the current SvelteKit UI with a Vite + React + Tailwind + shadcn/ui stack while preserving existing backend/Tauri plugin code.
- Outcomes: wallet-centric desktop UX (see wireframe), no bottom navigation, reusable component library wired to custom design tokens, zero behavioural regressions in backend-facing flows.
- Constraints: backend code and Tauri plugins stay untouched; new CSS theme will be supplied and should be integrated after shadcn scaffolding.

## Phase 0 – Preparation & Inventory
- Audit current frontend packages, routes, shared utilities, and API bridges (`src/lib`, store usage, IPC wrappers) to document what must be ported.
- Catalogue all UI flows/screens and map them to the new React views (dashboard, receive modal, send form, feature toggles/settings panels).
- Export/backup environment files, Tailwind config fragments, and assets (icons, fonts, localization) required post-migration.

## Phase 1 – Remove Svelte Frontend
- Delete Svelte-specific source (`src`, `static`, Svelte stores/components) and configuration files (`svelte.config.js`, Svelte Vite plugins, kit adapters) after capturing any logic to be ported.
- Prune Svelte dependencies from `package.json` and lockfiles (Svelte packages, vite-plugin-svelte, SvelteKit CLI tools).
- Verify Tauri build still runs with a placeholder frontend entry (temporary `index.html`) to keep desktop builds green during the transition.

## Phase 2 – Bootstrap React + Tailwind + shadcn/ui Stack
- Initialize a fresh Vite React project inside the existing workspace (`pnpm create vite` or manual config) using TypeScript and SWC.
- Configure Tailwind CSS (postcss config, tailwind config, directory scanning paths) and integrate it with Vite and Tauri build steps.
- Install shadcn/ui CLI and base dependencies (Radix UI, class-variance-authority, tailwind-merge, Lucide icons) and scaffold required components.
- Restore essential shared tooling: ESLint + Prettier configs, testing libraries (Vitest/Testing Library) aligned with React.
- Rewire Tauri IPC helpers (replace Svelte `$app` imports with React-friendly wrappers) and confirm the dev server hooks work with Tauri.

## Phase 3 – Apply Custom Styling
- Import the provided CSS theme (tokens, global styles) and merge it with Tailwind configuration (custom colors, fonts, spacing scale).
- Update shadcn component tokens (e.g., `theme.json`, `tailwind.config.js` utilities) to match the custom style guide.
- Validate global resets, typography, and dark/light mode expectations in a simple prototype screen before full UI build-out.

## Phase 4 – Implement Wallet-Centric UI
- Build core screens/components per wireframe:
  - Dashboard card with wallet balance, status tiles, and quick actions (Receive/Send buttons, settings trigger).
  - Receive flow with QR code display, copy/add amount controls.
  - Send flow with payment request input, validation, and submit CTA.
  - Feature toggle panel (tollgate, 402 proxy, NWC request widgets) with expandable details/budget inputs.
- Connect React state management (Context or Zustand/Redux if needed) to Tauri commands and existing backend APIs.
- Ensure accessibility (focus management, keyboard shortcuts) and desktop layout responsiveness (min/max window sizes).

## Phase 5 – Integration & QA
- Replace temporary placeholders with live data bindings; verify IPC calls, error handling, and loading states.
- Write/port unit and integration tests for critical flows (balance fetching, receive QR generation, send form submission, toggle persistence).
- Run end-to-end smoke tests in Tauri dev/production builds (macOS desktop focus) and confirm Android build compiles with shared code.
- Collect design feedback on the React implementation, iterate on styling adjustments, and freeze component API for future reuse.

## Phase 6 – Launch & Cleanup
- Update documentation (README, developer setup guides) to reflect the new tech stack and commands.
- Remove any unused assets/tooling left from the Svelte era and ensure CI pipelines (lint/test/build) point to React scripts.
- Plan follow-up backlog items (e.g., real desktop Wi-Fi implementations, deferred feature parity) before announcing completion.

## Open Questions
- Preferred state management library (pure React hooks vs. Zustand/Redux)?
- Any localization or theming requirements beyond the supplied CSS (dark mode, RTL)?
- Testing philosophy: maintain current coverage levels or expand with new E2E coverage?

## Next Steps
- Confirm answers to open questions.
- Schedule Phase 0 inventory session and assign owners.



here is the shadcn index.css theme we'll be using (just ignore the dark mode for now though)

```index.css
:root {
  --background: oklch(1.0000 0 0);
  --foreground: oklch(0.5542 0.2465 261.4449);
  --card: oklch(1.0000 0 0);
  --card-foreground: oklch(0.5542 0.2465 261.4449);
  --popover: oklch(1.0000 0 0);
  --popover-foreground: oklch(0.5542 0.2465 261.4449);
  --primary: oklch(0.5542 0.2465 261.4449);
  --primary-foreground: oklch(0.9401 0 0);
  --secondary: oklch(1.0000 0 0);
  --secondary-foreground: oklch(0.5542 0.2465 261.4449);
  --muted: oklch(0.9401 0 0);
  --muted-foreground: oklch(0.5542 0.2465 261.4449);
  --accent: oklch(0.9401 0 0);
  --accent-foreground: oklch(0.5542 0.2465 261.4449);
  --destructive: oklch(0.6290 0.1902 23.0704);
  --destructive-foreground: oklch(1.0000 0 0);
  --border: oklch(0.5542 0.2465 261.4449);
  --input: oklch(0.5542 0.2465 261.4449);
  --ring: oklch(0.5542 0.2465 261.4449);
  --chart-1: oklch(0.5542 0.2465 261.4449);
  --chart-2: oklch(0.5542 0.2465 261.4449);
  --chart-3: oklch(0.7187 0 0);
  --chart-4: oklch(0.9189 0 0);
  --chart-5: oklch(0.5590 0 0);
  --sidebar: oklch(1.0000 0 0);
  --sidebar-foreground: oklch(0.5542 0.2465 261.4449);
  --sidebar-primary: oklch(0.5542 0.2465 261.4449);
  --sidebar-primary-foreground: oklch(1.0000 0 0);
  --sidebar-accent: oklch(0.9168 0.0214 109.7161);
  --sidebar-accent-foreground: oklch(0.5542 0.2465 261.4449);
  --sidebar-border: oklch(0.5542 0.2465 261.4449);
  --sidebar-ring: oklch(0.5542 0.2465 261.4449);
  --font-sans: Geist Mono, sans-serif;
  --font-serif: Playfair Display, serif;
  --font-mono: Fira Code, monospace;
  --radius: 0.2rem;
  --shadow-2xs: 2px 2px 4px 0px hsl(0 0% 83.9216% / 0.10);
  --shadow-xs: 2px 2px 4px 0px hsl(0 0% 83.9216% / 0.10);
  --shadow-sm: 2px 2px 4px 0px hsl(0 0% 83.9216% / 0.20), 2px 1px 2px -1px hsl(0 0% 83.9216% / 0.20);
  --shadow: 2px 2px 4px 0px hsl(0 0% 83.9216% / 0.20), 2px 1px 2px -1px hsl(0 0% 83.9216% / 0.20);
  --shadow-md: 2px 2px 4px 0px hsl(0 0% 83.9216% / 0.20), 2px 2px 4px -1px hsl(0 0% 83.9216% / 0.20);
  --shadow-lg: 2px 2px 4px 0px hsl(0 0% 83.9216% / 0.20), 2px 4px 6px -1px hsl(0 0% 83.9216% / 0.20);
  --shadow-xl: 2px 2px 4px 0px hsl(0 0% 83.9216% / 0.20), 2px 8px 10px -1px hsl(0 0% 83.9216% / 0.20);
  --shadow-2xl: 2px 2px 4px 0px hsl(0 0% 83.9216% / 0.50);
  --tracking-normal: 0em;
  --spacing: 0.25rem;
}

.dark {
  --background: oklch(0.2584 0.1094 260.6533);
  --foreground: oklch(0.9299 0.0334 272.7879);
  --card: oklch(0.2584 0.1094 260.6533);
  --card-foreground: oklch(0.9299 0.0334 272.7879);
  --popover: oklch(0.2584 0.1094 260.6533);
  --popover-foreground: oklch(0.9299 0.0334 272.7879);
  --primary: oklch(0.4969 0.2090 260.5458);
  --primary-foreground: oklch(1.0000 0 0);
  --secondary: oklch(0.4969 0.2090 260.5458);
  --secondary-foreground: oklch(0.9299 0.0334 272.7879);
  --muted: oklch(0.3295 0.1378 260.4459);
  --muted-foreground: oklch(0.8112 0.1013 293.5712);
  --accent: oklch(0.4969 0.2090 260.5458);
  --accent-foreground: oklch(0.9299 0.0334 272.7879);
  --destructive: oklch(0.6368 0.2078 25.3313);
  --destructive-foreground: oklch(1.0000 0 0);
  --border: oklch(0.4172 0.1769 260.6875);
  --input: oklch(0.8244 0.0858 262.6781);
  --ring: oklch(0.8244 0.0858 262.6781);
  --chart-1: oklch(0.7090 0.1592 293.5412);
  --chart-2: oklch(0.6056 0.2189 292.7172);
  --chart-3: oklch(0.5413 0.2466 293.0090);
  --chart-4: oklch(0.4907 0.2412 292.5809);
  --chart-5: oklch(0.4320 0.2106 292.7591);
  --sidebar: oklch(0.2584 0.1094 260.6533);
  --sidebar-foreground: oklch(1.0000 0 0);
  --sidebar-primary: oklch(0.3295 0.1378 260.4459);
  --sidebar-primary-foreground: oklch(1.0000 0 0);
  --sidebar-accent: oklch(0.3295 0.1378 260.4459);
  --sidebar-accent-foreground: oklch(0.9299 0.0334 272.7879);
  --sidebar-border: oklch(0.3295 0.1378 260.4459);
  --sidebar-ring: oklch(0.9401 0 0);
  --font-sans: Roboto, sans-serif;
  --font-serif: Playfair Display, serif;
  --font-mono: Fira Code, monospace;
  --radius: 0.2rem;
  --shadow-2xs: 2px 2px 4px 0px hsl(255 86% 66% / 0.10);
  --shadow-xs: 2px 2px 4px 0px hsl(255 86% 66% / 0.10);
  --shadow-sm: 2px 2px 4px 0px hsl(255 86% 66% / 0.20), 2px 1px 2px -1px hsl(255 86% 66% / 0.20);
  --shadow: 2px 2px 4px 0px hsl(255 86% 66% / 0.20), 2px 1px 2px -1px hsl(255 86% 66% / 0.20);
  --shadow-md: 2px 2px 4px 0px hsl(255 86% 66% / 0.20), 2px 2px 4px -1px hsl(255 86% 66% / 0.20);
  --shadow-lg: 2px 2px 4px 0px hsl(255 86% 66% / 0.20), 2px 4px 6px -1px hsl(255 86% 66% / 0.20);
  --shadow-xl: 2px 2px 4px 0px hsl(255 86% 66% / 0.20), 2px 8px 10px -1px hsl(255 86% 66% / 0.20);
  --shadow-2xl: 2px 2px 4px 0px hsl(255 86% 66% / 0.50);
}

@theme inline {
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-card: var(--card);
  --color-card-foreground: var(--card-foreground);
  --color-popover: var(--popover);
  --color-popover-foreground: var(--popover-foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-destructive: var(--destructive);
  --color-destructive-foreground: var(--destructive-foreground);
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
  --color-chart-1: var(--chart-1);
  --color-chart-2: var(--chart-2);
  --color-chart-3: var(--chart-3);
  --color-chart-4: var(--chart-4);
  --color-chart-5: var(--chart-5);
  --color-sidebar: var(--sidebar);
  --color-sidebar-foreground: var(--sidebar-foreground);
  --color-sidebar-primary: var(--sidebar-primary);
  --color-sidebar-primary-foreground: var(--sidebar-primary-foreground);
  --color-sidebar-accent: var(--sidebar-accent);
  --color-sidebar-accent-foreground: var(--sidebar-accent-foreground);
  --color-sidebar-border: var(--sidebar-border);
  --color-sidebar-ring: var(--sidebar-ring);

  --font-sans: var(--font-sans);
  --font-mono: var(--font-mono);
  --font-serif: var(--font-serif);

  --radius-sm: calc(var(--radius) - 4px);
  --radius-md: calc(var(--radius) - 2px);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) + 4px);

  --shadow-2xs: var(--shadow-2xs);
  --shadow-xs: var(--shadow-xs);
  --shadow-sm: var(--shadow-sm);
  --shadow: var(--shadow);
  --shadow-md: var(--shadow-md);
  --shadow-lg: var(--shadow-lg);
  --shadow-xl: var(--shadow-xl);
  --shadow-2xl: var(--shadow-2xl);
}
```
