extern crate core;

use std::{sync::Arc, time::Instant};

use alloy::{
    primitives::{B256, U256},
    providers::{IpcConnect, Provider, ProviderBuilder},
};
use clap::Parser;

use crate::{meme, search};

#[derive(Debug, Parser)]
pub struct Args {
    tx: B256,

    #[arg(long, env = "ETH_RPC_URL")]
    url: String,
}

pub async fn run(args: Args) {
    tracing_subscriber::fmt().init();

    let provider = ProviderBuilder::new()
        .on_http(args.url.parse().unwrap());

    let provider: Arc<dyn Provider<_>> = Arc::new(provider);

    let tx = provider
        .get_transaction_by_hash(args.tx)
        .await
        .expect("failed to get transaction")
        .expect("transaction not found");

    let buy = meme::Buy::try_from(&tx).expect("not a buy tx");

    let block = provider
        .get_block_by_number(tx.block_number.unwrap().into(), false)
        .await
        .expect("failed to get block")
        .expect("block not found");

    let block_number = block.header.number.unwrap();

    let token_info = meme::get_token_info(Arc::clone(&provider), buy.token, block_number.into())
        .await
        .expect("fail to get token info");

    let fee_rate = meme::get_fee_rate(provider.as_ref(), block_number.into())
        .await
        .expect("fail to get fee rate");

    let min_fee = meme::get_min_fee(provider.as_ref(), block_number.into())
        .await
        .expect("fail to get min fee");

    let context = search::Context {
        token_info,
        fee_rate,
        min_fee,
        buy,
        token_balance: U256::ZERO,
    };

    let start = Instant::now();
    let solution = search::go(context).expect("cannot find solution");
    let optimal_elapsed = start.elapsed();

    println!("profit: {}", solution.profit);
    println!("time: {:?}", optimal_elapsed);

    // Prepare transactions for bundle simulation
    let mut txs = Vec::new();
    
    // Add the original buy transaction
    txs.push(tx.clone());
    
    // Add our solution's sell transaction
    let sell_tx = solution.to_transaction();
    txs.push(sell_tx);

    // Create the bundle simulation request
    let call_bundle = provider
        .request(
            "eth_callBundle",
            [serde_json::json!({
                "txs": txs,
                "blockNumber": block_number,
                "stateBlockNumber": block_number - 1,
                "timestamp": block.header.timestamp,
            })]
        )
        .await
        .expect("failed to simulate bundle");

    // Parse and validate the simulation results
    let simulation_result: serde_json::Value = call_bundle;
    
    // Check if simulation was successful
    if let Some(error) = simulation_result.get("error") {
        println!("Bundle simulation failed: {}", error);
        return;
    }

    // Extract and display relevant simulation results
    println!("\nBundle Simulation Results:");
    println!("------------------------");
    if let Some(success) = simulation_result.get("success") {
        println!("Simulation successful: {}", success);
    }
    
    if let Some(gas_used) = simulation_result.get("gasUsed") {
        println!("Total gas used: {}", gas_used);
    }

    println!("\nValidation complete - bundle can be executed safely.")
}
