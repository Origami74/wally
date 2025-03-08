import {CashuMint, CashuWallet, type Proof, getEncodedTokenV4, getDecodedToken} from "@cashu/cashu-ts";
import { getAmount, toCashuToken } from "../util/cashu";

export interface IWallet {
  add(proofs: Proof[]): Promise<number>;
  withdrawAll(pubkey?: string): Promise<Proof[]>;
  getBalance(): number;
}

export class Wallet implements IWallet {
  private nutSack: Proof[] = [];

  private nostrPrivateKey: string;
  private mint: CashuMint;
  private cashuWallet: CashuWallet;

  constructor(
      mintUrl: string,
      nostrPrivateKey: string,
  ) {

    this.nostrPrivateKey = nostrPrivateKey;
    this.mint = new CashuMint(mintUrl);
    this.cashuWallet = new CashuWallet(this.mint);
  }
  /**
   * Redeems tokens and adds them to wallet.
   * Returns total amount in wallet
   */
  public async add(proofs: Proof[]): Promise<number> {

    const token = getEncodedTokenV4({ mint: this.mint.mintUrl, proofs: proofs });
    const received = await this.cashuWallet.receive(token);

    this.nutSack = [...this.nutSack, ...received];

    const receivedAmount = getAmount(proofs);
    const nutSackAmount = getAmount(this.nutSack);
    console.log(`Received ${receivedAmount} sats, wallet now contains ${nutSackAmount} sats`);

    return nutSackAmount;
  }

  /**
   * If a pubkey is passed, the tokens will be locked to that pubkey.
   */
  public async withdrawAll(pubkey: string | undefined): Promise<Proof[]> {
    const nuts = this.nutSack;
    this.nutSack = [];

    const removedAmount = getAmount(nuts);
    const nutSackAmount = getAmount(this.nutSack);
    console.log(`Removed ${removedAmount} sats, wallet now contains ${nutSackAmount} sats`);

    const { keep, send } = await this.cashuWallet.send(removedAmount, nuts, {privkey: this.nostrPrivateKey});
    return send;
  }

  public getBalance = (): number => getAmount(this.nutSack);

  public async collectToken(token: string): Promise<number> {
    try{
      const proofs = getDecodedToken(token).proofs;

      await this.collectProofs(proofs);
      return getAmount(proofs)
    } catch (e) {
      console.error("Payment failed: Error redeeming cashu tokens", e);
      throw new Error("Payment failed");
    }
  }

  public async collectProofs(proofs: Proof[]): Promise<number> {
    try {
      await this.add(proofs);
      return getAmount(proofs);
    } catch (e) {
      console.error("Payment failed: Error redeeming cashu tokens", e);
      throw new Error("Payment failed");
    }
  }
}
