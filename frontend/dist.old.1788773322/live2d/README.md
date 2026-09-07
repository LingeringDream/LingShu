# Live2D Model Directory

Drop your Live2D Cubism model files here. The pet window will load
`model.model3.json` automatically.

## Required files

```
public/live2d/
├── model.model3.json    ← model metadata (entry point)
├── *.moc3               ← model data
├── textures/            ← texture images (.png)
├── motions/             ← motion files (.motion3.json, optional)
└── expressions/         ← expression files (.exp3.json, optional)
```

## Getting a model

1. **Official samples**: https://www.live2d.com/en/download/sample-data/
2. **Create your own**: Live2D Cubism Editor
3. **Community models**: Various free/paid models online

## How it works

- The pet checks for `model.model3.json` on startup
- If found → renders the Live2D model with animations
- If not found → falls back to an animated CSS character

## Cubism Core

The Cubism 5 Core (`live2dcubismcore.min.js`) is bundled from
`@hazart-pkg/live2d-core` and lives in this directory.
