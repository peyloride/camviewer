---
name: CamViewer
description: A rack elevation drawn on vellum, for a native desktop camera wall.
colors:
  vellum: "#fcfcfb"
  plate: "#ffffff"
  picture: "#0b0d0e"
  graphite: "#1a1c1e"
  muted: "#6b7075"
  quiet-on-picture: "#9aa0a6"
  hairline: "#c9c7c4"
  non-repro: "#a8c6e8"
  fault: "#1d4ed8"
  paper-wash: "#f0f0ee"
  control-hover: "#33363a"
  control-active: "#000000"
  field-focus: "#e4e4e1"
  scrim: "rgba(26, 28, 30, 0.8)"
typography:
  display:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "25px"
    fontWeight: 600
  title:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "22px"
    fontWeight: 600
  heading:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "28px"
    fontWeight: 500
  unit-name:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "21px"
    fontWeight: 500
  unit-name-compact:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 500
  body:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 500
  control:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
  caption:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 500
  label:
    fontFamily: "IBM Plex Mono, ui-monospace, monospace"
    fontSize: "11px"
    fontWeight: 400
  meta:
    fontFamily: "IBM Plex Mono, ui-monospace, monospace"
    fontSize: "12px"
    fontWeight: 400
rounded:
  none: "0px"
spacing:
  tight: "2px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
  unit-gap: "10px"
  sheet-pad: "26px"
  body-top: "14px"
components:
  button-outline:
    backgroundColor: "{colors.plate}"
    textColor: "{colors.graphite}"
    typography: "{typography.control}"
    rounded: "{rounded.none}"
    padding: "0 {spacing.sm}"
    height: "32px"
  button-filled:
    backgroundColor: "{colors.graphite}"
    textColor: "{colors.vellum}"
    typography: "{typography.control}"
    rounded: "{rounded.none}"
    padding: "0 {spacing.sm}"
    height: "32px"
  button-filled-hover:
    backgroundColor: "{colors.control-hover}"
    textColor: "{colors.vellum}"
  button-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    typography: "{typography.control}"
    rounded: "{rounded.none}"
    padding: "0 {spacing.sm}"
    height: "32px"
  input-field:
    backgroundColor: "{colors.vellum}"
    textColor: "{colors.graphite}"
    typography: "{typography.control}"
    rounded: "{rounded.none}"
    padding: "0 10px"
    height: "32px"
  state-symbol:
    backgroundColor: "{colors.plate}"
    textColor: "{colors.graphite}"
    rounded: "{rounded.none}"
    size: "13px"
    width: "13px"
    height: "13px"
  unit:
    backgroundColor: "{colors.plate}"
    textColor: "{colors.graphite}"
    typography: "{typography.unit-name}"
    rounded: "{rounded.none}"
    padding: "{spacing.md}"
---

# Design System: CamViewer

## Overview

**Creative North Star: "The Drawing-Office Rack Elevation Sheet"**

CamViewer's window is not a dashboard of camera windows; it is one continuous
vellum sheet, ruled in graphite, on which every camera holds a numbered unit in
a rack elevation. The sheet is an artifact with one piece of fixed furniture, the
title block, and the wall of units gives way around it rather than the block
stretching to the wall. Reading it is meant to feel like reading a drawing: the
eye runs down the wall and finds the one unit that is not normal without
counting or hovering.

The palette is the drawing's, not a theme's. White vellum, graphite ink, a pale
non-repro blue for a cause printed on the picture, and one drafting blue
reserved for a unit that has something wrong with it. Density is a ruled
sheet's: tight mono captions and generous black picture fields. Type is drawing-office lettering —
uppercase Archivo for the sheet's own voice, tabular IBM Plex Mono for anything
measured, numbered or stated as a state. There is no rounding and no shadow
anywhere: the sheet is flat, and depth is carried by rules and by the picture
fields alone.

The world explicitly refuses the category's dark dashboard and its black tile
grid, and it refuses to follow the operating system into a dark appearance — a
vellum sheet cannot invert and stay the same artifact.

**Key Characteristics:**
- One continuous sheet: title block and wall of units.
- Vellum ground, graphite ink, pale non-repro blue for a cause on the picture.
- One drafting blue, claimed by the unit that is not normal.
- Squared corners throughout; no shadow; no rounding.
- Uppercase Archivo for the voice, IBM Plex Mono for measurement and state.
- The state is printed, never implied: a word and a symbol, always visible.
- Fixed sheet measurements; the wall adapts, the furniture does not.

## Colors

The palette is a drawing sheet's: a white paper, one graphite ink, one pale blue
for a cause on the picture, and one saturated drafting blue kept for a single
job.

### Primary
- **Drafting Fault Blue** (`#1d4ed8`): claimed by the sheet for exactly one job —
  a unit that is not normal. It draws that unit's frame, its caption rule, its
  state word and its state symbol. Nothing else in the interface may be this
  blue, and no control is ever filled with it.

### Secondary
- **Non-Repro Blue** (`#a8c6e8`): the palest blue on the sheet, used as the ink
  for a cause printed *inside* a picture field, where the drafting blue is too
  dark to read.

### Neutral
- **Vellum** (`#fcfcfb`): the sheet ground, the root background, and the fill of
  an unassigned picture field waiting for a drawing.
- **Plate** (`#ffffff`): a caption plate, a dialog card, a unit's body — any
  surface that must sit above the vellum.
- **Picture** (`#0b0d0e`): the picture field. Near-black on purpose, so video is
  judged against the same ground at any hour; it never follows the theme.
- **Graphite** (`#1a1c1e`): every rule, every name, every number, and the one
  filled control on a view.
- **Muted** (`#6b7075`): secondary words — the host, the sheet meta, a quiet
  state, a field label.
- **Quiet-on-Picture** (`#9aa0a6`): a quiet cause printed inside a picture field,
  where the caption's muted grey would not carry.
- **Hairline** (`#c9c7c4`): the theme's border token — the softest pencil rule,
  for the library's own drawn edges.
- **Paper Wash** (`#f0f0ee`): the hover shade of the paper under a control.
- **Control Hover** (`#33363a`): the hover shade of a filled graphite control.
- **Control Active** (`#000000`): the pressed shade of a filled control.
- **Field Focus** (`#e4e4e1`): the wash behind a focused caption and a pressed
  ruled plate.
- **Scrim** (`rgba(26, 28, 30, 0.8)`): the backdrop behind the add dialog and
  the revision-notes card.

### Named Rules
**The One Job Rule.** The drafting blue (`#1d4ed8`) is claimed by the unit that
is not normal and by nothing else. It is never a control fill, never a hover
state, never decoration. Its meaning depends on its rarity.

**The Non-Repro Rule.** The pale non-repro blue (`#a8c6e8`) is picture-field ink
only. It marks a cause printed where the drafting blue would be unreadable —
never a control, never a name, never decoration.

## Typography

**Display Font:** Archivo (embedded, variable; with `system-ui, sans-serif`)
**Body Font:** Archivo (the same face carries the sheet's voice at every size)
**Label/Mono Font:** IBM Plex Mono (embedded, Regular / Medium / SemiBold; with
`ui-monospace, monospace`)

**Character:** Both faces are embedded and shipped under the SIL Open Font
Licence, so the sheet is the same artifact on Linux, macOS and Windows. Archivo
carries the sheet's own voice in uppercase — the wordmark, unit names, state
words and control labels. IBM Plex Mono carries everything measured, numbered or
stated as furniture: sheet meta, the host, field labels and the state word in a
caption. Numbers are monospaced so they do not move when they change.

### Hierarchy
- **Display** (600, 25px): the CAMVIEWER wordmark in the title block.
- **Heading** (500, 28px): the first-run headline, "NO CAMERAS ON THIS SHEET".
- **Title** (600, 22px): dialog titles — "ADD UNIT", "REVISION NOTES".
- **Unit name** (500, 21px; 15px when the unit is compact): a camera's name in
  its caption, uppercased. It keeps its room up to half the caption and truncates.
- **Body** (500, 14px): a cause or a placeholder printed in a picture field.
- **Control** (400, 14px, uppercase): every button label.
- **Caption** (500, 12px): an add-dialog validation error.
- **Label** (400, 11px mono): sheet meta, the host, legend words, field labels,
  the state word in a caption.
- **Meta** (400, 12px mono): the amendment strip's message.

### Named Rules
**The Two Voices Rule.** Archivo speaks; IBM Plex Mono measures. If a string is a
name, a word or a label, it is Archivo; if it is a number, an address, a
measurement, a revision or a state, it is IBM Plex Mono.

**The No-Tracking Rule.** The sheet's capitals are tracked by nothing, because
GPUI has no letter-spacing. Uppercase is carried by case, weight and size alone;
do not ask a future surface to add tracking, and do not fake it with spacing
characters. This is a framework limit, not a choice.

## Layout

The window is one sheet. An 88px title block runs across the top, ruled under by
a 2px graphite rule; below it sits an optional 42px amendment strip while a
newer build is offered. The sheet's body is inset by a 26px page margin, and the wall
fills it directly under the title block.

The wall is the squarest grid that holds every camera: 1 fills the window, 2 sit
side by side, 3–4 make a 2×2 block, 5–9 make a 3×3 block. Units fill their cell
and are separated by a 10px gap, so the wall covers the whole window. A short
last row keeps the grid complete with an unassigned slot that invites the next
camera. The wall's available box is the viewport minus the page margin, the
title block, the amendment strip and the body top gap — the wall gives way to
the furniture, never the reverse.

Units adapt in three steps, each a property of the *unit*, not the window:

- **Compact** (narrower than 400px): the name drops from 21px to 15px and the
  secondary column is given up.
- **Wide** (at least 560px): the host and the sound/remove controls are carried
  in the open, beside the name and the state. Narrower, both wait for the pointer,
  so the name and the state keep their room.
- The title block gives up its meta line below 780px of window width, and its
  state-symbol legend below 1040px.

When a camera is drawn alone, a 34px unit-data strip sits between the title
block and the sheet body, on the plate and ruled under by a hairline. It prints
what the stream is and how it is behaving: which address is drawn, the decoded
picture size, the measured frame rate, the codec, and how long the connection
has held, with the host at the end. Every field is 11px mono, a muted label and
an ink value. The strip appears only in that view, so the wall keeps its full
height. It gives up its host first, then its connected time, its codec and its
stream label as the window narrows; the picture size and the rate stay last,
because they are what the strip exists for. A frozen feed prints `0.0 FPS`:
the rate belongs to what is on screen now.

Spacing is a ruled sheet's: a 10px unit gap, a 26px page margin, a 14px gap
between the title block and the body, and an internal rhythm of 2, 8, 12, 16 and
20px inside captions, dialogs and the legend.

### Named Rules
**The Fixed-Furniture Rule.** The sheet's measurements are the drawing's: the
title block, caption and gaps are fixed, and the wall's grid is computed from
whatever is left. Never grow the furniture to fill the window.

**The Give-Way Rule.** When a unit is short of room, the address and the controls
give way first; then the name's size and its secondary column. The name and the
state are what the wall exists for, so they are what stays.

## Elevation & Depth

There are no shadows and no elevation. `paint_the_sheet` sets the theme's
`radius` and `radius_lg` to zero and `shadow` to false, and that is a deliberate
part of the world, not an omission. Depth is carried instead by tonal layering —
vellum ground, white plate, near-black picture field — and by rules: a 1px line
for a symbol and the softest rules, a 2px rule for the one that carries weight
(under the title block, around a unit, under a caption, around a dialog). A raised
surface is simply a plate with a 2px graphite rule around it.

### Named Rules
**The Flat Sheet Rule.** Nothing is rounded and nothing floats. Every corner is
square (0px) and no element carries a shadow. A surface that needs to sit above
the sheet earns it with a plate and a rule, never with elevation.

## Shapes

The form language is squared and ruled. Radii are zero everywhere — the theme
sets `radius` and `radius_lg` to `0px`, and the library's buttons and inputs
inherit that, so a control is a rectangle. Borders are the primary shape
carrier: a 1px hairline for a symbol and the softest rules, a 2px rule for a
unit's frame, a caption's top edge, a dialog and the title block's underline.
Picture fields are hard-clipped rectangles (overflow hidden) so a letterboxed
picture never bleeds past its unit. The state symbol is a 13–14px square, drawn
filled, half-filled or hollow by one shared function so the legend and a caption
cannot disagree.

### Named Rules
**The Square Corner Rule.** Every corner is a right angle (0px). Rounding is
never introduced to soften a control, a card or a dialog.

## Components

The world supplies no controls of its own. Every control is the platform's own
`Button` from gpui-kit, skinned by the theme, so the keyboard reaches it and
Enter or Space activates it. A control is ruled, squared and set in the sheet's
voice; it is never blue, because the fault colour is claimed by the one unit that
has something wrong with it.

### Buttons
Three tones, and the tone vocabulary is a durable rule, not a detail.

- **Shape:** squared (0px radius), 32px tall, 8px horizontal padding, labels
  uppercased in Archivo at 14px.
- **Outline (`Tone::Outline`, `primary().outline()`):** a ruled plate — a 1px
  graphite rule with graphite type on a faint graphite wash (graphite at 10%,
  hover at 20%). The sheet's default control: Full screen, All cameras, Later,
  Install and restart, Cancel, and + Add camera while the sheet is empty.
- **Filled (`Tone::Filled`, `primary()`):** the single filled control a view is
  allowed — graphite (`#1a1c1e`) with vellum (`#fcfcfb`) type, hover
  `#33363a`, active `#000000`. It is + Add camera, and the confirming button of a
  dialog (Add, Filed).
- **Quiet (`Tone::Quiet`, `ghost()`):** a word with a hover, no box around it —
  muted (`#6b7075`) type on transparent. It is the hover-revealed Remove and the
  hover-revealed Sound on a unit too narrow to carry them in the open.
- **Hover / Focus:** a hover is a shade of the paper or a lightening of the
  graphite, never a wash of colour; a focus ring is the drafting blue
  (`#1d4ed8`), the theme's `ring` token.

### Inputs / Fields
- **Style:** the library's `Input`, squared, 32px tall, 14px Archivo text on a
  vellum (`#fcfcfb`) field, caret graphite (`#1a1c1e`). The add dialog sets
  `cleanable(true)` so a field can be cleared. Field labels are 11px mono muted
  above the field; an optional note is 11px Archivo muted below it.
- **Focus:** the theme's `ring` token — the drafting blue.
- **Error:** the add dialog's validation error prints in 12px Archivo medium in
  the drafting blue below the fields.

### Signature Component — the Rack Unit
- **Character:** one camera, one numbered unit, framed like a panel on an
  elevation.
- **Frame:** a 2px rule in graphite (`#1a1c1e`) — or in the drafting blue
  (`#1d4ed8`) when the unit is not normal — around a white plate body.
- **Picture field:** a near-black (`#0b0d0e`) rectangle that fills the unit above
  the caption. A live or frozen stream draws its last frame, letterboxed inside
  it; otherwise it prints a cause in the picture-field ink (non-repro blue for a
  named fault, quiet-on-picture grey for a connection in progress).
- **Caption:** a 42px plate strip under a 2px rule, holding the uppercased name
  (21px/15px), the host in 11px mono when the unit is wide, a reserved slot for
  the remove control, the sound control, then the state word and its symbol at
  the far right. The remove control keeps its width while it is hidden, so the
  sound control sits beside the state and the caption never shifts.
- **State:** `ZoneState` — Live, Connecting, Frozen, NoSignal — always printed as
  a word and a symbol, never hidden behind a hover. Live and Frozen carry a
  filled square, Connecting a half-filled one, NoSignal a hollow one. A stream
  that has been live but has produced no frame for three seconds (`FROZEN_AFTER`)
  is reported Frozen rather than drawn as if it were live.
- **The fault rule:** a fault changes a unit's *colour* — its frame, its caption
  rule, its state word and its symbol — and never its *size*. A wall that shifts
  when something goes wrong is a wall you cannot read across a room.
- **Behaviour:** a unit is focusable; Enter or Space enlarges it, and the same
  action on the enlarged view returns to the wall. The frame, the caption rule
  and the symbol are the same weight whether or not the unit is in fault.

### Dialogs and the Amendment Strip
The add-camera card (520px) and the revision-notes card (560px) are white plates
with a 2px graphite rule, 20px internal padding, over a `rgba(26, 28, 30, 0.8)`
scrim that occludes the sheet behind. Titles are 22px Archivo semibold with an
11px mono meta line beneath. The amendment strip is a 42px plate ruled under the
title block, carrying a 12px mono message and outline actions.

## Do's and Don'ts

### Do:
- **Do** keep the sheet's measurements fixed — title block, caption, gaps — and
  compute the wall's grid from what is left.
- **Do** print the state: a word and a symbol, always visible, in every unit.
- **Do** spend the drafting blue (`#1d4ed8`) only on a unit that is not normal,
  and only on its frame, caption rule, state word and symbol.
- **Do** use the non-repro blue (`#a8c6e8`) only for a cause printed inside a
  picture field.
- **Do** square every corner (0px) and carry depth with a plate and a rule.
- **Do** set names and labels in Archivo and everything measured, numbered or
  stated as a state in IBM Plex Mono.
- **Do** allow exactly one filled control per view; step it back to outline when
  another action is the view's purpose.
- **Do** let the address and the controls give way on a narrow unit so the name
  and the state keep their room.

### Don't:
- **Don't** round a corner or add a shadow anywhere; the theme sets both to none.
- **Don't** use the drafting blue as a control fill, a hover, or decoration — its
  meaning is its rarity.
- **Don't** put a fault on a control; the fault belongs to a unit.
- **Don't** let a fault change a unit's size or the weight of its rule.
- **Don't** follow the operating system into a dark appearance; a vellum sheet
  cannot invert and stay the same artifact. (The OS still draws the window frame.)
- **Don't** hide the state word behind a hover, or replace it with a colour alone.
- **Don't** add letter-spacing or fake tracking; GPUI has none, and the capitals
  are carried by case, weight and size.
- **Don't** introduce recording, playback, a timeline or an event list — the
  product is live-only, and the recorder beside it owns all of that.
