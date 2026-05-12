# @squick/node

Node.js bindings for [Squick](../../readme.md).

```js
const { scan } = require('@squick/node');
const result = scan(process.cwd());
console.log(result.markdown);
```

Build (requires Rust toolchain and `@napi-rs/cli`):

```bash
npm install
npm run build
```

Licensed under the Apache License 2.0.
Copyright 2026 Horizon LLC.
