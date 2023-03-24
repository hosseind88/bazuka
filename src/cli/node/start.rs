use crate::{
    cli::{run_node, BazukaConfig},
    client::messages::SocialProfiles,
    wallet::Wallet,
};

pub async fn start(
    discord_handle: Option<String>,
    client_only: bool,
    conf: &BazukaConfig,
    wallet: &Wallet,
    dev: bool,
    dev_address: &str
) {
    run_node(
        conf.clone(),
        wallet.clone(),
        SocialProfiles {
            discord: discord_handle,
        },
        client_only,
        dev,
        dev_address
    )
    .await;
}
