# UI design system

> Status: **specification.** No interface exists yet (Windows arrives in Phase 6, Android in Phase 7). Every value here is the single source of truth for both platforms once they are built.

The interface must read as a premium financial instrument: dark, calm, precise, spacious. It should look expensive because of proportion, typography and restraint — not because of glow, gradients or decoration.

## 1. Colour

Black and purple are the only brand colours. Neutral greys exist for text, borders and surfaces. Semantic colours are functional only and never decorative.

### Surfaces

| Token | Value | Use |
| --- | --- | --- |
| `bg/base` | `#09090D` | Window background, behind everything |
| `bg/secondary` | `#111015` | Sidebar, secondary panels, table header |
| `surface` | `#17121F` | Cards, list rows, inputs |
| `surface/elevated` | `#20182B` | Modals, dropdowns, hovered cards, popovers |
| `surface/inset` | `#0D0C11` | Inset wells, code, chart plot background |

### Brand

| Token | Value | Use |
| --- | --- | --- |
| `brand/primary` | `#8547E8` | Primary buttons, active navigation, focus rings, selected rows |
| `brand/light` | `#A879E8` | Hover of primary elements, chart accent, links |
| `brand/soft` | `#C9A5FF` | Icons on dark surfaces, small highlights |
| `brand/subtle` | `rgba(133, 71, 232, 0.12)` | Active nav background, selected row tint, badge fill |
| `brand/border` | `rgba(133, 71, 232, 0.35)` | Border of active/selected elements |

### Text and borders

| Token | Value | Use |
| --- | --- | --- |
| `text/primary` | `#F4F1F8` | Titles, values, primary content |
| `text/secondary` | `#A8A1B2` | Descriptions, labels, metadata |
| `text/tertiary` | `#6E6779` | Disabled text, timestamps, placeholders |
| `border/subtle` | `#30283A` | Default dividers and card borders |
| `border/strong` | `#453A52` | Input borders, emphasized separators |

### Semantic (functional only)

| Token | Value | Use |
| --- | --- | --- |
| `success` | `#3FB68B` | Confirmed transaction, verified integrity — only when actually true |
| `warning` | `#D8A24A` | Stale price, unverified backup, sync conflict |
| `danger` | `#D9576B` | Failed transaction, destructive action, revoked device |
| `info` | `#8E8AA8` | Neutral notices |

Rules: never use semantic colour for decoration; never imply a verified state with green unless the check ran; text on a coloured fill must keep a 4.5:1 contrast ratio.

## 2. Typography

One family per platform, chosen for a technical/financial character and excellent numeric rendering.

* **Windows:** Inter (UI) with `tabular-nums` for all figures. Bundle the font; do not rely on what is installed.
* **Android:** Inter or the platform system font, matching weights and sizes.
* Fallback stack: `Inter, "Segoe UI Variable", "Segoe UI", Roboto, sans-serif`.

| Token | Size / line-height | Weight | Use |
| --- | --- | --- | --- |
| `display/amount` | 40 / 48 | 600 | Total portfolio value |
| `title/page` | 24 / 32 | 600 | Page title |
| `title/section` | 18 / 26 | 600 | Section heading |
| `title/card` | 15 / 22 | 600 | Card title |
| `body` | 14 / 22 | 400 | Default text |
| `body/strong` | 14 / 22 | 500 | Emphasis, table primary cell |
| `caption` | 12 / 18 | 400 | Metadata, helper text |
| `overline` | 11 / 16 | 600, letter-spacing 0.06em, uppercase | Column headers, section labels |
| `mono` | 13 / 20 | 400 | Addresses, transaction ids, derivation paths |

Rules: at most two weights on one screen besides body; no all-bold screens; numbers right-aligned in tables with tabular figures; addresses truncated in the middle with a copy action, never at the end.

## 3. Spacing, radii, elevation

* Spacing scale: `4, 8, 12, 16, 20, 24, 32, 40, 48`. All margins and paddings come from this scale.
* Card padding: 20 or 24. Never below 16. Page gutter: 32 (desktop), 20 (compact), 16 (mobile).
* Radii: `sm 6` (inputs, small buttons, badges), `md 10` (buttons, list rows), `lg 14` (cards), `xl 20` (modals, summary card). Never fully rounded except avatars and status dots.
* Borders: 1px `border/subtle` on every surface. A card without a border must instead have a distinctly lighter surface.
* Shadows: `0 1px 2px rgba(0,0,0,.35)` for cards, `0 20px 48px rgba(0,0,0,.55)` for modals. No coloured glow on buttons, no inner neon strokes.
* Grid: 12 columns, 24px gutter, maximum content width 1200 (desktop). Charts and tables may span the full content area; text blocks stay under 720px for readability.

## 4. Iconography

* One family: Lucide (or an equivalent 1.5px-stroke, 24px-grid set) on both platforms.
* Sizes: 16 (inline, table), 20 (navigation, buttons), 24 (page actions). Stroke weight stays consistent at the same size.
* Icons never replace a label in primary navigation on desktop; mobile bottom navigation may use icon + label.
* No emoji as interface icons. No decorative illustration inside cards. Network marks are monochrome glyphs at 20px inside a 28px circle with `surface/elevated`.

## 5. Layout

### Windows (≥ 960px)

```text
┌──────────────┬──────────────────────────────────────────────────────────┐
│ 240–260 px   │ page gutter 32                                            │
│ sidebar      │  title row:  title · subtitle · status · actions          │
│              │  content: max 1200, 12-column grid                        │
│ logo         │                                                           │
│ nav (9)      │                                                           │
│ …            │                                                           │
│ status block │                                                           │
│ version      │                                                           │
└──────────────┴──────────────────────────────────────────────────────────┘
```

* Sidebar: `bg/secondary`, right border `border/subtle`, logo and wordmark 20px from top, section label `overline` before the navigation group.
* Active item: `brand/subtle` fill, 3px `brand/primary` indicator on the left inside the item, text `text/primary`; inactive text `text/secondary`.
* Sidebar footer: a compact security block (lock state, watch-only count) and the version string in `caption`/`text/tertiary`. It shows real state only.
* Below 960px the sidebar collapses to a 72px icon rail; below 720px (or when the window is narrow) the layout switches to a compact single-column stack. The desktop app must not stretch a phone layout.

### Android

* Bottom navigation with five destinations: Overview, Wallets, Portfolio, Activity, Settings. Security, Devices & Sync and Backups live in Settings (and in the Overview security card).
* Top app bar: title left, notifications and overflow right; security/device status as a small pill when relevant.
* Content: 16px horizontal padding, vertical rhythm of 8px, cards stacked with 12px gaps; no dense tables — use list rows with two lines (label + value).
* Minimum touch target 48×48 dp; safe-area insets respected; content scrolls under the app bar with a scrim, never clipped.
* Bottom sheets for filters and confirmations; full-screen flows for pairing, restore and secret entry.

## 6. Components and required states

| Component | States |
| --- | --- |
| Button (primary / secondary / ghost / danger) | default, hover, pressed, focused (2px `brand/primary` ring at 2px offset), disabled, loading (spinner replaces label, width preserved) |
| Icon button | default, hover, pressed, focused, disabled, tooltip |
| Input / text field | default, hover, focused, filled, disabled, read-only, error (message below, `danger` border), success hint |
| Search field | empty, typing, with clear action, no results |
| Card | default, hover (elevated surface, no movement beyond 1px), selected, disabled, loading skeleton |
| Table | header, sortable column, hover row, selected row, empty, loading, error, pagination |
| Status pill | neutral, success, warning, danger, info, pending (animated dot) |
| Toggle / checkbox / radio | on, off, indeterminate, focused, disabled, error |
| Modal / dialog | default, destructive variant, loading, validation error, scrollable body, max height 80vh |
| Dropdown / select | closed, open, focused option, selected, disabled, searchable |
| Toast | neutral, success, warning, error; auto-dismiss 5s for info, never for errors |
| Empty state | icon (20px, `text/tertiary`), one-line title, one-sentence explanation, single action |
| Loading | skeleton rows matching final layout, or centred spinner ≥ 400ms delay; never a blank screen |
| Error | what failed, why (if known), what the user can do, retry action; never a raw stack trace |
| Confirmation dialog | consequence stated, destructive action labelled with the verb (`Delete wallet`), cancel is the default focus |

Every screen must be designed with empty, loading, error and partial-data states **before** it is considered finished. A screen that only looks correct with populated data is incomplete.

## 7. Data presentation rules

* Currency is always labelled: `$ 12,480.55 USD`, `۲٬۴۹۶٬۱۱۰ تومان` with the unit word or code beside the number. Rial and toman are distinct: never show a number without the unit.
* Exchange-rate source and timestamp appear near any converted value ("USD/IRR · from <source> · updated 12:40").
* Prices that are stale or unavailable show `—` with a `Stale` / `Unavailable` pill. A zero is never used as a substitute for unknown data.
* The hide-values toggle hides amounts everywhere (portfolio, wallet rows, balances) and persists as a preference; card layouts keep their dimensions so nothing shifts when toggled.
* Charts: 1.5px line, `brand/light` stroke, subtle area fill at 10% opacity, horizontal gridlines only at 24% `border/subtle`, no vertical grid, axes in `caption`/`text/tertiary`, tooltip is a small elevated surface with a 1px border. Empty chart area shows its empty state, not a flat invented line.
* Transaction direction: incoming `success` arrow, outgoing `text/secondary` arrow, pending animated dot, failed `danger` with a reason.

## 8. Motion

| Token | Duration | Use |
| --- | --- | --- |
| `motion/fast` | 120 ms | Hover, button press, toggle |
| `motion/base` | 180 ms | Page transition, modal fade, dropdown |
| `motion/slow` | 240 ms | Chart range change, panel expand |

Easing: `cubic-bezier(0.2, 0, 0.2, 1)`. Rules: motion communicates state change only; no looping animation except a loading indicator and a pending status dot; respect "reduce motion" (cross-fade instead of slide); never animate on every frame of a live price update.

## 9. Accessibility

* Contrast: body text ≥ 4.5:1, large text ≥ 3:1, borders and focus rings ≥ 3:1 against their background.
* Visible focus for every interactive element; keyboard reachable in a logical order; Escape closes modals; Enter submits.
* Screen-reader labels on icon-only buttons; live regions for toasts and sync status; amounts announced with unit.
* No state conveyed by colour alone: pair it with text or an icon.
* Windows: support 100–200% display scaling without clipping. Android: support font scale up to 1.3 without truncating values.

## 10. Not allowed

* Glowing buttons, neon outlines, rainbow gradients, mesh backgrounds.
* Pie-chart overload, 3D charts, decorative sparklines without data.
* Glassmorphism on every panel; blur only on modal scrims.
* Emoji as icons, mixed icon families, outlined + filled icons in one row.
* Fake balances, fake transactions, fake security scores, or a green "Protected" badge while the vault is not actually encrypted.
* Centring all text, or using a card for every single line of information.
