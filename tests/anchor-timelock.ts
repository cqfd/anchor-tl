import * as anchor from '@project-serum/anchor';
import * as spl from '@solana/spl-token';
import { Program } from '@project-serum/anchor';
import { AnchorTimelock } from '../target/types/anchor_timelock';
import { NodeWallet } from '@project-serum/anchor/dist/cjs/provider';
import * as assert from 'assert';


// Configure the client to use the local cluster.
anchor.setProvider(anchor.Provider.env());

const program = anchor.workspace.AnchorTimelock as Program<AnchorTimelock>;

describe('anchor-timelock', () => {

  let mint: spl.Token;
  let walletsTokens: anchor.web3.PublicKey;

  before(async () => {
    mint = await spl.Token.createMint(
      program.provider.connection,
      (program.provider.wallet as NodeWallet).payer,
      program.provider.wallet.publicKey,
      program.provider.wallet.publicKey,
      0,
      spl.TOKEN_PROGRAM_ID
    );
    const tx = new anchor.web3.Transaction();

    const walletsTokensKeypair = anchor.web3.Keypair.generate();
    walletsTokens = walletsTokensKeypair.publicKey;

    tx.add(
      anchor.web3.SystemProgram.createAccount({
        fromPubkey: program.provider.wallet.publicKey,
        newAccountPubkey: walletsTokens,
        space: 165,
        lamports: await program.provider.connection.getMinimumBalanceForRentExemption(165),
        programId: spl.TOKEN_PROGRAM_ID
      }),
      spl.Token.createInitAccountInstruction(
        spl.TOKEN_PROGRAM_ID,
        mint.publicKey,
        walletsTokens,
        program.provider.wallet.publicKey
      )
    );
    tx.recentBlockhash = (await program.provider.connection.getRecentBlockhash()).blockhash;
    const sig = await program.provider.send(tx, [walletsTokensKeypair]);
    program.provider.connection.confirmTransaction(
      sig,
      "confirmed"
    );

    let walletsTokensAccount = await program.provider.connection.getAccountInfo(walletsTokens);

    await mint.mintTo(
      walletsTokens,
      program.provider.wallet.publicKey,
      [],
      1000
    );

  });

  it('can lock and eventually unlock tokens', async () => {
    const receiver = anchor.web3.Keypair.generate();
    const receiverTokens = await spl.Token.getAssociatedTokenAddress(
      spl.ASSOCIATED_TOKEN_PROGRAM_ID,
      spl.TOKEN_PROGRAM_ID,
      mint.publicKey,
      receiver.publicKey,
    );
    const createAssociatedTokenAccountIx = spl.Token.createAssociatedTokenAccountInstruction(
      spl.ASSOCIATED_TOKEN_PROGRAM_ID,
      spl.TOKEN_PROGRAM_ID,
      mint.publicKey,
      receiverTokens,
      receiver.publicKey,
      program.provider.wallet.publicKey
    );

    const [timelock, bump] = await anchor.web3.PublicKey.findProgramAddress([receiver.publicKey.toBuffer()], program.programId);
    const lockDuration = 50;
    await program.rpc.lock(bump, new anchor.BN(lockDuration), {
      accounts: {
        timelock: timelock,
        initializer: program.provider.wallet.publicKey,
        receiver: receiver.publicKey,
        receiverTokens: receiverTokens,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: spl.TOKEN_PROGRAM_ID,
        tokensToLock: walletsTokens
      },
      instructions: [
        createAssociatedTokenAccountIx
      ]
    });

    let lockAccount = await program.account.timelock.fetch(timelock);

    // await sleep(lockDuration);
    await program.rpc.unlock({
      accounts: {
        receiver: receiver.publicKey,
        receiverTokens: receiverTokens,
        timelock: timelock,
        timelockedTokens: walletsTokens,
        tokenProgram: spl.TOKEN_PROGRAM_ID
      }
    });

    let receiverTokensAccount = await mint.getAccountInfo(receiverTokens);
    assert.equal(1000, receiverTokensAccount.amount);
  });
});

const sleep = seconds => new Promise(awaken => setTimeout(awaken, seconds * 1000));

async function fetchTokenAccount(address: anchor.web3.PublicKey): Promise<spl.AccountInfo> {
  let tokenAccountInfo = await program.provider.connection.getAccountInfo(address);
  return spl.AccountLayout.decode(tokenAccountInfo.data);
}