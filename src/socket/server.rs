use crate::{
    containers::{HashMap, Storage},
    gametypes::Result,
    socket::{Client, SocketState},
};
use log::{trace, warn};
use mio::{net::TcpListener, Events, Poll};
use std::{collections::VecDeque, io, net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::Mutex;

use super::GameServer;

pub const CLIENT_SERVER: mio::Token = mio::Token(0);
pub const GAME_SERVER: mio::Token = mio::Token(1);

pub struct Server {
    pub client_listener: TcpListener,
    pub server_listener: TcpListener,
    pub clients: HashMap<mio::Token, Arc<Mutex<Client>>>,
    pub game_servers: HashMap<mio::Token, Arc<Mutex<GameServer>>>,
    pub tokens: VecDeque<mio::Token>,
    pub tls_config: Arc<rustls::ServerConfig>,
}

impl Server {
    #[inline]
    pub fn new(
        poll: &mut Poll,
        addr: &str,
        clients_port: u16,
        servers_port: u16,
        max: usize,
        cfg: Arc<rustls::ServerConfig>,
    ) -> Result<Server> {
        assert_ne!(
            clients_port, servers_port,
            "Server Socket and Client Socket can not have the same Port."
        );

        /* Create a bag of unique tokens. */
        let mut tokens = VecDeque::with_capacity(max);

        for i in 2..max {
            tokens.push_back(mio::Token(i));
        }

        /* Set up the Clients TCP TLS listener. */
        let client_addr = SocketAddr::new(addr.parse()?, clients_port);
        let mut client_listener = TcpListener::bind(client_addr)?;

        poll.registry()
            .register(&mut client_listener, CLIENT_SERVER, mio::Interest::READABLE)?;

        /* Set up the Game Servers TCP listener. */
        let game_server_addr = SocketAddr::new(addr.parse()?, servers_port);
        let mut server_listener = TcpListener::bind(game_server_addr)?;

        poll.registry()
            .register(&mut server_listener, GAME_SERVER, mio::Interest::READABLE)?;

        Ok(Server {
            client_listener,
            server_listener,
            clients: HashMap::default(),
            game_servers: HashMap::default(),
            tokens,
            tls_config: cfg,
        })
    }

    pub async fn accept_client(&mut self, storage: &mut Storage) -> Result<()> {
        /* Wait for a new connection to accept and try to grab a token from the bag. */
        loop {
            let (stream, addr) = match self.client_listener.accept() {
                Ok((stream, addr)) => (stream, addr),
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    trace!("listener.accept error: {}", e);
                    return Err(e.into());
                }
            };

            stream.set_nodelay(true)?;

            if let Some(token) = self.tokens.pop_front() {
                let tls_conn = rustls::ServerConnection::new(Arc::clone(&self.tls_config))?;
                // Lets make the Client to handle hwo we send packets.
                let mut client = Client::new(stream, token, addr.to_string(), tls_conn)?;
                //Register the Poll to the client for recv and Sending
                client.register(&*storage.poll.read().await)?;

                // insert client into handled list.
                self.clients.insert(token, Arc::new(Mutex::new(client)));
            } else {
                warn!("listener.accept No tokens left to give out.");
                drop(stream);
            }
        }
        Ok(())
    }

    pub async fn accept_server(&mut self, storage: &mut Storage) -> Result<()> {
        /* Wait for a new connection to accept and try to grab a token from the bag. */
        loop {
            let (stream, addr) = match self.server_listener.accept() {
                Ok((stream, addr)) => (stream, addr),
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    trace!("listener.accept error: {}", e);
                    return Err(e.into());
                }
            };

            stream.set_nodelay(true)?;

            if let Some(token) = self.tokens.pop_front() {
                // Lets make the game_server to handle how we send packets.
                let mut game_server = GameServer::new(stream, token, addr.to_string())?;
                //Register the Poll to the client for recv and Sending
                game_server.register(&*storage.poll.read().await)?;

                // insert client into handled list.
                self.game_servers
                    .insert(token, Arc::new(Mutex::new(game_server)));
            } else {
                warn!("listener.accept No tokens left to give out.");
                drop(stream);
            }
        }
        Ok(())
    }

    #[inline]
    pub fn remove(&mut self, token: mio::Token) {
        /* If the token is valid, let's remove the connection and add the token back to the bag. */
        if self.clients.contains_key(&token) {
            self.clients.remove(&token);
            self.tokens.push_front(token);
        }
    }
}

pub async fn poll_events(storage: &mut Storage) -> Result<()> {
    let mut events = Events::with_capacity(1024);

    storage
        .poll
        .write()
        .await
        .poll(&mut events, Some(Duration::from_millis(0)))?;

    for event in events.iter() {
        match event.token() {
            CLIENT_SERVER => {
                let server = storage.server.clone();
                server.write().await.accept_client(storage).await?;
                storage.poll.read().await.registry().reregister(
                    &mut server.write().await.client_listener,
                    CLIENT_SERVER,
                    mio::Interest::READABLE,
                )?;
            }
            GAME_SERVER => {
                let server = storage.server.clone();
                server.write().await.accept_server(storage).await?;
                storage.poll.read().await.registry().reregister(
                    &mut server.write().await.server_listener,
                    GAME_SERVER,
                    mio::Interest::READABLE,
                )?;
            }
            token => {
                let mut server = storage.server.write().await;
                let state = if let Some(a) = server.clients.get(&token) {
                    let mut client = a.lock().await;
                    client.process(event, storage).await?;
                    client.state
                } else {
                    trace!("a token no longer exists within clients.");
                    SocketState::Closed
                };

                if state == SocketState::Closed {
                    server.remove(token);
                };
            }
        }
    }

    Ok(())
}
