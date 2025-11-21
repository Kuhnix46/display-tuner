# Display Tuner

A small Windows utility to list and tune display resolution and scaling.

## Usage

Display help

```
display-tuner -h
```

List displays

```
display-tuner list
```

Set configuration

```
# Apply to a specific display by source id
display-tuner set --id 123 --width 1920 --height 1080 --scaling 125

# Apply only scaling to all displays
display-tuner set --all --scaling-only --scaling 175
```

Notes

- The `--id` value is the source id printed by `list`.
- If neither `--id` nor `--all` are provided, the command will apply to all displays and log a warning.
