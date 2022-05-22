# payments-engine
A toy payments system with support for multiple accounts and simulated transactions. Takes in a file to a CSV of transactions and spits out a CSV of the account details to stdout.

## Usage
```bash
# view CLI parameters
cargo run -- --help 

# run engine through CLI
cargo run -- transactions.csv > account_details.csv
```

## Testing
* `cargo test` runs built in integration tests on the engine.
* The `examples/` folder contains .csv transactions for testing through cli.

## Notes
- Engine transaction errors are ignored/skipped through the CLI interface.
- Transactions with invalid inputs (i.e. negative amounts, missing or unknown ids, etc.) are rejected and return an error internally.
- Transactions which could leave accounts or other transactions in "invalid states" (negative balances, unfreezing accounts, multiple disputes/resolutions/chargebacks) are also rejected internally with an error.
- Commited transactions (deposits/withdrawals) marked as invalid (through dispute -> chargeback) freeze the account preventing any further transactions on it.
- Committed transactions can be disputed then resolved more than once as long as it would leave the account in a "valid" state.
- Chargebacks on disputed withdrawals re-applies the withdrawal instead of coverting it into a deposit which avoids reporting the balance with extra currency.