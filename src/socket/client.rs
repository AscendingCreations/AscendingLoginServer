use crate::{client_handle_data, containers::Storage, gametypes::*, socket::*};
use log::{error, trace, warn};
use mio::{net::TcpStream, Interest};
use mmap_bytey::BUFFER_SIZE;
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
};

#[derive(Debug)]
pub struct Client {
    pub stream: TcpStream,
    pub token: mio::Token,
    pub state: SocketState,
    pub sends: VecDeque<MByteBuffer>,
    pub poll_state: SocketPollState,
    pub buffer: ByteBuffer,
    pub addr: String,
    // used for sending encrypted Data.
    pub tls: rustls::ServerConnection,
}

impl Client {
    #[inline]
    pub fn new(
        stream: TcpStream,
        token: mio::Token,
        addr: String,
        tls: rustls::ServerConnection,
    ) -> Result<Client> {
        Ok(Client {
            stream,
            token,
            state: SocketState::Open,
            sends: VecDeque::with_capacity(32),
            poll_state: SocketPollState::Read,
            buffer: ByteBuffer::with_capacity(16_000)?,
            tls,
            addr,
        })
    }

    pub async fn process(&mut self, event: &mio::event::Event, storage: &Storage) -> Result<()> {
        //We set it as None so we can fully control when to enable it again based on conditions.
        self.poll_state.set(SocketPollState::Read);

        // Check if the Event has some readable Data from the Poll State.
        if event.is_readable() {
            self.tls_read().await?;
        }

        // Check if the Event has some writable Data from the Poll State.
        if event.is_writable() {
            self.tls_write().await;
        }

        if !self.sends.is_empty() {
            self.poll_state.add(SocketPollState::Write);
        }

        // Check if the Socket is closing if not lets reregister the poll event for it.
        // if `SocketPollState::None` is registers as the poll event we will not get data.
        match self.state {
            SocketState::Closing => self.close_socket(storage).await?,
            _ => self.reregister(&*storage.poll.read().await)?,
        }

        Ok(())
    }

    #[inline]
    pub fn deregister(&mut self, poll: &mio::Poll) -> Result<()> {
        Ok(poll.registry().deregister(&mut self.stream)?)
    }

    #[inline]
    pub async fn set_to_closing(&mut self, storage: &Storage) -> Result<()> {
        self.state = SocketState::Closing;
        self.poll_state.add(SocketPollState::Write);
        self.reregister(&*storage.poll.read().await)
    }

    #[inline]
    pub async fn close_socket(&mut self, storage: &Storage) -> Result<()> {
        match self.state {
            SocketState::Closed => Ok(()),
            _ => {
                //We dont care about errors here as they only occur when a socket is already disconnected by the client.
                self.deregister(&*storage.poll.read().await)?;
                let _ = self.stream.shutdown(std::net::Shutdown::Both);
                self.state = SocketState::Closed;

                Ok(())
            }
        }
    }

    pub async fn tls_read(&mut self) -> Result<()> {
        let pos = self.buffer.cursor();

        self.buffer.move_cursor_to_end();

        loop {
            match self.tls.read_tls(&mut self.stream) {
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(ref err) if err.kind() == io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(error) => {
                    error!("TLS read error: {:?}", error);
                    self.state = SocketState::Closing;
                    return Ok(());
                }
                Ok(0) => {
                    trace!("Client side socket closed");
                    self.state = SocketState::Closing;
                    return Ok(());
                }
                Ok(_) => {}
            }

            let io_state = match self.tls.process_new_packets() {
                Ok(io_state) => io_state,
                Err(err) => {
                    error!("TLS error: {:?}", err);
                    self.state = SocketState::Closing;
                    return Ok(());
                }
            };

            if io_state.plaintext_bytes_to_read() > 0 {
                let mut buf = vec![0u8; io_state.plaintext_bytes_to_read()];
                if let Err(e) = self.tls.reader().read_exact(&mut buf) {
                    trace!("TLS read error: {}", e);
                    self.state = SocketState::Closing;
                    return Ok(());
                }

                if let Err(e) = self.buffer.write_slice(&buf) {
                    trace!("TLS read error: {}", e);
                    self.state = SocketState::Closing;
                    return Ok(());
                }
            }

            if io_state.peer_has_closed() {
                trace!("TLS peer has closed");
                self.state = SocketState::Closing;
            }

            break;
        }

        // reset it back to the original pos so we can Read from it again.
        self.buffer.move_cursor(pos)?;

        if self.buffer.is_empty() {
            self.poll_state.add(SocketPollState::Read);
        }

        Ok(())
    }

    pub async fn tls_write(&mut self) {
        // lets only send 25 packets per socket each loop.
        loop {
            let mut packet = match self.sends.pop_front() {
                Some(packet) => packet,
                None => {
                    if self.sends.capacity() > 100 {
                        warn!(
                            "Socket TLS write: tls_sends Buffer Strink to 100, Current Capacity {}, Current len {}.",
                            self.sends.capacity(),
                            self.sends.len()
                        );
                        self.sends.shrink_to(100);
                    }
                    break;
                }
            };

            match self.tls.writer().write_all(packet.as_slice()) {
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                    self.sends.push_front(packet);
                    break;
                }
                Err(e) => {
                    trace!("tls write, error in write_all: {}", e);
                    self.state = SocketState::Closing;
                    return;
                }
                Ok(_) => {}
            }
        }

        loop {
            if self.tls.wants_write() {
                match self.tls.write_tls(&mut self.stream) {
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(ref err) if err.kind() == io::ErrorKind::Interrupted => {
                        continue;
                    }
                    Err(e) => {
                        trace!("tls write, error in write_tls: {}", e);
                        self.state = SocketState::Closing;
                        return;
                    }
                    Ok(_) => {}
                };
            } else {
                break;
            }
        }

        if !self.sends.is_empty() {
            self.poll_state.add(SocketPollState::Write);
        }
    }

    #[inline]
    pub fn event_set(&mut self) -> Option<Interest> {
        match self.poll_state {
            SocketPollState::None => None,
            SocketPollState::Read => Some(Interest::READABLE),
            SocketPollState::Write => Some(Interest::WRITABLE),
            SocketPollState::ReadWrite => Some(Interest::READABLE.add(Interest::WRITABLE)),
        }
    }

    #[inline]
    pub fn register(&mut self, poll: &mio::Poll) -> Result<()> {
        if let Some(interest) = self.event_set() {
            poll.registry()
                .register(&mut self.stream, self.token, interest)?;
        }
        Ok(())
    }

    #[inline]
    pub fn reregister(&mut self, poll: &mio::Poll) -> Result<()> {
        if let Some(interest) = self.event_set() {
            poll.registry()
                .reregister(&mut self.stream, self.token, interest)?;
        }
        Ok(())
    }

    #[inline]
    pub fn send(&mut self, poll: &mio::Poll, buf: MByteBuffer) -> Result<()> {
        self.sends.push_back(buf);
        self.add_write_state(poll)
    }

    #[inline]
    pub fn add_write_state(&mut self, poll: &mio::Poll) -> Result<()> {
        if !self.poll_state.contains(SocketPollState::Write) {
            self.poll_state.add(SocketPollState::Write);
            self.reregister(poll)?;
        }

        Ok(())
    }
}

pub async fn get_length(storage: &Storage, client: &mut Client) -> Result<Option<u64>> {
    if client.buffer.length() - client.buffer.cursor() >= 8 {
        let length = client.buffer.read::<u64>()?;

        if !(1..=8192).contains(&length) {
            trace!("Player was disconnected on get_length LENGTH: {:?}", length);
            client.set_to_closing(storage).await?;
            return Ok(None);
        }

        Ok(Some(length))
    } else {
        client.poll_state.add(SocketPollState::Read);
        client.reregister(&*storage.poll.read().await)?;

        Ok(None)
    }
}

pub const MAX_PROCESSED_PACKETS: i32 = 25;

pub async fn process_client_packets(storage: &mut Storage) -> Result<()> {
    let mut packet = MByteBuffer::new()?;
    let client_ids = storage.client_ids.clone();

    'user_loop: for token in &client_ids {
        let client = storage.server.read().await.clients.get(token).cloned();

        if let Some(client) = client {
            let mut client = client.lock().await;
            let mut count = 0;

            loop {
                packet.move_cursor_to_start();
                let length = match get_length(storage, &mut client).await? {
                    Some(n) => n,
                    None => {
                        socket_update(storage, &mut client, false).await?;
                        break;
                    }
                };

                if length == 0 {
                    trace!(
                        "Length was Zero. Bad or malformed packet from IP: {}",
                        client.addr
                    );

                    socket_update(storage, &mut client, true).await?;
                    continue 'user_loop;
                }

                if length > BUFFER_SIZE as u64 {
                    trace!(
                        "Length was {} greater than the max packet size of {}. Bad or malformed packet from IP: {}",
                        length,
                        client.addr,
                        BUFFER_SIZE
                    );

                    socket_update(storage, &mut client, true).await?;
                    continue 'user_loop;
                }

                if length <= (client.buffer.length() - client.buffer.cursor()) as u64 {
                    let mut errored = false;

                    if let Ok(bytes) = client.buffer.read_slice(length as usize) {
                        if packet.write_slice(bytes).is_err() {
                            errored = true;
                        }

                        packet.move_cursor_to_start();
                    } else {
                        errored = true;
                    }

                    if errored {
                        warn!(
                            "IP: {} was disconnected due to error on packet length.",
                            client.addr
                        );
                        socket_update(storage, &mut client, true).await?;
                        continue 'user_loop;
                    }

                    if client_handle_data(storage, &mut packet, &mut client)
                        .await
                        .is_err()
                    {
                        warn!(
                            "IP: {} was disconnected due to invalid packets",
                            client.addr
                        );
                        socket_update(storage, &mut client, true).await?;
                        continue 'user_loop;
                    }

                    count += 1
                } else {
                    let cursor = client.buffer.cursor() - 8;
                    client.buffer.move_cursor(cursor)?;
                    socket_update(storage, &mut client, false).await?;
                    break;
                }

                if count == MAX_PROCESSED_PACKETS {
                    break;
                }
            }
        }
    }

    Ok(())
}

pub async fn socket_update(
    storage: &mut Storage,
    client: &mut Client,
    should_close: bool,
) -> Result<()> {
    storage.client_ids.swap_remove(&client.token);

    if should_close {
        client.set_to_closing(storage).await?;
    }

    Ok(())
}
