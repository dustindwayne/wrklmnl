# UI DESIGN STANDARD

**Version:** 1.1<br>
**Scope:** Universal, project-agnostic interface design, including terminal and console interfaces<br>
**Direction:** Minimal · Modern · Intuitive · Precise · Dimensional

Version 1.1 adds an explicit no-borders directive and a terminal UI implementation direction. These rules take precedence over any older suggestion to use borders, framed panels, or rounded containers.

## 1. Core Philosophy

Create interfaces that feel refined, responsive, and effortless to navigate.

Minimalism is the foundation. Depth, color, motion, and visual effects exist to improve clarity, usability, or character, not to decorate empty space.

**Guiding principles:**

- **Clarity first.** Every element should have an understandable purpose.
- **Intuitive by design.** Common actions should be immediately recognizable.
- **Controlled complexity.** Complex functionality should not require a visually complex interface.
- **Purposeful aesthetics.** Effects must contribute to hierarchy, feedback, or visual identity.
- **Precision.** Clean geometry, consistent spacing, deliberate alignment.
- **Visual restraint.** Use only as much detail as the interface benefits from.
- **Consistency without rigidity.** Maintain a recognizable design language while adapting to context.

The goal is not absolute minimalism. It is eliminating unnecessary visual and interaction complexity.

## 2. Visual Language

### Overall Appearance

- Modern and minimal with subtle dimensional qualities.
- Clean, sharp silhouettes and geometry.
- Generous negative space without wasting usable area.
- Clear separation between primary content and supporting controls.
- Restrained gradients, lighting, and transparency.
- Occasional abstract visual details, only when they strengthen composition.
- Strong visual hierarchy through typography, alignment, spacing, padding, and subtle tones.

### No Borders or Framed Boxes

- **Do not add borders where styling, spacing, padding, alignment, typography, or subtle surface tones will suffice.**
- Do not wrap regions, tabs, controls, or content in decorative frames.
- A rounded or curved box is still a border. Removing its visible stroke does not make an unnecessary enclosing shape acceptable.
- Do not replace outlined boxes with ornamental filled cards that reproduce the same unnecessary framing.
- Avoid horizontal rules, vertical dividers, and terminal box-drawing characters used as decoration.
- Begin without borders. Establish grouping through shared alignment, proximity, whitespace, text hierarchy, and restrained tonal differences.
- A line that carries actual information, such as a plotted data trace, is different from a decorative frame. Keep such marks minimal and necessary to understanding the content.
- Native operating-system window chrome is outside the content layout. Do not recreate it decoratively inside the interface.

### Geometry

- Prefer clean, sharp edges.
- Maintain consistent geometry among related components.
- Avoid excessive rounding and pill-shaped everything.
- Avoid overlapping content, awkward clipping, and decorative geometry that interferes with usability.
- Square corners do not justify an unnecessary container. Remove the container when spacing and styling provide sufficient structure.

### Depth

Create depth through subtle differences in surface tone, elevation, transparency, and lighting. Depth should establish relationships, not compete for attention.

Avoid stacking shadows, gradients, and glass effects on the same element without reason. Do not use depth effects to disguise unnecessary framing.

## 3. Color System

Support both dark and light themes as first-class designs.

### Dark Theme

| Token | Reference |
|---|---|
| Base background | `#111111` |
| Secondary background | `#1A1A1A` |
| Elevated surface | `#222222` |
| Raised surface | `#2B2B2B` |
| Primary text | `#F5F5F5` |
| Secondary text | `#A3A3A3` |

Dark surfaces should use charcoal variation rather than relying exclusively on pure black.

### Light Theme

| Token | Reference |
|---|---|
| Base background | `#FAFAFA` |
| Secondary background | `#F1F1F1` |
| Elevated surface | `#FFFFFF` |
| Raised surface | `#EAEAEA` |
| Primary text | `#171717` |
| Secondary text | `#626262` |

Light themes should preserve the same precision and dimensional hierarchy as dark themes. Legacy subtle-border tokens are intentionally omitted; they are not an instruction to draw borders.

### Accent Colors

- Use one dominant accent color per interface.
- Default preferred accent: `#FF0077`.
- Accent colors should be configurable rather than permanently embedded into component styling.
- Use accents for active states, selected controls, focus, primary actions, and meaningful highlights.
- Keep neutral colors dominant.
- Avoid scattering accent colors across unrelated decorative elements.
- Adjust accent intensity and contrast independently for dark and light themes.
- Do not introduce neon or glow to express an accent.

Color should communicate purpose before personality.

## 4. Typography

Typography should establish hierarchy primarily through size, weight, spacing, and contrast.

### Direction

- Prefer clean, modern sans-serif fonts for graphical interfaces.
- Ubuntu-style typography is a strong reference.
- Use a readable monospace font for terminal interfaces and aligned technical data.
- Heavy, confident display weights where appropriate.
- Readable body text with comfortable spacing.
- Avoid generic decorative typography and unnecessary font mixing.

### Hierarchy

- Display: bold, deliberate, reserved for significant content.
- Headings: strong, compact, unmistakable.
- Body: readable, neutral, and efficient.
- Labels: concise and consistent.
- Metadata: quieter than content without becoming illegible.

### Rules

- Never shrink text simply to force a layout to work.
- Adapt the layout to the content.
- Avoid excessive letter spacing in normal text.
- Keep text alignment intentional.
- Maintain consistent typographic scale.
- Do not use weight changes randomly.
- Avoid unnecessary all-caps labels. Preserve short established terminal labels when they aid recognition.

## 5. Layout and Spacing

Use a consistent spacing system without making layouts mechanically repetitive.

**Suggested graphical base scale:** 4, 8, 12, 16, 24, 32, 48, 64 pixels.

**Terminal equivalent:** deliberate character-cell padding and blank rows; use a small consistent scale such as 1, 2, 3, and 4 columns or rows according to context.

### Rules

- Prioritize alignment and rhythm.
- Use negative space to establish hierarchy.
- Group related actions spatially.
- Keep frequently used controls accessible.
- Allow content to breathe without sacrificing density.
- Avoid excessive nesting of panels within panels.
- Respect the natural flow of reading and interaction.
- Preserve deliberate element placement.
- Adapt layouts responsively rather than blindly scaling them.
- Separate regions through styling and padding before adding any structural decoration.

### Density

Interface density should match the complexity of the task. Simple interactions benefit from space. Information-heavy interfaces benefit from compact, organized presentation. Neither should sacrifice readability or navigation.

## 6. Navigation and Interaction

Interfaces should feel intuitive without requiring extensive explanation.

### Interaction Principles

- Make the primary action obvious.
- Keep related tools near the content they affect.
- Reduce unnecessary steps.
- Prefer direct manipulation when it improves usability.
- Use progressive disclosure for secondary or advanced controls.
- Maintain predictable behavior across related elements.
- Avoid presenting choices that do not meaningfully differ.
- Provide immediate, understandable feedback.

### Choosing Controls

- Use visible buttons for important graphical actions; use visible, labeled keyboard commands in terminal interfaces.
- Use segmented controls when a small number of mutually exclusive options benefit from direct visibility.
- Use dropdowns only when the number of choices or available space justifies them.
- Use contextual menus for secondary functionality.
- Use sliders when continuous adjustment is intuitive.
- Use direct text input when precision matters.
- Avoid modal dialogs for interactions that can remain within the existing interface.

The interface should minimize decision friction, not eliminate useful choice.

## 7. Component Design

### Buttons

- Clear primary, secondary, and tertiary hierarchy.
- Restrained accent highlights or functional tonal changes.
- Distinct hover, pressed, focused, disabled, and loading states.
- No exaggerated glow or ornamental animation.
- Use text, contrast, spacing, or a restrained state treatment rather than enclosing every action in a box.

### Content Regions

- Use grouping only when it adds meaning.
- Favor spacing and alignment; use subtle tonal separation when needed.
- Avoid excessive nested cards and unnecessary filled containers.
- Keep padding and alignment consistent.
- No decorative borders, framed panels, or rounded enclosing boxes.

### Inputs

- Clear labels and visible focus states.
- Consistent spacing and concise validation.
- Use a restrained input tone, caret, and focus emphasis where these sufficiently identify the editable area.
- No decorative effects that reduce legibility.

### Icons

- Minimal, recognizable shapes.
- Consistent stroke weight and visual size.
- Avoid mixing unrelated icon styles.
- Pair ambiguous icons with labels or tooltips.
- Icons should communicate actions, not merely fill space.

### Tooltips and Overlays

- Appear contextually.
- Never obscure critical content unnecessarily.
- Use concise language.
- Maintain visual consistency with the current theme.

## 8. Glass and Transparency

Glass is an optional visual treatment, not a universal requirement.

### Appropriate Use

- Floating navigation.
- Temporary overlays.
- Contextual controls.
- Select elevated interface layers.

### Rules

- Use restrained backdrop blur.
- Maintain sufficient opacity for readable content.
- Preserve clear visual relationships through tone and spacing.
- Avoid layering translucent surfaces excessively.
- Do not let background detail compete with foreground controls.
- Do not simulate glass in terminals using noisy characters or decorative frames.

Glass should provide dimensional separation without reducing clarity.

## 9. Lighting, Gradients, and Effects

Visual effects should be subtle and intentional.

### Lighting

- Restrained directional illumination.
- Subtle emphasis on active controls.
- Use only where it clarifies state or depth.
- No neon, exaggerated glow, or ornamental edge outlines.

### Gradients

- Prefer low-contrast tonal transitions.
- Use gradients to communicate depth or surface structure.
- Avoid unnecessary multicolor transitions.
- Preserve a clean, neutral foundation.

### Shadows

- Soft and restrained.
- Used to clarify elevation.
- Avoid large, muddy shadows.
- Maintain different shadow behavior for dark and light themes.

### Abstract Details

Controlled visual complexity is acceptable when it adds character without interfering with content. Avoid visual noise masquerading as sophistication. A reference-led build should not acquire new decorative details that the reference or requirements do not call for.

## 10. Motion and Animation

**Motion should communicate, not entertain by default.**

Use subtle animation where it improves feedback, orientation, continuity, or perceived responsiveness.

| Interaction | Suggested duration |
|---|---|
| Hover transition | 100–160 ms |
| Button press | 80–140 ms |
| Focus or selection | 120–200 ms |
| Panel transition | 180–280 ms |
| Overlay appearance | 150–250 ms |
| Layout transition | 200–350 ms |

These are starting points, not mandatory durations.

- Prefer opacity, transform, and restrained color transitions.
- Favor quick, smooth easing.
- Avoid unnecessary bouncing, elastic motion, and exaggerated spring effects.
- Use motion to preserve spatial continuity.
- Keep frequent interactions especially responsive.
- Never delay functionality for animation.
- Avoid continuous ambient animations unless they convey useful information or establish necessary context.
- Support reduced-motion preferences.
- Terminal redraws should communicate actual changes; do not continuously animate fixed sample data.

A static interface is preferable to motion without purpose.

## 11. Responsive Behavior

Responsive design should reorganize interfaces intelligently.

- Preserve hierarchy at every viewport size.
- Reflow content instead of simply shrinking it.
- Collapse secondary navigation when necessary.
- Keep important controls reachable.
- Preserve usable touch targets where touch applies.
- Avoid horizontal overflow except where intentional.
- Allow information density to adapt to available space.
- Maintain typography and spacing consistency across layouts.
- In terminals, respond to available columns and rows. Stack secondary regions or provide explicit scrolling instead of reducing the font size.

The layout should adapt to the user, not force the user to adapt to it.

## 12. Accessibility and Feedback

Visual refinement must not compromise usability.

- Maintain appropriate text contrast.
- Support keyboard navigation.
- Make focus and active states clearly visible.
- Provide sufficiently large interaction targets where pointing input applies.
- Do not rely solely on color to convey meaning.
- Make loading and processing states understandable.
- Provide clear error messages and recovery paths.
- Respect reduced-motion settings.
- Ensure interactive controls have accessible labels.
- Provide plain-text output when terminal styling or full-screen interaction is unavailable.

Accessibility should be built into the design system rather than added afterward.

## 13. Anti-Patterns

Avoid:

- Borders where styling and padding will suffice.
- Framed or rounded boxes used to separate ordinary content.
- Filled cards that merely disguise unnecessary framing.
- Terminal box-drawing decorations, ASCII frames, and decorative separators.
- Excessive rounded corners.
- Overly decorative glassmorphism.
- Neon glow on interactive elements.
- Heavy shadows and muddy contrast.
- Unnecessary gradients and excessive animations.
- Multiple competing accent colors.
- Cramped typography or shrinking text to compensate for poor layout.
- Unnecessary dropdown menus and modal overload.
- Excessive nested containers.
- Floating controls with no contextual relationship.
- Large empty regions without compositional purpose.
- Ambiguous icons without discoverable meaning.
- Decorative complexity that obstructs functionality.
- Inconsistent geometry or interaction behavior.
- Invented features, fabricated live readings, and assumptions about outside projects.

## 14. Implementation Tokens

```css
:root {
  --accent: #ff0077;
  --border-width: 0;
  --radius-none: 0px;
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --space-8: 32px;
  --space-12: 48px;
  --space-16: 64px;
  --motion-fast: 120ms;
  --motion-normal: 200ms;
  --motion-slow: 300ms;
  --ease-standard: cubic-bezier(0.2, 0, 0, 1);
  --font-ui: "Ubuntu", system-ui, sans-serif;
  --font-terminal: "Ubuntu Mono", "Cascadia Mono", Consolas, monospace;
}

[data-theme="dark"] {
  --bg: #111111;
  --bg-secondary: #1a1a1a;
  --surface: #222222;
  --surface-raised: #2b2b2b;
  --text-primary: #f5f5f5;
  --text-secondary: #a3a3a3;
}

[data-theme="light"] {
  --bg: #fafafa;
  --bg-secondary: #f1f1f1;
  --surface: #ffffff;
  --surface-raised: #eaeaea;
  --text-primary: #171717;
  --text-secondary: #626262;
}

@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

Tokens do not justify adding a visual element. Terminal implementations translate these intentions into character cells, supported colors, and terminal capabilities rather than copying CSS literally.

## 15. Design Decision Framework

Before introducing any component, color, animation, or effect, evaluate:

1. **Purpose:** What problem does this solve?
2. **Clarity:** Does it make the interface easier to understand?
3. **Hierarchy:** Does it help distinguish important information?
4. **Interaction:** Does it reduce friction or improve feedback?
5. **Consistency:** Does it fit the established visual language?
6. **Restraint:** Can the same result be achieved more simply?
7. **Framing:** Can spacing, padding, alignment, typography, or subtle tones provide the structure without a border or box?

If an element adds neither functional nor meaningful visual value, remove it.

## 16. Core Directive

Design clean, sharp, modern interfaces with intentional spacing, strong typography, intuitive interaction, and controlled dimensionality.

Support both dark and light themes with restrained, configurable accent colors.

Use subtle lighting, glass, gradients, and animation selectively where they improve the experience.

Prioritize clarity, direct interaction, and purposeful visual structure. Do not add borders or framed boxes where styling and padding will suffice.

**Minimal by default. Dimensional when useful. Animated when meaningful. Intuitive throughout. Border-free where spacing and styling suffice.**

## 17. Terminal / Console UI Direction

### Build the Actual Interface

- Deliver a running, interactive terminal interface when a console UI is requested. A static text dump, screenshot, website, or windowed GUI is not an interchangeable substitute.
- For a PowerShell implementation, provide runnable `.ps1` source and a clear launcher. Keep the files needed to run it together.
- Describe runtime requirements and the exact launch command.
- Preserve identifiable reference sections, placement, information hierarchy, labels, and workflows. Adapt framing to the no-borders rule.
- Do not import assumptions, features, branding, or architecture from outside projects.

### Composition

- Use a readable monospace font controlled by the user's terminal.
- Organize the header, navigation, primary work area, supporting status, and footer through alignment and whitespace.
- Use a consistent column grid and intentional blank rows.
- Do not draw boxes around sections, tables, tabs, inputs, help, or status areas.
- Keep supporting status quieter than the main task without making it illegible.
- Keep command hints close to their relevant content.
- Preserve information density without crowding or shrinking type.

### Navigation and Input

- Make keyboard controls visible and predictable.
- Clearly mark the active tab or selection using text and restrained contrast; color alone is insufficient.
- Scope commands to the relevant view. Do not let typing into an input activate global shortcuts.
- Provide an inline editing workflow with Enter to submit, Backspace to edit, and Escape to cancel.
- Validate before applying changes. Explain the error and preserve prior valid data.
- Keep help and quit behavior discoverable at all sizes.
- If mouse interaction is supported, provide an equivalent keyboard path.

### Resize and Rendering

- Measure available character columns and rows.
- Reflow or stack content when narrow. Keep secondary regions reachable through clear scrolling or navigation.
- Keep important values and commands readable. Do not silently hide essential information or clip it without a recovery path.
- Redraw on input, real data changes, and resize. Avoid unnecessary full-screen clearing or flicker.
- Use the alternate screen when available and restore cursor visibility, colors, and terminal state on normal exit or error.
- Offer a non-interactive plain-text rendering mode for redirected output and environments without full terminal support.
- Use neutral theme colors and a configurable accent. Do not require decorative Unicode to understand the interface.

### Data Honesty and Scope

- Label mock, demo, snapshot, and live data explicitly.
- Fixed sample readings must not appear to be live monitoring.
- Do not claim a service, security policy, process, or integration is active without observing it.
- A network plan must not imply routes, interfaces, listeners, or firewall settings were actually changed.
- Identify what persists locally and what exists only for the current session.
- Represent unavailable integrations honestly; do not create fake functionality to fill a tab.
- Do not add privileged operations or external connections that the requested workflow does not require.

### Verification and Delivery

- Exercise the actual interactive controls, not only a static render.
- Verify resize/reflow, input cancellation, validation errors, persistence, and clean exit as relevant.
- Check both themes and narrow and wide layouts.
- Document tested behavior and material limitations accurately.
- Link the runnable launcher, source, and updated standard clearly.
