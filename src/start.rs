use std::sync::Arc;

use alloy::{
    primitives::B256,
    providers::{Provider, ProviderBuilder},
    signers::{local::PrivateKeySigner, Signer},
};
use burberry::{map_collector, Engine};
use clap::Parser;

use crate::{
    strategy::{Config, Event, Strategy},
    collectors::{PollingBlockCollector, PollingMempoolCollector},
};

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, env = "ETH_RPC_URL")]
    pub url: String,

    #[arg(long)]
    pub private_key: B256,

    #[command(flatten)]
    pub config: Config,
}

pub async fn run(args: Args) {
    tracing_subscriber::fmt::init();

    let provider = ProviderBuilder::new()
        .on_http(args.url.parse().unwrap());
    let provider: Arc<dyn Provider<_>> = Arc::new(provider);

    let chain_id = provider.get_chain_id().await.expect("fail to get chain id");

    let signer = PrivateKeySigner::from_bytes(&args.private_key)
        .expect("fail to parse private key")
        .with_chain_id(Some(chain_id));

    let attacker = signer.address();

    let mut engine = Engine::default();

    engine.add_collector(map_collector!(
        PollingMempoolCollector::new(provider.clone()),
        Event::PendingTx
    ));
    engine.add_collector(map_collector!(
        PollingBlockCollector::new(provider.clone()),
        Event::FullBlock
    ));

    let strategy = Strategy::new(args.config.clone().into(), attacker, Arc::clone(&provider));

    engine.add_strategy(Box::new(strategy));

    engine.run_and_join().await.unwrap();
}