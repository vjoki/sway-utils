# sway-window-nav
Cycle focus, or move focused window, forwards/backwards through _all_ windows in
the currently focused workspace in a hopefully deterministic order.

- Navigation using two key bindings (next or previous), relative to focused window.
- Move focused window by swapping position with the next or previous container.
- Cycle through tabbed, stacked and floating windows.

## Traversal order
Order generally follows the node tree, but when in doubt goes from left to
right, top to bottom.

```
.---------------.
|   1   |   4   |
|       |       |
|-------|       |
|   2   |       |
|-------|       |
|   3   |       |
|       |       |
._______|_______.
```

```
.---------------.
|   | 2 | 3 |   |
| 1 |___|___| 5 |
|   |   4   |   |
|---------------|
|       6       |
._______________.
```

## Usage
```
bindsym $mod+j exec sway-window-nav focus next
bindsym $mod+k exec sway-window-nav focus prev
bindsym $mod+Shift+j exec sway-window-nav move next
bindsym $mod+Shift+k exec sway-window-nav move prev
```
