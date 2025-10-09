import type { WalletBalance } from "./api";

export function msatToSat(msat: number): number {
  return Math.floor(msat / 1000);
}

export function satToMsat(sat: number): number {
  return sat * 1000;
}

export function calculateTotalMsat(balances: WalletBalance[]): number {
  return balances.reduce((total, balance) => {
    if (balance.unit === "sat") {
      return total + satToMsat(balance.balance);
    } else if (balance.unit === "msat") {
      return total + balance.balance;
    } else {
      return total + satToMsat(balance.balance);
    }
  }, 0);
}

export function formatBalanceDisplay(totalMsat: number): {
  primary: string;
  secondary: string | null;
  unit: string;
} {
  const totalSat = msatToSat(totalMsat);

  if (totalSat >= 1) {
    return {
      primary: totalSat.toLocaleString(),
      secondary: `${totalMsat.toLocaleString()} msats`,
      unit: "sats",
    };
  } else {
    return {
      primary: totalMsat.toLocaleString(),
      secondary: null,
      unit: "msats",
    };
  }
}
