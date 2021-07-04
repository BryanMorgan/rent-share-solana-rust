# Rent Share 
[![Build](https://github.com/BryanMorgan/rent-share-solana-rust/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/BryanMorgan/rent-share-solana-rust/actions/workflows/build.yml)

Manage rental agreements between two parties by capturing the agreement and rental payments on the Solana blockchain using a Solana Smart Contract (Program)

# Instructions
## Building
```bash
cargo build-bpf
```

## Deploy to devnet
### Validate Solana Configuration
Make sure your solana environment is pointing to devnet

```bash
solana config set --url https://api.devnet.solana.com
solana config get
```

### Deploy BPF Program
```bash
solana program deploy ./target/deploy/rentshare.so
```
## Debugging
Debug messages are written to the solana logs with a `[RentShare]` prefix so you can filter messages using:

```bash
solana logs | grep "\[RentShare\]"
```

## Program Call Examples
The examples below show how to call the program with 2 instructions using the `@solana/web3.js` library. 

### 1. First, Create the Rent Agreement Account
Using an externally created "Company" Account to fund the transactions (try [Sollet](https://www.sollet.io/)), create a new rental agreement using a unique `seed` and the program ID from the Rust BPF output:

```javascript
  const rentAgreementPublicKey = await PublicKey.createWithSeed(
    rentCompanyAccountOwner.publicKey,
    seed,
    programId,
  );

  const lamports = await connection.getMinimumBalanceForRentExemption(
    RENT_AGREEMENT_SCHEMA_SIZE, // Currently 90
  );

  const transaction = new Transaction().add(
    SystemProgram.createAccountWithSeed({
      fromPubkey: accountOwner.publicKey,
      basePubkey: accountOwner.publicKey,
      seed,
      newAccountPubkey: rentAgreementPublicKey,
      lamports,
      space: RENT_AGREEMENT_SCHEMA_SIZE,
      programId,
    }),
  );
  await sendAndConfirmTransaction(connection, transaction, [accountOwner]);
```
### 2. Initialize Rent Agreement Account
Initialize the rent agreement account data using the rental terms - duration, rent amount, and deposit amount - by invoking the program with instruction `0`.
This will also record the payee (owner) and payer (renter) public keys to ensure future transactions are only between these two parties.

```javascript
  const instruction = 0;

  const transactionInstruction = new TransactionInstruction({
    keys: [
      { pubkey: rentAgreementPublicKey, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    ],
    programId,
    data: Buffer.from(Uint8Array.of(instruction,
      ...Array.from(payeePublicKey.toBytes()),
      ...Array.from(payerPublicKey.toBytes()),
      ...new BN(deposit).toArray("le", 8),
      ...new BN(rentAmount).toArray("le", 8),
      ...new BN(duration).toArray("le", 8),
      ...new BN(durationUnit).toArray("le", 1),
    ))
  })

await sendAndConfirmTransaction(
    connection,
    new Transaction().add(transactionInstruction),
    [rentCompanyAccountOwner],
  );
```

### 3. Pay Rent
Transfer lamports from the payer (renter) to the payee (owner) for rent due using instruction `1`. This will decrement the `remaining_payments` saved
in the rental agreement account data. They payer account must sign the transaction to tranfer funds to the payee.

```javascript
  const instruction = 1;

  const transactionInstruction = new TransactionInstruction({
    keys: [
      { pubkey: rentAgreementPublicKey, isSigner: false, isWritable: true },
      { pubkey: payeePrivateKey.publicKey, isSigner: false, isWritable: true },
      { pubkey: payerPrivateKey.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: true },
    ],
    programId,
    data: Buffer.from(Uint8Array.of(instruction,
      ...new BN(rentAmount).toArray("le", 8),
    ))
  })

  await sendAndConfirmTransaction(
    connection,
    new Transaction().add(transactionInstruction),
    [payerPrivateKey],
  );
```

