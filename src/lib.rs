#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]

mod api;
mod engine;

pub use self::{
    api::{Client, ClientId, Currency, Operation, Transaction, TransactionId},
    engine::Engine,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // for generating transaction ids during tests
    fn gen_tx_id() -> TransactionId {
        static TX_ID_GEN: AtomicU32 = AtomicU32::new(0);
        TX_ID_GEN.fetch_add(1, Ordering::Relaxed)
    }

    #[test]
    fn deposit() -> anyhow::Result<()> {
        let mut engine = Engine::new();

        // Normal Deposit
        engine.apply(Transaction {
            id: gen_tx_id(),
            operation: Operation::Deposit,
            client_id: 0,
            amount: Some(10.0),
        })?;

        let client = engine.clients().next().unwrap(); // there should be only one client
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 10.0); // make sure value & client was stored
        assert_eq!(client.under_dispute, 0.0);

        // Ignored deposits
        for (amount, passes) in [
            (None, false),
            (Some(Currency::NAN), false),
            (Some(Currency::INFINITY), false),
            (Some(0.0), true),
            (Some(-0.0), true),
        ] {
            let result = engine.apply(Transaction {
                id: gen_tx_id(),
                operation: Operation::Deposit,
                client_id: 0,
                amount: amount,
            });
            assert_eq!(result.is_ok(), passes);

            // make sure everything stayed the same
            let client = engine.clients().next().unwrap();
            assert!(!client.is_frozen);
            assert_eq!(client.balance, 10.0);
            assert_eq!(client.under_dispute, 0.0);
        }

        // Multiple deposits
        engine.apply(Transaction {
            id: gen_tx_id(),
            operation: Operation::Deposit,
            client_id: 0,
            amount: Some(20.0),
        })?;

        let client = engine.clients().next().unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 30.0); // make sure value compounded
        assert_eq!(client.under_dispute, 0.0);

        // Deposits create multiple clients
        engine.apply(Transaction {
            id: gen_tx_id(),
            operation: Operation::Deposit,
            client_id: 1,
            amount: Some(42.0),
        })?;

        let client = engine.clients().find(|c| c.id == 0).unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 30.0);
        assert_eq!(client.under_dispute, 0.0);

        let client = engine.clients().find(|c| c.id == 1).unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 42.0);
        assert_eq!(client.under_dispute, 0.0);

        // Multiple deposites affect clients separately
        engine.apply(Transaction {
            id: gen_tx_id(),
            operation: Operation::Deposit,
            client_id: 0,
            amount: Some(10.0),
        })?;

        engine.apply(Transaction {
            id: gen_tx_id(),
            operation: Operation::Deposit,
            client_id: 1,
            amount: Some(8.0),
        })?;

        let client = engine.clients().find(|c| c.id == 0).unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 40.0);
        assert_eq!(client.under_dispute, 0.0);

        let client = engine.clients().find(|c| c.id == 1).unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 50.0);
        assert_eq!(client.under_dispute, 0.0);

        Ok(())
    }

    #[test]
    fn withdraw() -> anyhow::Result<()> {
        let mut engine = Engine::new();

        // Normal deposit
        engine.apply(Transaction {
            id: gen_tx_id(),
            operation: Operation::Deposit,
            client_id: 0,
            amount: Some(10.0),
        })?;

        let client = engine.clients().next().unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 10.0);
        assert_eq!(client.under_dispute, 0.0);

        // Normal Withdrawal
        engine.apply(Transaction {
            id: gen_tx_id(),
            operation: Operation::Withdrawal,
            client_id: 0,
            amount: Some(7.0),
        })?;

        let client = engine.clients().next().unwrap(); // there should be only one client
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 3.0); // make sure value was subtracted from deposit
        assert_eq!(client.under_dispute, 0.0);

        // Ignored withdrawals
        for (amount, passes) in [
            (None, false),
            (Some(Currency::NAN), false),
            (Some(Currency::INFINITY), false),
            (Some(0.0), true),
            (Some(-0.0), true),
            (Some(9999.0), false),
            (Some(3.1), false),
        ] {
            let result = engine.apply(Transaction {
                id: gen_tx_id(),
                operation: Operation::Withdrawal,
                client_id: 0,
                amount: amount,
            });
            assert_eq!(result.is_ok(), passes);

            // make sure everything stayed the same
            let client = engine.clients().next().unwrap();
            assert!(!client.is_frozen);
            assert_eq!(client.balance, 3.0);
            assert_eq!(client.under_dispute, 0.0);
        }

        Ok(())
    }

    #[test]
    fn dispute_resolve_chargeback() -> anyhow::Result<()> {
        let mut engine = Engine::new();

        // Normal Deposit
        let deposit_id = gen_tx_id();
        engine.apply(Transaction {
            id: deposit_id,
            operation: Operation::Deposit,
            client_id: 0,
            amount: Some(10.0),
        })?;

        let client = engine.clients().next().unwrap(); // there should be only one client
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 10.0); // make sure value & client was stored
        assert_eq!(client.under_dispute, 0.0);

        // Normal Withdrawal
        let withdrawal_id = gen_tx_id();
        engine.apply(Transaction {
            id: withdrawal_id,
            operation: Operation::Withdrawal,
            client_id: 0,
            amount: Some(3.0),
        })?;

        let client = engine.clients().next().unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 7.0); // make sure value was subtracted
        assert_eq!(client.under_dispute, 0.0);

        // Fail to Dispute deposit (client balance would go negative)
        let bad_tx = engine.apply(Transaction {
            id: deposit_id,
            operation: Operation::Dispute,
            client_id: 0,
            amount: None,
        });
        assert!(bad_tx.is_err());

        let client = engine.clients().next().unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 7.0); // make sure nothing changed
        assert_eq!(client.under_dispute, 0.0);

        // Dispute withdrawal
        engine.apply(Transaction {
            id: withdrawal_id,
            operation: Operation::Dispute,
            client_id: 0,
            amount: Some(4.0), // amount should be ignored
        })?;

        let client = engine.clients().next().unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 10.0); // make sure value goes back to before withdrawal
        assert_eq!(client.under_dispute, -3.0);

        // Fail to dispute withdrawal multiple times
        let bad_tx = engine.apply(Transaction {
            id: withdrawal_id,
            operation: Operation::Dispute,
            client_id: 0,
            amount: Some(4.0), // amount should be ignored
        });
        assert!(bad_tx.is_err());

        let client = engine.clients().next().unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 10.0); // make sure nothing changed
        assert_eq!(client.under_dispute, -3.0);

        // Resolve withdrawal
        engine.apply(Transaction {
            id: withdrawal_id,
            operation: Operation::Resolve,
            client_id: 0,
            amount: Some(5.0), // amount should be ignored
        })?;

        let client = engine.clients().next().unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 7.0); // make sure value gets subtracted again
        assert_eq!(client.under_dispute, 0.0); // make sure this was reset

        // Dispute withdrawal (again)
        engine.apply(Transaction {
            id: withdrawal_id,
            operation: Operation::Dispute,
            client_id: 0,
            amount: Some(4.0), // amount should be ignored
        })?;

        let client = engine.clients().next().unwrap();
        assert!(!client.is_frozen);
        assert_eq!(client.balance, 10.0); // make sure value goes back to before withdrawal
        assert_eq!(client.under_dispute, -3.0);

        // Chargeback
        engine.apply(Transaction {
            id: withdrawal_id,
            operation: Operation::Chargeback,
            client_id: 0,
            amount: Some(42.0), // amount should be ignored
        })?;

        let client = engine.clients().next().unwrap();
        assert!(client.is_frozen); // client should be frozen
        assert_eq!(client.balance, 10.0); // make sure value is the same
        assert_eq!(client.under_dispute, 0.0); // make sure the dispute was settled from chargeback

        // Make sure all forms of transactions fail on the frozen account
        for (id, operation, amount) in [
            (gen_tx_id(), Operation::Deposit, Some(10.0)),
            (gen_tx_id(), Operation::Withdrawal, Some(1.0)),
            (withdrawal_id, Operation::Dispute, None),
            (withdrawal_id, Operation::Resolve, None),
            (withdrawal_id, Operation::Chargeback, None),
        ] {
            // Fail to dispute withdrawal multiple times
            let bad_tx = engine.apply(Transaction {
                id,
                operation,
                client_id: 0,
                amount,
            });
            assert!(bad_tx.is_err());

            let client = engine.clients().next().unwrap();
            assert!(client.is_frozen); // client should still be frozen
            assert_eq!(client.balance, 10.0); // make sure nothing changed
            assert_eq!(client.under_dispute, 0.0);
        }

        Ok(())
    }
}
