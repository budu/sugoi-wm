# Implementation decisions

What the plans leave open, decided as the milestones go. Newest entries at the top of
each section.

## Configuration

- **CRuby, embedded** (user, 2026-10-31). Full Ruby and gems through `magnus`, rather
  than mruby: the config is personal, so sandboxing matters less than the language and
  the bindings. The cost is a build tied to one libruby (0.1 plan, Risks).

## Layouts

- **Tile, max and float only** (user, 2026-10-31). awesome's fair, bottom and top
  variants are dropped.

## Workspaces

- **Cycling by repeat** (user, 2026-10-31). Mod-n goes to n, then n+10, n+20, wrapping,
  decided by the current workspace rather than a timeout.
- **One monitor** (user, 2026-10-31). No multi-monitor support.

## Spawning

- **No workspace freezing** (user, 2026-10-31). Freezing hidden apps' cgroups stops
  background work and breaks network connections.
