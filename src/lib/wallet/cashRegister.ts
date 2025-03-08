import { inject, injectable } from "tsyringe";
import {
  getDecodedToken,
  PaymentRequest,
  PaymentRequestTransport,
  PaymentRequestTransportType,
  Proof
} from "@cashu/cashu-ts";

import type { IWallet } from "./wallet.ts"
import { Wallet } from "./wallet.ts";
import { MINT_URL, PRICE_PER_SEC, PRICE_UNIT, PROFIT_PAYOUT_THRESHOLD, PROFITS_PUBKEY } from "../utils/env.ts";
import { randomUUID } from "node:crypto";
import {EventPublisher, type IEventPublisher} from "../publisher/EventPublisher.ts";
import pino from "npm:pino@9.4.0";

export interface ICashRegister {
  createPaymentRequest(): PaymentRequest;
  collectToken(token: String): Promise<number>;
  collectPayment(proofs: Proof[]): Promise<number>;
  payoutOwner(ignoreThreshold: boolean): Promise<void>;
}

@injectable()
export class CashRegister implements ICashRegister {
  private profitsPubkey: string = PROFITS_PUBKEY;
  private profitsPayoutThreshold: number = PROFIT_PAYOUT_THRESHOLD;

  private wallet: IWallet;
  private eventPublisher: IEventPublisher;

  constructor(
      @inject("Logger") private logger: pino.Logger,
      @inject(Wallet.name) wallet: IWallet,
      @inject(EventPublisher.name) eventPublisher: IEventPublisher) {
    this.wallet = wallet;
    this.eventPublisher = eventPublisher;
  }



  public async payoutOwner(ignoreThreshold: boolean = false) {
    const balance = this.wallet.getBalance();
    if (!ignoreThreshold && balance <= this.profitsPayoutThreshold) {
      this.logger.warn(
        `Balance of ${balance} not enough for payout threshold of ${this.profitsPayoutThreshold}, skipping payout...`,
      );
      return;
    }

    const nuts = await this.wallet.withdrawAll();

    try {
      const cashuToken = toCashuToken(nuts, this.wallet.mintUrl);
      await this.eventPublisher.publishDM(
        this.profitsPubkey,
        `Here's your profits from your relay proxying service. At ${new Date().toUTCString()}.\n ${cashuToken}`,
      );
    } catch (e) {
      console.error("Failed to forward payment in dm", e);

      // NOTE: this will not work if the nuts are locked to the profitsPubkey
      await this.wallet.add(nuts);
    }
  }
}
