# Keybindings

Open **Keybindings** in the navigation, press **Ctrl+Shift+K**, or launch with `--keybindings`. Edit a shortcut and choose **Save keybindings** to persist it beside your recent-location file as `keybindings.json`. Blank fields disable actions. Duplicate shortcuts and conflicts with terminal clipboard/clear commands are rejected.

| Action | Default |
|---|---|
| New window | Ctrl+Shift+N |
| Toggle Grid / Splits | Ctrl+Shift+Space |
| Show Grid | Ctrl+Alt+1 |
| Show Splits | Ctrl+Alt+2 |
| Split right | Ctrl+Shift+D |
| Split below | Ctrl+Alt+D |
| Close active pane | Ctrl+Shift+W |
| Restart active pane | Ctrl+Shift+R |
| Next / previous pane | Ctrl+Alt+Right / Left |
| Detach active pane | Ctrl+Shift+E |
| Dock active pane | Ctrl+Shift+B |
| Focus terminal canvas | Ctrl+Shift+T |
| Keybindings | Ctrl+Shift+K |
| Dashboard | Ctrl+Shift+H |
| Copy selection or visible terminal screen | Ctrl+Shift+C |

Terminal actions apply while the Terminal page or a detached terminal window is active. Navigation shortcuts and new-window shortcuts are also available from detached windows. Application shortcuts require Ctrl, Cmd, or Alt, leaving ordinary terminal keys available to the shell. `Cmd` can be used for custom macOS shortcuts; defaults above use Ctrl explicitly.

## Terminal canvas

- Tab, Shift+Tab, Escape, arrows, function keys, and Ctrl+L go to the terminal while its canvas has focus.
- Ctrl+C copies an active selection and clears it. With no selection it interrupts the running command. Ctrl+X sends its control byte when there is no selection. On macOS, Cmd+C remains a clipboard action.
- Ctrl+V / platform Paste pastes once. Escape bytes are removed from pasted text so it cannot escape bracketed paste.
- Shift+PageUp / PageDown scrolls through output. Hold Shift to select text while an application requests mouse reporting.
- Ctrl+click opens HTTP(S) links or recognized document paths. Executables, scripts, and unrecognized file types are blocked from implicit launch.

## Completion and code editing

- Ctrl+Space requests completion. Up/Down selects a candidate, Escape dismisses, and Tab accepts the selected candidate.
- In a terminal session, normal command editing, Up/Down history, Tab completion and Enter go to the real shell. Ctrl+Space opens inline suggestions for the tracked line; accepting a suggestion edits that same terminal command. No separate command field is used.
- In the code editor, Tab indents when completion is closed. Ctrl+S / platform Cmd+S saves. Unsaved text is protected when opening another file or quitting.

## Designer

Click the canvas to focus it. Tab cycles top-level objects; arrows nudge placed objects by the selected grid/subgrid step. Shift+arrows resize them. Delete removes the selected top-level object. Ctrl+Z / platform Cmd+Z undoes; Ctrl+Shift+Z / Cmd+Shift+Z redoes. Nested flow objects are editable through the properties inspector. Grid and subgrid settings also apply to pointer placement and sizing.
