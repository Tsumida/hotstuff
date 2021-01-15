use crate::config::{Committee, Parameters};
use crate::core::Core;
use crate::error::ConsensusResult;
use crate::leader::LeaderElector;
use crate::mempool::{MempoolDriver, NodeMempool};
use crate::messages::Block;
use crate::network::{NetReceiver, NetSender};
use crate::synchronizer::Synchronizer;
use crypto::{PublicKey, SignatureService};
use store::Store;
use tokio::sync::mpsc::{channel, Sender};

#[cfg(test)]
#[path = "tests/consensus_tests.rs"]
pub mod consensus_tests;

pub struct Consensus;

impl Consensus {
    pub async fn run<Mempool: 'static + NodeMempool>(
        name: PublicKey,
        committee: Committee,
        parameters: Parameters,
        signature_service: SignatureService,
        store: Store,
        mempool: Mempool,
        commit_channel: Sender<Block>,
    ) -> ConsensusResult<()> {
        let (tx_core, rx_core) = channel(1000);

        // Net
        let network_channel = NetSender::make(name, committee.clone());
        let address = committee.address(&name).map(|mut x| {
            x.set_ip("0.0.0.0".parse().unwrap());
            x
        })?;
        let () = NetReceiver::make(&address, tx_core.clone()).await;

        // The leader elector algorithm.
        let leader_elector = LeaderElector::new(committee.clone());

        // Make the mempool driver which will mediate our requests to the mempool.
        let mempool_driver = MempoolDriver::new(mempool, tx_core.clone(), store.clone());

        // Make the synchronizer. This instance runs in a background thread
        // and asks other nodes for any block that we may be missing.
        let synchronizer = Synchronizer::new(
            name,
            store.clone(),
            network_channel.clone(),
            /* core_channel */ tx_core.clone(),
            parameters.sync_retry_delay,
        )
        .await;

        let mut core = Core::new(
            name,
            committee,
            parameters,
            signature_service,
            store,
            leader_elector,
            mempool_driver,
            synchronizer,
            /* core_channel */ rx_core,
            /* loopback_channel */ tx_core,
            network_channel,
            commit_channel,
        );
        tokio::spawn(async move {
            core.run().await;
        });

        Ok(())
    }
}
