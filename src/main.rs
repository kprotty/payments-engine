use anyhow::Result;
use clap::Parser;
use payments_engine::{Engine, Transaction};

#[derive(Parser, Debug)]
struct Arguments {
    csv_file_path: String,
}

fn main() -> Result<()> {
    let args = Arguments::parse();
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) // trim whitespace from both headers and values
        .flexible(true) // allow missing "amount" fields for non deposit/withdrawal types
        .from_path(args.csv_file_path)?;

    let mut engine = Engine::new();
    for result in reader.deserialize() {
        let transaction: Transaction = result?;
        if let Err(e) = engine.apply(transaction) {
            eprintln!("{e:?} {transaction:?}");
        }
    }

    let mut writer = csv::Writer::from_writer(std::io::stdout());
    for client in engine.clients() {
        writer.serialize(client)?;
    }

    writer.flush()?;
    Ok(())
}
