mod clients;
mod events;
mod storage;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use clients::Client;
use events::{Event, Record};
use log::*;
use storage::MemoryStore;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "payment-processor",
    about = "A tool for processing payment events"
)]
struct Opt {
    /// Print error messages to stderr
    #[structopt(long)]
    verbose: bool,
    /// The CSV file containing payment events
    input_file: String,
}

fn handle_entry(
    entry: Result<Record>,
    clients_state: &mut HashMap<u16, Client<Arc<Mutex<MemoryStore>>>>,
    store: Arc<Mutex<MemoryStore>>,
) -> Result<()> {
    let record = entry?;
    let event = Event::try_from(record)?;
    let client = clients_state
        .entry(event.client_id())
        .or_insert_with(|| Client::new(event.client_id(), store));
    client
        .update(&event)
        .with_context(|| format!("processing {:?}", event))
}

fn main() {
    let opt = Opt::from_args();
    let v = if opt.verbose {
        stderrlog::LogLevelNum::Error
    } else {
        stderrlog::LogLevelNum::Off
    };
    stderrlog::new()
        .module(module_path!())
        .verbosity(v)
        .init()
        .unwrap();

    let store = MemoryStore::new();
    let mut clients_state: HashMap<u16, Client<Arc<Mutex<MemoryStore>>>> = HashMap::new();
    let mut rdr = csv::Reader::from_path(opt.input_file).unwrap();
    for entry in rdr.deserialize() {
        if let Err(e) = handle_entry(
            entry.map_err(anyhow::Error::msg),
            &mut clients_state,
            Arc::clone(&store),
        ) {
            error!("{:?}", e);
        }
    }

    println!("client,available,held,total,locked");
    let output: Vec<String> = clients_state
        .into_values()
        .map(|client| {
            format!(
                "{},{},{},{},{}",
                client.id(),
                client.available(),
                client.held(),
                client.total(),
                client.locked()
            )
        })
        .collect();
    println!("{}", output.join("\n"));
}
