# locale1-xkb-config-rs
Source Sway keyboard config from the [systemd-localed D-Bus
interface](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.locale1.html).
Useful when keyboard settings are configured using `localectl set-x11-keymap`.

## Usage

1. Build the binary using `cargo build --release`, and place it somewhere
   suitable (ex. `/usr/local/bin`).
2. Add `exec /path/to/locale1-xkb-config-rs --oneshot` to Sway config.

Remove the `--oneshot` flag if you want localectl changes to take effect
immediately on change, rather than get applied only on Sway reload.


## Acknowledgements
- [sway-systemd](https://github.com/alebastr/sway-systemd) - The original source
  implementation of this port.
