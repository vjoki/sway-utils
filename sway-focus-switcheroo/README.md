# sway-focus-switcheroo
Switch to the previously focused window.

## Usage
1. Start the listener in the background, so that we can keep
   track of the previously focused window:
```
sway-focus-switcheroo listen &
```
NOTE: Expects `XDG_RUNTIME_DIR` to be defined.

2. Bind the command to switch to previously focused window in sway:
```
bindsym $mod+Tab exec sway-focus-switcheroo
```

## Other similar utils
- [i3-focus-last](https://github.com/lbonn/i3-focus-last)
- [sway-alttab](https://github.com/reisub0/sway-alttab)
