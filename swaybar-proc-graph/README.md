# swaybar-proc-graph
Print out a CPU or memory usage graph using braille symbols, compatible with the
[Waybar](https://github.com/Alexays/Waybar) custom module.

Sample output:
```json
{"percentage": 34, "text": "⣀⣀⣀⣿⣶⣀⣀⣀⣀⣀", "tooltip": "CPU usage 33.84%"}
```

## Usage
Example configuration for Waybar:
```json
    "custom/cpugraph": {
        "format": " <span size='small' stretch='extracondensed'>{}</span>",
        "exec": "swaybar-proc-graph",
        "return-type": "json"
    },
    "custom/memgraph": {
        "format": " <span size='small' stretch='extracondensed'>{}</span>",
        "exec": "swaybar-proc-graph --memory",
        "return-type": "json"
    },
```
