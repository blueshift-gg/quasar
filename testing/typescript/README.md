# @blueshift-gg/quasar-test

`quasar-test` is the TypeScript test SDK for Quasar programs. It wraps
QuasarSVM with a persistent account world, fixture helpers, and fluent result
assertions. Both web3.js and Solana Kit use the same API.

```typescript
import { QuasarTest } from "@blueshift-gg/quasar-test/kit";
import { PROGRAM_ADDRESS, QuasarVaultClient } from "../target/client/typescript/quasar-vault/kit.js";

const client = new QuasarVaultClient();
const q = await QuasarTest.load(PROGRAM_ADDRESS);
const authority = await q.actor();

const result = await q.send(client.createInitializeInstruction({ authority }));
result.succeeds().cuBelow(10_000);
```

Use `@blueshift-gg/quasar-test/web3.js` for the web3.js adapter. The underlying
`q.svm` remains public for runtime-specific setup.
