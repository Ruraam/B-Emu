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

## Default mapping

| Controller input | B-Emu action |
| --- | --- |
| D-pad up / left stick up | Navigate up |
| D-pad down / left stick down | Navigate down |
| D-pad left / left stick left | Navigate left |
| D-pad right / left stick right | Navigate right |
| South face button (A on common Xbox layouts) | Confirm |
| East face button (B on common Xbox layouts) | Back to Home |

Analog directions use a 0.65 activation threshold and 0.35 return threshold
to reduce jitter. Direction changes are emitted once per threshold transition,
not continuously while an axis is held.

The current application shell has a vertical sidebar. Therefore left/right
actions are normalized now but intentionally do not alter the sidebar state;
they can be consumed by later screens with horizontal navigation.

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
