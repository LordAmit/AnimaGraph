# Using Animagraph from an assistant (MCP)

Assistants that support MCP, such as Claude Code and Claude Desktop, can
edit and save photos with Animagraph. Everything stays on this computer;
nothing is uploaded by Animagraph.

There are two modes, chosen automatically for each request:

- **Live:** while the Animagraph window is open, the assistant works in it.
  Photos it opens appear in the Files panel, and each change shows on the
  canvas as it happens. Undo works on its colour and crop changes as on
  your own. `list_open_images` lists the window's files.
- **Background:** when the window is closed (or live access is switched
  off), the assistant uses its own copy of Animagraph with no window.

Live access can be switched off in **View → Assistant access (MCP)…**. The
window and the assistant talk through a private connection that only your
user account can open.

## Connect

Open Animagraph, then **View → Assistant access (MCP)…**. The window shows
the exact lines for this computer, with Copy buttons.

- **Claude Code:** run the command shown, which looks like
  ```
  claude mcp add animagraph -- "/Applications/Animagraph.app/Contents/MacOS/animagraph" --mcp
  ```
- **Claude Desktop:** Settings → Developer → Edit Config, and add the
  `animagraph` entry shown to `claude_desktop_config.json`, then restart
  Claude Desktop.

## Folders

The assistant can only open photos from, and save into, the allowed
folders. By default these are Pictures, Desktop and Downloads. Add or
remove folders in the same window. A new list is used the next time the
assistant starts Animagraph.

## What the assistant can do

| Tool | What it does |
|---|---|
| `allowed_folders` | Lists the folders it may use |
| `open_image` | Opens a photo (JPEG, PNG, HEIC, RAW) and returns an id |
| `list_open_images` | Photos open in this session |
| `list_styles` | Names of your imported Lightroom presets, the built-in looks and the film simulations |
| `apply_style` | Applies one of those by name (film simulations take a strength) |
| `set_adjustments` | Exposure, contrast, highlights, shadows, whites, blacks, temperature, tint, vibrance, saturation, texture, clarity, dehaze, sharpening, fade, vignette, grain, S-curve, in the same units as the app; also the tone curve (all, red, green, blue), HSL per colour band, colour grading wheels and finer details (vignette shape, grain size, split toning), in the layout `get_settings` reports |
| `reset_adjustments` | Removes all colour changes |
| `set_framing` | Quarter turns, straighten, crop ratio, border (style, thickness, colour, rounded corners, corner mounts, Polaroid strip) and caption (text, font, size, colour, place, alignment, on the picture or in the border) |
| `get_settings` | All current settings of a photo: sliders, tone curve, HSL, colour grading, details and framing |
| `preview` | A small picture of the result for the assistant to look at |
| `save_image` | Saves the finished photo as `.heic`, `.jpg` or `.png`, at full size or a chosen size |
| `close_image` | Closes a photo |

Not in this version: animation (making loops), video files and layouts.

## For testing by hand

`animagraph --mcp` reads one JSON-RPC message per line on standard input
and answers on standard output; status messages go to standard error.
