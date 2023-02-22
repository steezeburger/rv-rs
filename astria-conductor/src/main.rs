use std::time::Duration;

use tokio::{signal, time};

use crate::alert::Alert;
use crate::cli::args;
use crate::conf::Conf;
use crate::driver::DriverCommand;
use crate::error::*;

pub mod alert;
mod cli;
pub mod conf;
mod driver;
mod error;
mod executor;
mod logger;
mod reader;

#[tokio::main]
async fn main() -> Result<()> {
    // logs
    logger::initialize();

    // parse args
    let matches = args::parse_args();
    let base_url = matches.get_one::<String>("url").expect("url required");
    let namespace_id = matches
        .get_one::<String>("namespace_id")
        .expect("namespace id required");
    let rpc_address = matches
        .get_one::<String>("rpc_address")
        .expect("RPC address required");

    // configuration
    let conf = Conf::new(
        base_url.to_owned(),
        namespace_id.to_owned(),
        rpc_address.to_owned(),
    );
    log::info!("Using node at {}", conf.celestia_node_url);

    // spawn our driver
    let (mut driver_handle, mut alert_rx) = driver::spawn(conf)?;

    // NOTE - this will most likely be replaced by an RPC server that will receive gossip
    //  messages from the sequencer
    let mut interval = time::interval(Duration::from_secs(3));

    let mut run = true;
    while run {
        tokio::select! {
            // handle alerts from the driver
            Some(alert) = alert_rx.recv() => {
                match alert {
                    Alert::DriverError(error_string) => {
                        println!("error: {}", error_string);
                        run = false;
                    }
                    Alert::BlockReceived{block_height} => {
                        println!("block received at {}", block_height);
                    }
                }
            }
            // request new blocks every X seconds
            _ = interval.tick() => {
                driver_handle.tx.send(DriverCommand::GetNewBlocks)?;
            }
            // shutdown properly on ctrl-c
            _ = signal::ctrl_c() => {
                driver_handle.shutdown().await?;
            }
        }
        if !run {
            break;
        }
    }

    Ok(())
}
