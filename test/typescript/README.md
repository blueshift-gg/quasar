# @blueshift-gg/quasar-test

The TypeScript `quasar-test` package is the Kit and Web3.js sibling of the Rust
crate. Both expose the same test model: an isolated `Test` world, composable
fixtures, generated instructions, and structured outcomes.

```bash
npm install --save-dev @blueshift-gg/quasar-test @solana/kit
```

```ts
import { Test, wallet } from "@blueshift-gg/quasar-test/kit";
import {
  PROGRAM_ADDRESS,
  VaultClient,
  findVaultAddress,
} from "./client/index.js";

using test = await Test.load(PROGRAM_ADDRESS, "target/deploy/vault.so");
const signer = await test.add(wallet());
const vault = await findVaultAddress(signer);
const client = new VaultClient();
test
  .send(await client.createDepositInstruction({ signer, lamports: 1_000n }))
  .succeeds()
  .hasLamports(vault, 1_000n)
  .cuAtMost(10_000n);
```

Use `@blueshift-gg/quasar-test/web3.js` for the same API with Web3.js address,
account, and instruction types. Fixture addresses are deterministic and match
between the Kit, Web3.js, and Rust harnesses.

Built-in fixtures are `wallet`, `mint`, `tokenAccount`,
`associatedTokenAccount`, and `program`. Application fixtures are ordinary
objects implementing `Fixture`; `test.add` is the only composition primitive.
The canonical generated instruction infers PDAs and ATAs. Generated
`create{Name}InstructionRaw` methods make those addresses explicit only for
adversarial tests.

`send`, `sendAll`, and `simulate` return `Outcome`. Its stable assertions are
`succeeds`, `fails`, `failsWith`, `cuAtMost`, `hasLamports`, `hasTokens`,
`hasSupply`, and `isClosed`. `accountAs`, `events`, and `returnValue` accept
generated decoders directly; `accountChanges` reports writable before/after
state in instruction order.

Pass `{ computeUnitLimit: 200_000n }` as the third `Test` constructor argument
or `Test.load` option to set the same per-transaction ceiling as Rust's
`Test::builder(...).compute_unit_limit(...)`.

`Test.load(PROGRAM_ADDRESS)` reads `QUASAR_PROGRAM_PATH`, which `quasar test`
sets after building the program. Passing the ELF explicitly keeps direct test
runner invocation straightforward.
