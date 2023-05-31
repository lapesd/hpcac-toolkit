# fresh project

### Usage

Start the project:

```
deno task start
```

This will watch the project directory and restart as necessary.

### Deno and VS Code

Add the following to your `.vscode/settings.json` file:

```json
{
  "deno.enable": true,
  "deno.importMap": "./hpcc_ui/import_map.json"
}
```
