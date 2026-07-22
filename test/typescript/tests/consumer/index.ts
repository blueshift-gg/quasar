import { getAddressDecoder, type Address as KitAddress } from "@solana/kit";
import { Address as Web3Address } from "@solana/web3.js";
import {
  Test as KitTest,
  wallet as kitWallet,
  type ProgramError as KitProgramError,
} from "@blueshift-gg/quasar-test/kit";
import {
  Test as Web3Test,
  wallet as web3Wallet,
  type ProgramError as Web3ProgramError,
} from "@blueshift-gg/quasar-test/web3.js";
import {
  PROGRAM_ADDRESS,
  QuasarVaultClient as KitVaultClient,
} from "../fixtures/vault/clients/kit/quasar-vault/client.js";
import { QuasarVaultClient as Web3VaultClient } from "../fixtures/vault/clients/web3/quasar-vault/client.js";

const bytes = new Uint8Array(32).fill(1);
const kitAddress = getAddressDecoder().decode(bytes) as KitAddress;
const web3Address = new Web3Address(bytes);
const kitError: KitProgramError = { type: "Custom", code: 3002 };
const web3Error: Web3ProgramError = kitError;

async function compilePublicContract() {
  using kit = new KitTest(PROGRAM_ADDRESS, new Uint8Array(), {
    computeUnitLimit: 200_000n,
  });
  using web3 = new Web3Test(Web3VaultClient.programId, new Uint8Array(), {
    computeUnitLimit: 200_000n,
  });
  const kitUser = await kit.add(kitWallet({ address: kitAddress }));
  const web3User = await web3.add(web3Wallet({ address: web3Address }));
  kit.send(
    await new KitVaultClient().createDepositInstruction({
      user: kitUser,
      amount: 1n,
    }),
  ).fails(kitError);
  web3
    .send(
      await new Web3VaultClient().createDepositInstruction({
        user: web3User,
        amount: 1n,
      }),
    )
    .fails(web3Error);
}

void compilePublicContract;
