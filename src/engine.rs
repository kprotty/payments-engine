use super::api::{Client, ClientId, Currency, Operation, Transaction, TransactionId};
use anyhow::{anyhow, Result};
use std::collections::{hash_map::Entry, HashMap};
use std::ops::Neg;

#[derive(Default)]
struct Account {
    is_frozen: bool,
    balance: Currency,
    disputing: Currency,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum AdjustmentState {
    Valid,
    UnderDispute,
    Invalid,
}

#[derive(Copy, Clone, Debug)]
struct Adjustment {
    account_id: ClientId,
    amount: Currency,
    state: AdjustmentState,
}

#[derive(Debug)]
struct AccountTransaction {
    amount: Currency,
    balance: Currency,
    disputing: Currency,
    state: AdjustmentState,
}

impl AccountTransaction {
    fn apply(
        account: &mut Account,
        adjustment: &mut Adjustment,
        update_tx: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        // Check if the AccountTransaction can even occur
        if account.is_frozen {
            return Err(anyhow!("transaction attempt on frozen account"));
        }
        if adjustment.state == AdjustmentState::Invalid {
            return Err(anyhow!("invalid transaction"));
        }

        // Perform the AccountTransaction
        let mut tx = Self {
            amount: adjustment.amount,
            balance: account.balance,
            disputing: account.disputing,
            state: adjustment.state,
        };
        update_tx(&mut tx)?;

        // Make sure the balance is always valid (real number and not negative)
        if !tx.balance.is_finite() || tx.balance < 0.0 {
            return Err(anyhow!("transaction invalidated client balance"));
        }

        // Make sure the dispute amount is always valid (can be negative when disputing withdrawals)
        if !tx.disputing.is_finite() {
            return Err(anyhow!("transaction invalidated client dispute balance"));
        }

        // Commit it to the Account and Adjustment
        Ok({
            adjustment.state = tx.state;
            account.balance = tx.balance;
            account.disputing = tx.disputing;
            account.is_frozen = tx.state == AdjustmentState::Invalid;
        })
    }
}

#[derive(Default)]
pub struct Engine {
    accounts: HashMap<ClientId, Account>,
    adjustments: HashMap<TransactionId, Adjustment>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            adjustments: HashMap::new(),
        }
    }

    pub fn clients(&self) -> impl Iterator<Item = Client> + '_ {
        self.accounts.iter().map(|(account_id, account)| Client {
            id: *account_id,
            allocated: account.balance + account.disputing,
            balance: account.balance,
            under_dispute: account.disputing,
            is_frozen: account.is_frozen,
        })
    }

    pub fn apply(&mut self, transaction: Transaction) -> Result<()> {
        match transaction.operation {
            Operation::Deposit => {
                let to_accredit = |amount: Currency| amount;
                self.insert_transaction(transaction, to_accredit)
            }
            Operation::Withdrawal => {
                let to_accredit = |amount: Currency| f64::neg(amount);
                self.insert_transaction(transaction, to_accredit)
            }
            Operation::Dispute => self.update_transaction(transaction, |tx| {
                if tx.state != AdjustmentState::Valid {
                    return Err(anyhow!("disputing an invalid transaction"));
                }

                // Clients available funds are transferred to disputing
                tx.balance -= tx.amount;
                tx.disputing += tx.amount;
                tx.state = AdjustmentState::UnderDispute;
                Ok(())
            }),
            Operation::Resolve => self.update_transaction(transaction, |tx| {
                if tx.state != AdjustmentState::UnderDispute {
                    return Err(anyhow!("resolving a transaction not under dispute"));
                }

                // Clients disputing funds are transferred to available
                tx.disputing -= tx.amount;
                tx.balance += tx.amount;
                tx.state = AdjustmentState::Valid;
                Ok(())
            }),
            Operation::Chargeback => self.update_transaction(transaction, |tx| {
                if tx.state != AdjustmentState::UnderDispute {
                    return Err(anyhow!("invalidating a transaction not under dispute"));
                }

                // Clients disputing funds and available funds are removed.
                // An invalid transaction locks the account from further transactions
                tx.disputing -= tx.amount;
                tx.state = AdjustmentState::Invalid;
                Ok(())
            }),
        }
    }

    fn insert_transaction(
        &mut self,
        transaction: Transaction,
        to_accredit: impl FnOnce(Currency) -> Currency,
    ) -> Result<()> {
        let amount = transaction.amount;
        let adjustment_id = transaction.id;
        let account_id = transaction.client_id;

        // Validate the transaction amount passed in
        let amount = amount.ok_or(anyhow!("transaction amount missing"))?;
        if !amount.is_finite() {
            return Err(anyhow!("invalid transaction amount"));
        }

        match self.adjustments.entry(adjustment_id) {
            Entry::Occupied(_) => Err(anyhow!("transaction already exists")),
            Entry::Vacant(entry) => {
                // Get or create the Account if it doesn't exist.
                // Also prepare the corresponding Adjustment.
                let account = self.accounts.entry(account_id).or_default();
                let mut adjustment = Adjustment {
                    account_id,
                    amount: to_accredit(amount),
                    state: AdjustmentState::Valid,
                };

                // Try to apply the transaction to the account and adjustment.
                // A withdrawal will have a negative amount so += will be subtraciton.
                AccountTransaction::apply(account, &mut adjustment, |tx| {
                    assert_eq!(tx.state, AdjustmentState::Valid);
                    tx.balance += tx.amount;
                    Ok(())
                })?;

                // Only once the transaction succeeds do we commit it.
                entry.insert(adjustment);
                Ok(())
            }
        }
    }

    fn update_transaction(
        &mut self,
        transaction: Transaction,
        update_tx: impl FnOnce(&mut AccountTransaction) -> Result<()>,
    ) -> Result<()> {
        // Make sure the transaction exists.
        let adjustment_id = transaction.id;
        let adjustment = self
            .adjustments
            .get_mut(&adjustment_id)
            .ok_or(anyhow!("transaction reference does not exist"))?;

        // Make sure it matches the account.
        let account_id = transaction.client_id;
        if adjustment.account_id != account_id {
            return Err(anyhow!("transaction reference client-mismatch"));
        }

        // If the transaction exists, the account must exist as well from insert_transaction().
        let account = self
            .accounts
            .get_mut(&account_id)
            .expect("transaction exists without an account");

        // Apply the update to the account and adjustment
        AccountTransaction::apply(account, adjustment, update_tx)
    }
}
