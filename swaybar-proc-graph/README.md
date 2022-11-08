# swaybar-proc-graph
Print out a CPU or memory usage graph using braille symbols, compatible with the
[Waybar](https://github.com/Alexays/Waybar) custom module. Optionally also
supports graphing Nvidia GPU and VRAM usage, using NVML library bindings.

Sample output:
```json
{"percentage": 34, "text": "⣀⣀⣀⣿⣶⣀⣀⣀⣀⣀", "tooltip": "CPU usage 33.84%"}
```

## Usage
Example configuration for Waybar:
```json
    "custom/cpugraph": {
        "format": " <span size='small' stretch='extracondensed'>{}</span>",
        "exec": "swaybar-proc-graph cpu",
        "return-type": "json"
    },
    "custom/memgraph": {
        "format": " <span size='small' stretch='extracondensed'>{}</span>",
        "exec": "swaybar-proc-graph -i 5 --len 5 memory",
        "return-type": "json"
    },
    "custom/gpugraph": {
        "format": " <span size='small' stretch='extracondensed'>{}</span>",
        "exec": "swaybar-proc-graph nvgpu",
        "return-type": "json"
    },
    "custom/vramgraph": {
        "format": " <span size='small' stretch='extracondensed'>{}</span>",
        "exec": "swaybar-proc-graph -i 5 nvvram --gpu-index 2",
        "return-type": "json"
    },
```
