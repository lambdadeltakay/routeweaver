use clap::Parser;
use config::Config;
use dashmap::DashMap;

use deadqueue::unlimited::Queue;
use proto::Protocol;
use runtime::accept_connections_from_peers;
use scc::HashCache;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::time::sleep;
use transport::Transport;

mod config;
mod error;
mod limited;
mod peer;
mod proto;
mod runtime;
mod test;
mod transport;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    // Final will be /etc/routeweaver/config.toml
    #[arg(short, long, default_value = "config/latitude-7490.toml")]
    config_location: PathBuf,
}

#[tokio::main]
async fn main() {
    flexi_logger::Logger::try_with_str("debug")
        .unwrap()
        .start()
        .unwrap();

    let cli = Cli::parse();

    let config = std::fs::read_to_string(cli.config_location).unwrap();
    let config: Config = toml::from_str(&config).unwrap();
    let config = Arc::new(config);
    let pre_assembled_message_tracker = Arc::new(HashCache::with_capacity(0, 1024));
    let clear_text_message_queue = Arc::new(Queue::new());

    for protocol in &config.enabled_transports {
        match protocol {
            #[cfg(tcp_transport)]
            Protocol::Tcp => {
                tokio::spawn(
                    accept_connections_from_peers::<transport::tcp::TcpTransport>(
                        clear_text_message_queue.clone(),
                        pre_assembled_message_tracker.clone(),
                        config.clone(),
                    ),
                );
            }
            #[cfg(unix_transport)]
            Protocol::Unix => {
                tokio::spawn(accept_connections_from_peers::<
                    transport::unix::UnixTransport,
                >(
                    clear_text_message_queue.clone(),
                    pre_assembled_message_tracker.clone(),
                    config.clone(),
                ));
            }
            Protocol::Http => todo!(),
            #[allow(unreachable_patterns)]
            _ => {
                log::error!("Unsupported transport: {:?}", protocol);
                continue;
            }
        };
    }

    loop {
        sleep(Duration::from_secs(100)).await;
    }
}
