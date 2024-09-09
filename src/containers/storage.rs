use crate::{
    containers::{IndexMap, IndexSet},
    gametypes::*,
    socket::Server,
};
use log::LevelFilter;
use mio::Poll;
use mmap_bytey::{MByteBufferRead, MByteBufferWrite};
use rustls::{
    crypto::{ring as provider, CryptoProvider},
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConfig,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, PgPool,
};
use std::{fs, io::BufReader, sync::Arc};
use tokio::sync::RwLock;

use super::{RotatableJwtKey, KEY_LENGTH};

#[derive(Clone, Debug, Serialize, Deserialize, MByteBufferRead, MByteBufferWrite)]
pub struct GameServerInfo {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub players_on: u64,
    pub max_players: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, MByteBufferRead, MByteBufferWrite)]
pub struct UserClaim {
    pub server_name: String,
    pub uid: i64,
}

pub struct Storage {
    pub servers: IndexMap<mio::Token, GameServerInfo>,
    pub server_names: IndexMap<String, mio::Token>,
    pub client_ids: IndexSet<mio::Token>,
    pub servers_ids: IndexSet<mio::Token>,
    pub poll: RwLock<mio::Poll>,
    pub server: Arc<RwLock<Server>>,
    pub pgconn: PgPool,
    pub config: Config,
    pub keys: RotatableJwtKey,
}

async fn establish_connection(config: &Config) -> Result<PgPool> {
    let mut connect_opts = PgConnectOptions::new();
    connect_opts = connect_opts.log_statements(log::LevelFilter::Debug);
    connect_opts = connect_opts.database(&config.database);
    connect_opts = connect_opts.username(&config.username);
    connect_opts = connect_opts.password(&config.password);
    connect_opts = connect_opts.host(&config.host);
    connect_opts = connect_opts.port(config.port);

    Ok(PgPoolOptions::new()
        .max_connections(5)
        .connect_with(connect_opts)
        .await?)
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerLevelFilter {
    /// A level lower than all log levels.
    Off,
    /// Corresponds to the `Error` log level.
    Error,
    /// Corresponds to the `Warn` log level.
    Warn,
    /// Corresponds to the `Info` log level.
    Info,
    /// Corresponds to the `Debug` log level.
    Debug,
    /// Corresponds to the `Trace` log level.
    Trace,
}

impl ServerLevelFilter {
    pub fn parse_enum(&self) -> LevelFilter {
        match self {
            ServerLevelFilter::Off => LevelFilter::Off,
            ServerLevelFilter::Error => LevelFilter::Error,
            ServerLevelFilter::Warn => LevelFilter::Warn,
            ServerLevelFilter::Info => LevelFilter::Info,
            ServerLevelFilter::Debug => LevelFilter::Debug,
            ServerLevelFilter::Trace => LevelFilter::Trace,
        }
    }
}

#[derive(Deserialize)]
pub struct Config {
    pub listen: String,
    pub clients_port: u16,
    pub servers_port: u16,
    pub server_cert: String,
    pub server_key: String,
    pub ca_root: String,
    pub maxconnections: usize,
    pub database: String,
    pub username: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub enable_backtrace: bool,
    pub level_filter: ServerLevelFilter,
}

pub fn read_config(path: &str) -> Config {
    let data = fs::read_to_string(path).unwrap();
    toml::from_str(&data).unwrap()
}

fn load_certs(filename: &str) -> Vec<CertificateDer<'static>> {
    let certfile = fs::File::open(filename).expect("cannot open certificate file");
    let mut reader = BufReader::new(certfile);
    rustls_pemfile::certs(&mut reader)
        .map(|result| result.unwrap())
        .collect()
}

fn load_private_key(filename: &str) -> PrivateKeyDer<'static> {
    let keyfile = fs::File::open(filename).expect("cannot open private key file");
    let mut reader = BufReader::new(keyfile);

    loop {
        match rustls_pemfile::read_one(&mut reader).expect("cannot parse private key .pem file") {
            Some(rustls_pemfile::Item::Pkcs1Key(key)) => return key.into(),
            Some(rustls_pemfile::Item::Pkcs8Key(key)) => return key.into(),
            Some(rustls_pemfile::Item::Sec1Key(key)) => return key.into(),
            None => break,
            _ => {}
        }
    }

    panic!(
        "no keys found in {:?} (encrypted keys not supported)",
        filename
    );
}

fn build_tls_config(
    server_certs_path: &str,
    server_key_path: &str,
    _ca_root_path: &str,
) -> Result<Arc<rustls::ServerConfig>> {
    let certs = load_certs(server_certs_path);
    let private_key = load_private_key(server_key_path);

    let config = ServerConfig::builder_with_provider(
        CryptoProvider {
            cipher_suites: provider::ALL_CIPHER_SUITES.to_vec(),
            ..provider::default_provider()
        }
        .into(),
    )
    .with_protocol_versions(rustls::ALL_VERSIONS)
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(certs, private_key)?;

    Ok(Arc::new(config))
}

impl Storage {
    pub async fn new(config: Config) -> Option<Self> {
        assert_ne!(
            config.clients_port, config.servers_port,
            "Client Port and Server Port can not be the same."
        );

        let mut poll = Poll::new().ok()?;
        let tls_config =
            build_tls_config(&config.server_cert, &config.server_key, &config.ca_root).unwrap();
        let server = Server::new(
            &mut poll,
            &config.listen,
            config.clients_port,
            config.servers_port,
            config.maxconnections,
            tls_config,
        )
        .ok()?;

        let pgconn = establish_connection(&config).await.unwrap();
        crate::sql::initiate(&pgconn).await.unwrap();
        let keys = RotatableJwtKey::new(KEY_LENGTH);

        Some(Self {
            servers: IndexMap::default(),
            server_names: IndexMap::default(),
            client_ids: IndexSet::default(),
            servers_ids: IndexSet::default(),
            poll: RwLock::new(poll),
            server: Arc::new(RwLock::new(server)),
            pgconn,
            config,
            keys,
        })
    }
}
