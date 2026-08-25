# Orchestr Design System

## Purpose

Orchestr is an engineering control room for completed, integrated, healthy
software progress. Its interface should make project state, workflow pressure,
and operational detail easy to read without resembling a generic SaaS Kanban
tool.

The visual language is deliberately restrained: dense information, dark neutral
surfaces, thin borders, clear status color, and small moments of motion that
explain a state change rather than decorate it.

## Design principles

- **Operational clarity first.** Health, blockers, validation, and integration
  state are more prominent than agent activity or usage metrics.
- **Dense, not crowded.** Keep controls compact and aligned, with whitespace
  used to separate systems rather than to make the interface feel sparse.
- **State is visible.** Color supports status labels, icons, and copy; it is
  never the only cue.
- **Technical, not hostile.** Use monospace for identifiers, commands, branches,
  timestamps, and system metadata. Use a readable sans-serif for task and
  product copy.
- **Motion explains navigation.** Short, subtle transitions establish hierarchy
  and preserve orientation. They must respect reduced-motion preferences.

## Foundations

### Color

| Role | Value | Use |
| --- | --- | --- |
| App background | `#101214` | Application frame and deep surfaces |
| Raised surface | `#15181a` | Sidebars, columns, panels |
| Interactive surface | `#1a1e21` | Cards and selectable content |
| Hover surface | `#22272b` | Hovered controls and cards |
| Standard border | `#292d31` | Primary separation |
| Strong border | `#343c41` | Cards and active containers |
| Primary text | `#e7e9ea` | Headings and important content |
| Muted text | `#89939a` | Supporting copy |
| Technical text | `#8dc4e1` | Paths, commit IDs, and machine metadata |

Status colors should retain the existing workflow semantics:

| State | Color family |
| --- | --- |
| Backlog | neutral / slate |
| Ready | blue |
| In progress | amber |
| Needs input | yellow |
| Review | violet |
| Approved | indigo |
| Integrating | cyan |
| Done / healthy | green |
| Blocked | orange |
| Failed / broken | red |

### Typography

- UI and product copy: `Inter, ui-sans-serif, system-ui, -apple-system,
  BlinkMacSystemFont, "Segoe UI", sans-serif`.
- Technical metadata: `"SFMono-Regular", Consolas, monospace`.
- Page title: 25px, compact tracking, medium weight.
- Section and card titles: 14–15px.
- Technical labels and metadata: 10–11px, usually uppercase when acting as a
  label.

### Borders and shape

- Prefer 1px borders over elevation to define boundaries.
- Use square or minimally rounded surfaces; rounded scrollbar thumbs are an
  exception because they indicate a draggable handle.
- Shadows are reserved for overlays, drag previews, and inspectors that sit
  above another surface.

## Application layout

### Navigation rail

- The persistent left rail contains brand, primary navigation, local status, and
  the collapse control.
- Expanded width is 236px; collapsed width is 58px.
- In the collapsed rail, brand, navigation, and footer icons share the same
  vertical center line.
- Navigation remains visible as icons when collapsed. Labels are hidden, but
  controls must retain accessible names.

### Content areas

- Full pages own their scroll region; do not allow the document body to scroll.
- Boards use independently scrolling columns and horizontal overflow for the
  kanban lanes.
- Detail, repository, integration, quality, and flow panels use their own
  scroll region so the board remains stable behind them.
- Preserve a minimum content width and avoid horizontal clipping of task
  metadata. Use ellipsis only for long secondary values such as paths.

### Cards and panels

- Use a dark raised surface with a thin border.
- A card’s primary action is the title/content area; secondary actions are
  compact and may be revealed on hover when an accessible alternative exists.
- Keep technical status and workflow state close to the entity they describe.
- Use panels for detailed operations instead of navigating away from the board
  when context is valuable.

## Interaction and motion

- Standard hover/focus feedback: 120–160ms.
- Navigation/layout transitions: roughly 150–220ms, `ease-out`.
- Opening a project board uses a two-stage entry: header first, then board body
  after a short delay. The vertical movement is small (6–10px) and paired with
  a fade.
- Running work may use a slow, low-contrast pulse; never use flashing or
  continuous high-attention motion.
- Honour `prefers-reduced-motion: reduce` by disabling non-essential animation.

## Scrollbars

Scrollbars are part of the product surface rather than an operating-system
default. Use the global treatment for all scrollable regions:

- compact 10px rail;
- deep neutral track (`#121517`);
- inset, rounded slate thumb (`#48545b`);
- lighter thumb on hover and muted blue while active;
- Firefox uses `scrollbar-width: thin` and matching `scrollbar-color`.

Avoid per-component scrollbar styles unless a specific high-density surface
needs a documented exception.

## Accessibility

- Never communicate workflow state by color alone; combine it with a label,
  icon, or text value.
- Preserve visible keyboard focus for interactive controls.
- Maintain semantic buttons, links, headings, and labelled icon-only controls.
- Do not make hover the only way to reach a necessary action.
- Respect reduced-motion preferences.
- Keep contrast high enough for prolonged technical work in a dark interface.

## Implementation guidance

- Put shared tokens and cross-cutting behavior in `apps/desktop/src/styles/global.css`.
- Keep component-specific layout and state styling beside its component.
- Do not introduce a heavyweight component library solely for visual polish.
- Prefer CSS transitions and keyframes to JavaScript-driven animation for UI
  state that does not need physics or measurement.
- Validate desktop layouts in both expanded and collapsed navigation states,
  and at narrow window widths.

## Review checklist

Before accepting a UI change, confirm:

- It makes integrated project progress or current workflow state easier to
  understand.
- It fits the dark, industrial, technical visual language.
- It works with the navigation rail expanded and collapsed.
- Scroll behavior remains contained to the intended region.
- It remains usable with keyboard navigation and reduced motion.
- New colors follow the established semantic status palette.
