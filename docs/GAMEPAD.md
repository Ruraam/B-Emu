# Gamepad input

B-Emu reads Linux gamepads through GilRs and the Linux evdev/udev stack. The
input backend is independent from Slint: it translates controller-specific
events into the normalized actions in `input::GamepadAction`.

## Runtime behavior

- Controller discovery and hotplug events are handled by GilRs.
- Polling runs on the dedicated `b-emu-gamepad` thread; it never blocks the
  Slint UI event loop.
- UI updates are delivered with Slint's
  `Weak::upgrade_in_event_loop` API.
- The worker is stopped and joined when the main window's event loop returns.
- If the gamepad subsystem cannot initialize, B-Emu logs a diagnostic and
  continues with keyboard and mouse input.
- Controller disconnects are logged and do not terminate the application.
- When several controllers are connected, the first one owns navigation.
  Actions from other controllers are ignored until it disconnects, so two
  players cannot fight over the same focus state.

## Default mapping

| Controller input | B-Emu action |
| --- | --- |
| D-pad up / left stick up | Navigate up |
| D-pad down / left stick down | Navigate down |
| D-pad left / left stick left | Navigate left |
| D-pad right / left stick right | Navigate right |
| South face button (A on common Xbox layouts) | Confirm |
| East face button (B on common Xbox layouts) | Back to Home |
| West face button (X on common Xbox layouts) | Toggle favorite on the selected game |
| North face button (Y on common Xbox layouts) | Mark the selected game as played |

Analog directions use a 0.65 activation threshold and 0.35 return threshold
to reduce jitter. Direction changes are emitted once per threshold transition,
not continuously while an axis is held.

## Focus model

The shell has two focus zones: the sidebar and the content area.

- In the sidebar, up/down moves the highlighted page and Confirm opens it.
  Back returns to Home. Left/right actions are normalized but intentionally do
  not alter the vertical sidebar.
- In a screen, up/down moves the highlighted item (wrapping around), Confirm
  selects it, and Back returns focus to the sidebar without leaving the page.
  Settings toggles the focused row on Confirm.
- Moving between zones also moves Slint's real keyboard focus, so mouse,
  keyboard and gamepad input all drive the same state.

The pure transition function lives in `input::navigation` and is unit-tested
without a window. The gamepad worker forwards normalized actions to Slint,
which calls back into that same function through one shared navigation path.

## Controller profiles

Controllers whose buttons differ from the default mapping can be remapped
through `controller_profiles` in `config.json`. A profile matches a device by
name substring or 16-byte GUID and maps default action keys (`up`, `down`,
`left`, `right`, `confirm`, `back`, `favorite`, `played`) to other actions:

```json
{
  "controller_profiles": [
    {
      "id": "inverted-ab",
      "name": "Inverted A/B",
      "device_name_contains": "My Arcade Pad",
      "remap": { "confirm": "back", "back": "confirm" }
    }
  ]
}
```

The first matching profile wins. Profiles are applied when events are
normalized, before they reach the navigation state, so the rest of the
application stays controller-model independent. Changes are read at startup;
restart B-Emu after editing the file.

## Permissions and rootless operation

B-Emu does not request root privileges, change device-node permissions, add
the user to the `input` group, or install a privileged helper. Access depends
on the active Linux seat/session policy (commonly udev `uaccess` ACLs).

If the session does not grant access to a controller, the gamepad subsystem
may be unavailable. B-Emu reports that condition and remains usable with
keyboard and mouse. Users should fix access through their desktop/seat
configuration rather than running B-Emu as root.

## Validation

The pure sidebar state transitions are unit-tested without requiring a
physical controller or a running window server. Hardware validation remains
a separate manual check on a Linux Wayland session with a controller attached.
