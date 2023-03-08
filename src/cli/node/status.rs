use futures::try_join;

use crate::{
    cli::BazukaConfig,
    client::{BazukaClient, NodeError},
    wallet::{TxBuilder, Wallet},
};

pub async fn status(conf: Option<BazukaConfig>, wallet: Option<Wallet>) {
    let (conf, wallet) = conf.zip(wallet).expect("Bazuka is not initialized!");
    let wallet = TxBuilder::new(&wallet.seed());
    let (req_loop, client) = BazukaClient::connect(
        wallet.get_priv_key(),
        conf.random_node(),
        conf.network,
        None,
    );
    try_join!(
        async move {
            println!("{:#?}", client.stats().await?);
            Ok::<(), NodeError>(())
        },
        req_loop
    )
    .unwrap();
}
