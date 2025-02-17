use std::{pin::Pin, sync::Arc, time::Duration};

use alloy::{
    providers::Provider,
    rpc::types::{Block, BlockNumberOrTag, BlockTransactions, Transaction},
    transports::Transport,
};
use burberry::Collector;
use eyre::Report as ErrReport;
use futures::Stream;
use tokio::time;

pub struct PollingBlockCollector<T: Transport + Clone> {
    provider: Arc<dyn Provider<T>>,
    interval: Duration,
    last_block: u64,
}

impl<T: Transport + Clone> PollingBlockCollector<T> {
    pub fn new(provider: Arc<dyn Provider<T>>) -> Self {
        Self {
            provider,
            interval: Duration::from_secs(1),
            last_block: 0,
        }
    }
}

#[async_trait::async_trait]
impl<T: Transport + Clone + Send + Sync + 'static> Collector<Block> for PollingBlockCollector<T> {
    async fn get_event_stream(&self) -> Result<Pin<Box<dyn Stream<Item = Block> + Send + '_>>, ErrReport> {
        let provider = self.provider.clone();
        let interval = self.interval;
        let mut last_block = self.last_block;

        let stream = async_stream::stream! {
            let mut interval = time::interval(interval);
            loop {
                interval.tick().await;

                if let Ok(Some(block)) = provider.get_block_by_number(BlockNumberOrTag::Latest, true).await {
                    let block_number = block.header.number.unwrap();
                    let block_number = block_number.try_into().unwrap_or(0u64);
                    if block_number > last_block {
                        last_block = block_number;
                        yield block;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

pub struct PollingMempoolCollector<T: Transport + Clone> {
    provider: Arc<dyn Provider<T>>,
    interval: Duration,
}

impl<T: Transport + Clone> PollingMempoolCollector<T> {
    pub fn new(provider: Arc<dyn Provider<T>>) -> Self {
        Self {
            provider,
            interval: Duration::from_millis(100),
        }
    }
}

#[async_trait::async_trait]
impl<T: Transport + Clone + Send + Sync + 'static> Collector<Transaction> for PollingMempoolCollector<T> {
    async fn get_event_stream(&self) -> Result<Pin<Box<dyn Stream<Item = Transaction> + Send + '_>>, ErrReport> {
        let provider = self.provider.clone();
        let interval = self.interval;

        let stream = async_stream::stream! {
            let mut interval = time::interval(interval);
            let mut seen_txs = std::collections::HashSet::new();

            loop {
                interval.tick().await;

                if let Ok(Some(block)) = provider.get_block_by_number(BlockNumberOrTag::Latest, true).await {
                    match block.transactions {
                        BlockTransactions::Full(txs) => {
                            for tx in txs {
                                let hash = tx.hash;
                                if !seen_txs.contains(&hash) {
                                    seen_txs.insert(hash);
                                    yield tx;
                                }
                            }
                        }
                        BlockTransactions::Hashes(_) => {}
                        BlockTransactions::Uncle => {}
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
