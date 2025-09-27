//! Cashu wallet integration for TollGate payments
//! 
//! Handles:
//! - Cashu token creation and management
//! - Mint compatibility checking
//! - Balance management
//! - Payment token generation

use crate::tollgate::errors::{TollGateError, TollGateResult};
use crate::tollgate::protocol::PricingOption;
use cdk::Amount;
use cdk::wallet::{MintQuote, Wallet, SendKind};
use cdk::cdk_database::WalletMemoryDatabase;
use cdk::nuts::CurrencyUnit;
use cdk::amount::SplitTarget;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Cashu wallet for TollGate payments
pub struct TollGateWallet {
    wallets: HashMap<String, Wallet>, // keyed by mint URL
    default_mint: Option<String>,
}

/// Payment token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentToken {
    pub token: String,
    pub amount: u64,
    pub mint_url: String,
    pub unit: String,
}

/// Wallet balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalance {
    pub mint_url: String,
    pub balance: u64,
    pub unit: String,
    pub pending: u64,
}

impl TollGateWallet {
    pub fn new() -> Self {
        Self {
            wallets: HashMap::new(),
            default_mint: None,
        }
    }

    /// Add a mint to the wallet
    pub async fn add_mint(&mut self, mint_url: &str, seed: [u8; 32]) -> TollGateResult<()> {
        if self.wallets.contains_key(mint_url) {
            return Ok(()); // Already added
        }

        let localstore = WalletMemoryDatabase::default();
        let wallet = Wallet::new(
            mint_url,
            CurrencyUnit::Sat,
            Arc::new(localstore),
            &seed,
            None,
        )
        .map_err(|e| TollGateError::wallet(format!("Failed to create wallet for mint {}: {}", mint_url, e)))?;

        self.wallets.insert(mint_url.to_string(), wallet);

        // Set as default if it's the first mint
        if self.default_mint.is_none() {
            self.default_mint = Some(mint_url.to_string());
        }

        log::info!("Added mint to wallet: {}", mint_url);
        Ok(())
    }

    /// Get available mints
    #[allow(dead_code)]
    pub fn get_available_mints(&self) -> Vec<String> {
        self.wallets.keys().cloned().collect()
    }

    /// Get balance for a specific mint
    pub async fn get_balance(&self, mint_url: &str) -> TollGateResult<u64> {
        let wallet = self.wallets.get(mint_url)
            .ok_or_else(|| TollGateError::wallet(format!("Mint not found: {}", mint_url)))?;

        let balance = wallet.total_balance().await
            .map_err(|e| TollGateError::wallet(format!("Failed to get balance: {}", e)))?;

        Ok(balance.into())
    }

    /// Get balance for all mints
    pub async fn get_all_balances(&self) -> TollGateResult<Vec<WalletBalance>> {
        let mut balances = Vec::new();

        for (mint_url, wallet) in &self.wallets {
            let balance = wallet.total_balance().await
                .map_err(|e| TollGateError::wallet(format!("Failed to get balance for {}: {}", mint_url, e)))?;

            balances.push(WalletBalance {
                mint_url: mint_url.clone(),
                balance: balance.into(),
                unit: "sat".to_string(), // TODO: Get actual unit from wallet
                pending: 0, // TODO: Get pending balance
            });
        }

        Ok(balances)
    }

    /// Check if we can afford a payment
    pub async fn can_afford(&self, pricing_option: &PricingOption, steps: u64) -> TollGateResult<bool> {
        let required_amount = pricing_option.price_per_step * steps;
        let balance = self.get_balance(&pricing_option.mint_url).await?;
        
        Ok(balance >= required_amount)
    }

    /// Create a payment token for the specified amount
    pub async fn create_payment_token(
        &self,
        pricing_option: &PricingOption,
        steps: u64,
    ) -> TollGateResult<PaymentToken> {
        let amount = pricing_option.price_per_step * steps;
        
        if amount < pricing_option.min_steps * pricing_option.price_per_step {
            return Err(TollGateError::wallet(format!(
                "Amount {} is below minimum {} steps",
                steps,
                pricing_option.min_steps
            )));
        }

        let wallet = self.wallets.get(&pricing_option.mint_url)
            .ok_or_else(|| TollGateError::wallet(format!("Mint not found: {}", pricing_option.mint_url)))?;

        // Check balance
        let balance = wallet.total_balance().await
            .map_err(|e| TollGateError::wallet(format!("Failed to get balance: {}", e)))?;

        if balance < Amount::from(amount) {
            return Err(TollGateError::InsufficientFunds {
                needed: amount,
                available: balance.into(),
            });
        }

        // Create the payment token
        let token = wallet.send(
            Amount::from(amount),
            None,
            None,
            &SplitTarget::default(),
            &SendKind::OnlineExact,
            false
        ).await
            .map_err(|e| TollGateError::wallet(format!("Failed to create payment token: {}", e)))?;

        Ok(PaymentToken {
            token: token.to_string(),
            amount,
            mint_url: pricing_option.mint_url.clone(),
            unit: pricing_option.price_unit.clone(),
        })
    }

    /// Request a mint quote for loading the wallet
    #[allow(dead_code)]
    pub async fn request_mint_quote(&self, mint_url: &str, amount: u64) -> TollGateResult<MintQuote> {
        let wallet = self.wallets.get(mint_url)
            .ok_or_else(|| TollGateError::wallet(format!("Mint not found: {}", mint_url)))?;

        let quote = wallet.mint_quote(Amount::from(amount), None).await
            .map_err(|e| TollGateError::wallet(format!("Failed to request mint quote: {}", e)))?;

        Ok(quote)
    }

    /// Check mint quote status and mint tokens if paid
    #[allow(dead_code)]
    pub async fn check_mint_quote(&self, mint_url: &str, quote_id: &str) -> TollGateResult<bool> {
        let wallet = self.wallets.get(mint_url)
            .ok_or_else(|| TollGateError::wallet(format!("Mint not found: {}", mint_url)))?;

        let status = wallet.mint_quote_state(quote_id).await
            .map_err(|e| TollGateError::wallet(format!("Failed to check mint quote: {}", e)))?;

        if status.state == cdk::nuts::MintQuoteState::Paid {
            // Mint the tokens
            wallet.mint(&status.quote, SplitTarget::default(), None).await
                .map_err(|e| TollGateError::wallet(format!("Failed to mint tokens: {}", e)))?;
            
            log::info!("Successfully minted tokens for quote {}", quote_id);
            return Ok(true);
        }

        Ok(false)
    }

    /// Find the best pricing option based on available balances
    pub async fn select_best_pricing_option(
        &self,
        options: &[PricingOption],
        steps: u64,
    ) -> TollGateResult<PricingOption> {
        let mut compatible_options = Vec::new();

        for option in options {
            if self.wallets.contains_key(&option.mint_url) {
                if let Ok(can_afford) = self.can_afford(option, steps).await {
                    if can_afford {
                        compatible_options.push(option.clone());
                    }
                }
            }
        }

        if compatible_options.is_empty() {
            return Err(TollGateError::wallet("No compatible pricing options with sufficient balance"));
        }

        // Select the option with the lowest total cost
        let best_option = compatible_options
            .into_iter()
            .min_by_key(|option| option.price_per_step * steps)
            .unwrap();

        Ok(best_option)
    }

    /// Get wallet statistics
    #[allow(dead_code)]
    pub async fn get_wallet_stats(&self) -> TollGateResult<WalletStats> {
        let balances = self.get_all_balances().await?;
        let total_balance: u64 = balances.iter().map(|b| b.balance).sum();
        let mint_count = self.wallets.len();

        Ok(WalletStats {
            total_balance,
            mint_count,
            balances,
            default_mint: self.default_mint.clone(),
        })
    }

    /// Set default mint
    #[allow(dead_code)]
    pub fn set_default_mint(&mut self, mint_url: &str) -> TollGateResult<()> {
        if !self.wallets.contains_key(mint_url) {
            return Err(TollGateError::wallet(format!("Mint not found: {}", mint_url)));
        }

        self.default_mint = Some(mint_url.to_string());
        Ok(())
    }

    /// Get default mint
    #[allow(dead_code)]
    pub fn get_default_mint(&self) -> Option<&String> {
        self.default_mint.as_ref()
    }
}

/// Wallet statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct WalletStats {
    pub total_balance: u64,
    pub mint_count: usize,
    pub balances: Vec<WalletBalance>,
    pub default_mint: Option<String>,
}

impl Default for TollGateWallet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wallet_creation() {
        let wallet = TollGateWallet::new();
        assert_eq!(wallet.get_available_mints().len(), 0);
        assert!(wallet.get_default_mint().is_none());
    }

    #[test]
    fn test_payment_token_creation() {
        let token = PaymentToken {
            token: "cashuAbc123".to_string(),
            amount: 100,
            mint_url: "https://mint.example.com".to_string(),
            unit: "sat".to_string(),
        };

        assert_eq!(token.amount, 100);
        assert_eq!(token.unit, "sat");
    }

    #[test]
    fn test_wallet_balance() {
        let balance = WalletBalance {
            mint_url: "https://mint.example.com".to_string(),
            balance: 1000,
            unit: "sat".to_string(),
            pending: 0,
        };

        assert_eq!(balance.balance, 1000);
        assert_eq!(balance.pending, 0);
    }
}