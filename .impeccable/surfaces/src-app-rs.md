---
version: 1
slug: "src-app-rs"
primary_target: "src/app.rs"
related_targets: ["tests/ui.rs"]
---

# Surface brief — the camera wall

## Scope and mode

One surface: the CamViewer window. Mode **Operate**. The visitor arrives with a
job, which is to know whether their cameras are up, so the screen is a working
product first and the world second.

## Audience, job, action

Home lab and small site owners with 4 to 8 cameras and a recorder already
running beside this app. The window is left open all day and pushed to full
screen on a second display. They glance at it; they do not sit in front of it.
The one action that matters is reading the wall's state without a click.

## Proof and content

Real data only: the camera's name, the host taken from its RTSP address (never
the credentials), the resolution of the live stream, and the sound state. There
is no recording, no playback and no timeline, so none may appear. States worth
seeing, and every one of them is a real condition: live, connecting, no signal,
frozen.

## Constraints

- Cross-platform desktop: Linux, macOS, Windows. One binary.
- `grid_columns`, `tile_size`, `enlarged_size` and `draw_solo` remain the layout
  contract, with their unit tests.
- One RTSP connection per camera, carrying both picture and sound.
- The measured ffmpeg latency arguments do not change in this work.
- Standard desktop controls only. The world lends type, palette, density and one
  signature move; it never supplies the navigation or the controls.

## Chosen direction and memorable moment

The rack elevation sheet, chosen by the user from the dealt hand. The memorable
moment is the drafting blue claimed by the one unit that is not normal: the blue
frame and the printed state word together, always visible, never on hover.

## Unresolved decisions

- Whether the app keeps following the OS light or dark appearance. A vellum
  sheet cannot invert without becoming a different artifact, so this build holds
  one fixed sheet appearance and states that plainly.

## Direction contract

THESIS: The wall is a rack elevation drawn on vellum, where every camera holds a
numbered unit in one continuous sheet, so the window reads as a record rather
than a mosaic of windows. It refuses the category's dark dashboard and its black
tile grid.

OWN-WORLD: White vellum ground, graphite ink, pale non-repro blue construction
rules, and one drafting blue claimed only by the unit that is not normal.
Squared corners throughout. Uppercase drawing-office lettering for device names,
tabular monospace for numbers and measurement. Density is a ruled sheet's: tight
captions and generous picture fields.

STORY: The visitor glances once and knows whether every camera is up, which one
is not, and when it changed. They count nothing and hover nothing.

FIRST VIEWPORT: A 92px title block ruled in graphite carries CAMVIEWER, RACK
ELEVATION, the sheet meta and a legend of the three state symbols, with Add
camera at its right. Below a dimension line, a 54px numbered margin gutter runs
down the left edge, and the wall is a grid of units. Each unit is a black
picture field over a 40px caption carrying the name, the host, and the state
word with its symbol. The unit that is not normal wears the drafting blue on its
frame, its caption rule and its symbol.

FORM: The rack elevation sheet, position 1 of my ordered list of 7. seed:df71cec3910bdce7

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance.
