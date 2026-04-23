---
name: example-skill
description: "A demonstration skill that reverses the input text. Shows the stdin/stdout JSON protocol used by all Cascade Agent skills."
version: "0.1.0"
tags: [demo, example]
input_format:
  content_type: "json"
  schema:
    type: object
    properties:
      text:
        type: string
        description: "The text to reverse."
    required:
      - text
---

# Example Skill

This skill demonstrates the Cascade Agent skill protocol.

## Protocol

1. The agent sends a JSON object via **stdin** matching the schema above.
2. This script processes the input and writes a JSON result to **stdout**.
3. The result must follow the `ToolResult` format:

```json
{
  "status": "success",
  "data": "...",
  "error": null
}
```

## Usage

The agent will automatically discover this skill and register it as a tool
called `example-skill`. You can invoke it by asking the agent to reverse text.
