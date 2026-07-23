# @blueshift-gg/quasar-test

The TypeScript `quasar-test` package is the Kit and Web3.js sibling of the Rust
crate. Both expose the same test model: an isolated `Test` world, composable
fixtures, generated instructions, and structured outcomes.

```bash
npm install --save-dev @blueshift-gg/quasar-test @solana/kit
```

```ts
import { Test, wallet } from "@blueshift-gg/quasar-test/kit";
import { PROGRAM_ADDRESS, VaultClient } from "./client/index.js";

using test = await Test.load(PROGRAM_ADDRESS, "target/deploy/vault.so");
const user = await test.add(wallet({ fund: 1_000_000n }));
const client = new VaultClient();
const deposit = await client.createDepositInstruction({ user, amount: 1_000n });
test
  .send(deposit)
  .succeeds()
  .hasLamports(deposit.vaultAddress, 1_000n)
  .cuAtMost(10_000n);
```

Actors are `wallet()` fixtures: `test.add(wallet({ fund }))` installs a funded
account and returns its address. A transaction may still name a signer the world
never installed — a read-only co-signer such as a multisig member is auto-funded
on `send` — but an account that pays or is created is world state, so a payer
needs a `wallet()` and an init target enters empty. This mirrors the Rust
harness exactly.

Use `@blueshift-gg/quasar-test/web3.js` for the same API with Web3.js address,
account, and instruction types. Fixture addresses are deterministic and match
between the Kit, Web3.js, and Rust harnesses.

Built-in fixtures are `wallet`, `mint`, `tokenAccount`,
`associatedTokenAccount`, and `program`. Application fixtures are ordinary
objects implementing `Fixture`; `test.add` is the only composition primitive.
The canonical generated instruction infers PDAs and ATAs and exposes each
derived address as a `{field}Address` property on the returned instruction, so
tests name it without calling `find{Name}Address` by hand. Generated
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
