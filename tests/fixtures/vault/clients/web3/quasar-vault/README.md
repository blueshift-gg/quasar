# quasar_vault @solana/web3.js client

Generated from the `quasar_vault` IDL (version 0.1.0). Program address:

```
33333333333333333333333333333333333333333333
```

## Usage

```ts
import { quasar_vaultClient } from "./client.js";

const client = new quasar_vaultClient();
const instruction = await client.create<Name>Instruction({ /* inputs */ });
```

## What the builders resolve

Every address the client can work out itself — program-derived addresses, associated token accounts, well-known programs, and addresses already carried by an instruction argument — is computed for you. Callers pass only what cannot be derived. Each builder also accepts overrides so any resolved address can be replaced.

## Regenerating

This file is generated. Re-run the generator instead of editing it:

```sh
quasar client <idl-path>
```
