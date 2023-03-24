#[cfg(feature = "node")]
use {
    crate::blockchain::KvStoreChain,
    crate::client::{messages::SocialProfiles, Limit, NodeRequest},
    crate::common::*,
    crate::db::LevelDbKvStore,
    crate::db::RamKvStore,
    crate::db::KvStore,
    crate::node::{node_create, Firewall},
    hyper::server::conn::AddrStream,
    hyper::service::{make_service_fn, service_fn},
    hyper::{Body, Client, Request, Response, Server, StatusCode},
    std::sync::Arc,
    tokio::sync::mpsc,
};

#[cfg(feature = "client")]
use {
    crate::client::{NodeError, PeerAddress},
    crate::config,
    crate::core::{Address, Amount, TokenId, ZieshaAddress},
    crate::wallet::{TxBuilder, Wallet},
    colored::Colorize,
    rand::Rng,
    serde::{Deserialize, Serialize},
    std::net::SocketAddr,
    std::path::{Path, PathBuf},
    structopt::StructOpt,
    tokio::try_join,
};

pub mod chain;
pub mod init;
pub mod node;
pub mod wallet;
pub use init::*;

#[cfg(feature = "client")]
const DEFAULT_PORT: u16 = 8765;
const BAZUKA_NOT_INITILIZED: &str = "Bazuka is not initialized";

#[cfg(feature = "client")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BazukaConfig {
    listen: SocketAddr,
    external: PeerAddress,
    network: String,
    miner_token: String,
    bootstrap: Vec<PeerAddress>,
    db: PathBuf,
}

#[cfg(feature = "client")]
impl BazukaConfig {
    fn random_node(&self) -> PeerAddress {
        PeerAddress(SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT)))
        /*self.bootstrap
        .choose(&mut rand::thread_rng())
        .unwrap_or(&PeerAddress(SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT))))
        .clone()*/
    }
}

#[derive(StructOpt)]
#[allow(clippy::large_enum_variant)]
#[cfg(feature = "client")]
enum WalletOptions {
    /// Add a new token to the wallet
    AddToken {
        #[structopt(long)]
        id: TokenId,
    },
    /// Creates a new token
    NewToken {
        #[structopt(long)]
        memo: Option<String>,
        #[structopt(long)]
        name: String,
        #[structopt(long)]
        symbol: String,
        #[structopt(long)]
        supply: Amount,
        #[structopt(long, default_value = "0")]
        decimals: u8,
        #[structopt(long)]
        mintable: bool,
        #[structopt(long, default_value = "0")]
        fee: Amount,
    },
    /// Send money
    Send {
        #[structopt(long)]
        memo: Option<String>,
        #[structopt(long)]
        from: ZieshaAddress,
        #[structopt(long)]
        to: ZieshaAddress,
        #[structopt(long)]
        token: Option<usize>,
        #[structopt(long)]
        amount: Amount,
        #[structopt(long, default_value = "0")]
        fee: Amount,
    },
    /// Register your validator
    RegisterValidator {
        #[structopt(long)]
        memo: Option<String>,
        #[structopt(long, default_value = "0")]
        fee: Amount,
    },
    /// Delegate to a validator
    Delegate {
        #[structopt(long)]
        memo: Option<String>,
        #[structopt(long)]
        to: Address,
        #[structopt(long)]
        amount: Amount,
        #[structopt(long, default_value = "0")]
        fee: Amount,
    },
    /// Reclaim funds inside an ended delegatation back to your account
    ReclaimDelegate {
        #[structopt(long)]
        memo: Option<String>,
        #[structopt(long)]
        from: Address,
        #[structopt(long)]
        amount: Amount,
        #[structopt(long, default_value = "0")]
        fee: Amount,
    },
    /// Resets wallet nonces
    Reset {},
    /// Get info and balances of the wallet
    Info {},
    /// Resend pending transactions
    ResendPending {
        #[structopt(long)]
        fill_gaps: bool,
        #[structopt(long)]
        shift: bool,
    },
}

#[derive(StructOpt)]
#[allow(clippy::large_enum_variant)]
#[cfg(feature = "node")]
enum NodeCliOptions {
    /// Start the node
    Start {
        #[structopt(long)]
        client_only: bool,
        #[structopt(long)]
        discord_handle: Option<String>,
        #[structopt(long)]
        dev: bool,
        #[structopt(long)]
        dev_address: String,
    },
    /// Get status of a node
    Status {},
}

#[derive(StructOpt)]
#[allow(clippy::large_enum_variant)]
#[cfg(feature = "node")]
enum ChainCliOptions {
    /// Rollback the blockchain
    Rollback {},
    /// Query the underlying database
    DbQuery { prefix: String },
    /// Check health of the blockchain
    HealthCheck {},
}

#[derive(StructOpt)]
#[allow(clippy::large_enum_variant)]
#[cfg(feature = "client")]
#[structopt(name = "Bazuka!", about = "Node software for Ziesha Network")]
enum CliOptions {
    #[cfg(not(feature = "client"))]
    Init,
    #[cfg(feature = "client")]
    /// Initialize node/wallet
    Init {
        #[structopt(long, default_value = "mainnet")]
        network: String,
        #[structopt(long)]
        bootstrap: Vec<PeerAddress>,
        #[structopt(long)]
        mnemonic: Option<bip39::Mnemonic>,
        #[structopt(long)]
        listen: Option<SocketAddr>,
        #[structopt(long)]
        external: Option<PeerAddress>,
        #[structopt(long)]
        db: Option<PathBuf>,
    },

    #[cfg(feature = "node")]
    /// Node subcommand
    Node(NodeCliOptions),

    /// Wallet subcommand
    Wallet(WalletOptions),

    /// Chain subcommand
    Chain(ChainCliOptions),
}

enum KvStoreEnum {
    RamKvStore(RamKvStore),
    LevelDbKvStore(LevelDbKvStore)
}

#[cfg(feature = "node")]
async fn run_node(
    bazuka_config: BazukaConfig,
    wallet: Wallet,
    social_profiles: SocialProfiles,
    client_only: bool,
    dev: bool,
    dev_address: &str
) -> Result<(), NodeError> {
    let address = if client_only {
        None
    } else {
        Some(bazuka_config.external)
    };

    let wallet = TxBuilder::new(&wallet.seed());

    println!(
        "{} v{}",
        "Bazuka!".bright_green(),
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("{} {}", "Listening:".bright_yellow(), bazuka_config.listen);
    if let Some(addr) = &address {
        println!("{} {}", "Internet endpoint:".bright_yellow(), addr);
    }
    println!("{} {}", "Network:".bright_yellow(), bazuka_config.network);

    println!(
        "{} {}",
        "Wallet address:".bright_yellow(),
        wallet.get_address()
    );
    println!(
        "{} {}",
        "Wallet zk address:".bright_yellow(),
        wallet.get_zk_address()
    );
    println!(
        "{} {}",
        "Miner token:".bright_yellow(),
        bazuka_config.miner_token
    );

    let (inc_send, inc_recv) = mpsc::unbounded_channel::<NodeRequest>();
    let (out_send, mut out_recv) = mpsc::unbounded_channel::<NodeRequest>();

    let bootstrap_nodes = bazuka_config.bootstrap.clone();

    let bazuka_dir = bazuka_config.db.clone();
    let blockchain_config = if dev {
        config::blockchain::get_dev_blockchain_config(dev_address)
    } else {
        config::blockchain::get_blockchain_config()
    };
    let database = if dev {
        KvStoreEnum::RamKvStore(RamKvStore::new())
    } else {
        KvStoreEnum::LevelDbKvStore(LevelDbKvStore::new(&bazuka_dir, 64).unwrap())
    };
    let database = match database {
        KvStoreEnum::RamKvStore(store) => Box::new(store) as Box<dyn KvStore>,
        KvStoreEnum::LevelDbKvStore(store) => Box::new(store) as Box<dyn KvStore>,
    };


    // 60 request per minute / 4GB per 15min
    let firewall = Firewall::new(360, 4 * GB);

    // Async loop that is responsible for answering external requests and gathering
    // data from external world through a heartbeat loop.
    let node = node_create(
        config::node::get_node_options(),
        &bazuka_config.network,
        address,
        bootstrap_nodes,
        KvStoreChain::new(
            *database,
            blockchain_config,
        )
        .unwrap(),
        0,
        wallet,
        social_profiles,
        inc_recv,
        out_send,
        Some(firewall),
        Some(bazuka_config.miner_token.clone()),
    );

    // Async loop that is responsible for getting incoming HTTP requests through a
    // socket and redirecting it to the node channels.
    let server_loop = async {
        let arc_inc_send = Arc::new(inc_send);
        Server::bind(&bazuka_config.listen)
            .serve(make_service_fn(|conn: &AddrStream| {
                let client = conn.remote_addr();
                let arc_inc_send = Arc::clone(&arc_inc_send);
                async move {
                    Ok::<_, NodeError>(service_fn(move |req: Request<Body>| {
                        let arc_inc_send = Arc::clone(&arc_inc_send);
                        async move {
                            let (resp_snd, mut resp_rcv) =
                                mpsc::channel::<Result<Response<Body>, NodeError>>(1);
                            let req = NodeRequest {
                                limit: Limit::default(),
                                socket_addr: Some(client),
                                body: req,
                                resp: resp_snd,
                            };
                            arc_inc_send
                                .send(req)
                                .map_err(|_| NodeError::NotListeningError)?;
                            Ok::<Response<Body>, NodeError>(
                                match resp_rcv.recv().await.ok_or(NodeError::NotAnsweringError)? {
                                    Ok(resp) => resp,
                                    Err(e) => {
                                        let mut resp =
                                            Response::new(Body::from(format!("Error: {}", e)));
                                        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                                        resp
                                    }
                                },
                            )
                        }
                    }))
                }
            }))
            .await?;
        Ok::<(), NodeError>(())
    };

    // Async loop that is responsible for redirecting node requests from its outgoing
    // channel to the Internet and piping back the responses.
    let client_loop = async {
        while let Some(req) = out_recv.recv().await {
            tokio::spawn(async move {
                let resp = async {
                    let client = Client::new();
                    let resp = if let Some(time_limit) = req.limit.time {
                        tokio::time::timeout(time_limit, client.request(req.body)).await?
                    } else {
                        client.request(req.body).await
                    }?;
                    Ok::<_, NodeError>(resp)
                }
                .await;
                if let Err(e) = req.resp.send(resp).await {
                    log::debug!("Node not listening to its HTTP request answer: {}", e);
                }
            });
        }
        Ok::<(), NodeError>(())
    };

    try_join!(server_loop, client_loop, node).unwrap();

    Ok(())
}

#[cfg(feature = "client")]
fn generate_miner_token() -> String {
    use rand::distributions::Alphanumeric;
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}

fn get_wallet() -> Option<Wallet> {
    let wallet_path = get_wallet_path();
    let wallet = Wallet::open(wallet_path.clone()).unwrap();
    wallet
}

fn get_wallet_path() -> PathBuf {
    let wallet_path = home::home_dir().unwrap().join(Path::new(".bazuka-wallet"));
    wallet_path
}

fn get_conf_path() -> PathBuf {
    let conf_path = home::home_dir().unwrap().join(Path::new(".bazuka.yaml"));
    conf_path
}

fn get_conf() -> Option<BazukaConfig> {
    let conf_path = get_conf_path();
    let conf: Option<BazukaConfig> = std::fs::File::open(conf_path.clone())
        .ok()
        .map(|f| serde_yaml::from_reader(f).unwrap());
    conf
}

pub async fn initialize_cli() {
    let opts = CliOptions::from_args();

    let conf_path = get_conf_path();

    let mut conf = get_conf();

    if let Some(ref mut conf) = &mut conf {
        if conf.miner_token.is_empty() {
            conf.miner_token = generate_miner_token();
        }
        std::fs::write(conf_path.clone(), serde_yaml::to_string(conf).unwrap()).unwrap();
    }
    let conf = get_conf();
    let wallet = get_wallet();
    let wallet_path = get_wallet_path();

    match opts {
        #[cfg(feature = "node")]
        CliOptions::Chain(chain_opts) => match chain_opts {
            ChainCliOptions::Rollback {} => {
                crate::cli::chain::rollback(&conf.expect(BAZUKA_NOT_INITILIZED)).await;
            }
            ChainCliOptions::DbQuery { prefix } => {
                crate::cli::chain::db_query(prefix, &conf.expect(BAZUKA_NOT_INITILIZED));
            }
            ChainCliOptions::HealthCheck {} => {
                crate::cli::chain::health_check(&conf.expect(BAZUKA_NOT_INITILIZED));
            }
        },
        #[cfg(feature = "node")]
        CliOptions::Node(node_opts) => match node_opts {
            NodeCliOptions::Start {
                discord_handle,
                client_only,
                dev,
                dev_address
            } => {
                crate::cli::node::start(
                    discord_handle,
                    client_only,
                    &conf.expect(BAZUKA_NOT_INITILIZED),
                    &wallet.expect(BAZUKA_NOT_INITILIZED),
                    dev,
                    &dev_address
                )
                .await;
            }
            NodeCliOptions::Status {} => {
                crate::cli::node::status(get_conf(), get_wallet()).await;
            }
        },
        #[cfg(feature = "client")]
        CliOptions::Init {
            network,
            bootstrap,
            mnemonic,
            external,
            listen,
            db,
        } => {
            crate::cli::init(
                network,
                bootstrap,
                mnemonic,
                external,
                listen,
                db,
                conf,
                &conf_path,
                wallet,
                &wallet_path,
            )
            .await
        }
        #[cfg(not(feature = "client"))]
        CliOptions::Init { .. } => {
            println!("Client feature not turned on!");
        }
        CliOptions::Wallet(wallet_opts) => match wallet_opts {
            WalletOptions::AddToken { id } => {
                crate::cli::wallet::add_token(
                    id,
                    &mut wallet.expect(BAZUKA_NOT_INITILIZED),
                    &wallet_path,
                );
            }
            WalletOptions::NewToken {
                memo,
                name,
                symbol,
                supply,
                decimals,
                mintable,
                fee,
            } => {
                crate::cli::wallet::new_token(
                    memo,
                    name,
                    symbol,
                    supply,
                    decimals,
                    mintable,
                    fee,
                    get_conf(),
                    get_wallet(),
                    &wallet_path,
                )
                .await;
            }
            WalletOptions::Send {
                memo,
                from,
                to,
                amount,
                fee,
                token,
            } => {
                crate::cli::wallet::send(
                    memo,
                    from,
                    to,
                    amount,
                    fee,
                    token,
                    conf,
                    wallet,
                    &wallet_path,
                )
                .await;
            }
            WalletOptions::Reset {} => {
                crate::cli::wallet::reset(&mut wallet.expect(BAZUKA_NOT_INITILIZED), &wallet_path);
            }
            WalletOptions::RegisterValidator { memo, fee } => {
                crate::cli::wallet::register_validator(
                    memo,
                    fee,
                    get_conf(),
                    get_wallet(),
                    &get_wallet_path(),
                )
                .await;
            }
            WalletOptions::ReclaimDelegate { .. } => {
                unimplemented!();
            }
            WalletOptions::Delegate {
                memo,
                amount,
                to,
                fee,
            } => {
                crate::cli::wallet::delegate(memo, amount, to, fee).await;
            }
            WalletOptions::ResendPending { fill_gaps, shift } => {
                crate::cli::wallet::resend_pending(fill_gaps, shift).await;
            }
            WalletOptions::Info {} => {
                crate::cli::wallet::info(get_conf(), get_wallet()).await;
            }
        },
    }
}
