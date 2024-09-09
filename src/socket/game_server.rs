use crate::{containers::Storage, game_server_handle_data, gametypes::*, socket::*};
use log::{trace, warn};
use mio::{net::TcpStream, Interest};
use mmap_bytey::BUFFER_SIZE;
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SocketState {
    Open,
    Closing,
    Closed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SocketPollState {
    None,
    Read,
    Write,
    ReadWrite,
}

impl SocketPollState {
    #[inline]
    pub fn add(&mut self, state: SocketPollState) {
        match (*self, state) {
            (SocketPollState::None, _) => *self = state,
            (SocketPollState::Read, SocketPollState::Write) => *self = SocketPollState::ReadWrite,
            (SocketPollState::Write, SocketPollState::Read) => *self = SocketPollState::ReadWrite,
            (_, _) => {}
        }
    }

    #[inline]
    pub fn set(&mut self, state: SocketPollState) {
        *self = state;
    }

    #[inline]
    pub fn remove(&mut self, state: SocketPollState) {
        match (*self, state) {
            (SocketPollState::Read, SocketPollState::Read) => *self = SocketPollState::None,
            (SocketPollState::Write, SocketPollState::Write) => *self = SocketPollState::None,
            (SocketPollState::ReadWrite, SocketPollState::Write) => *self = SocketPollState::Read,
            (SocketPollState::ReadWrite, SocketPollState::Read) => *self = SocketPollState::Write,
            (_, SocketPollState::ReadWrite) => *self = SocketPollState::None,
            (_, _) => {}
        }
    }

    pub fn contains(&mut self, state: SocketPollState) -> bool {
        ((*self == SocketPollState::Read || *self == SocketPollState::ReadWrite)
            && (state == SocketPollState::Read || state == SocketPollState::ReadWrite))
            || ((*self == SocketPollState::Write || *self == SocketPollState::ReadWrite)
                && (state == SocketPollState::Write || state == SocketPollState::ReadWrite))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EncryptionState {
    /// Send Unencrypted packets only.
    None,
    /// Send Encrypted for both read and write.
    ReadWrite,
    ///Migrating from encrypted to unencrypted after the last send.
    ///Read will start to read unencrypted traffic at this point.
    ///Only call this when we send the last nagotiation packet.
    WriteTransfering,
}

#[derive(Debug)]
pub struct GameServer {
    pub stream: TcpStream,
    pub token: mio::Token,
    pub state: SocketState,
    pub sends: VecDeque<MByteBuffer>,
    pub poll_state: SocketPollState,
    pub buffer: ByteBuffer,
    pub addr: String,
}

impl GameServer {
    #[inline]
    pub fn new(stream: TcpStream, token: mio::Token, addr: String) -> Result<GameServer> {
        Ok(GameServer {
            stream,
            token,
            state: SocketState::Open,
            sends: VecDeque::with_capacity(32),
            poll_state: SocketPollState::Read,
            buffer: ByteBuffer::with_capacity(16_000)?,
            addr,
        })
    }

    pub async fn process(
        &mut self,
        event: &mio::event::Event,
        storage: &mut Storage,
    ) -> Result<()> {
        //We set it as None so we can fully control when to enable it again based on conditions.
        self.poll_state.set(SocketPollState::Read);

        // Check if the Event has some readable Data from the Poll State.
        if event.is_readable() {
            self.read(storage).await?;
        }

        // Check if the Event has some writable Data from the Poll State.
        if event.is_writable() {
            self.write().await;
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

    pub async fn read(&mut self, storage: &mut Storage) -> Result<()> {
        // get the current pos so we can reset it back for reading.

        let pos = self.buffer.cursor();
        self.buffer.move_cursor_to_end();

        let mut buf: [u8; 4096] = [0; 4096];
        let mut closing = false;

        loop {
            match self.stream.read(&mut buf) {
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(ref err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Ok(0) => closing = true,
                Err(e) => {
                    trace!("stream.read, error in socket read: {}", e);
                    closing = true;
                }
                Ok(n) => {
                    if let Err(e) = self.buffer.write_slice(&buf[0..n]) {
                        trace!("buffer.write_slice, error in socket read: {}", e);
                        closing = true;
                    }
                }
            }

            if closing {
                // We are closing the socket so we dont need to handle it again.
                self.state = SocketState::Closing;
                return Ok(());
            }
        }

        // reset it back to the original pos so we can Read from it again.
        self.buffer.move_cursor(pos)?;

        if !self.buffer.is_empty() {
            storage.servers_ids.insert(self.token);
        } else {
            // we are not going to handle any reads so lets mark it back as read again so it can
            //continue to get packets.
            self.poll_state.add(SocketPollState::Read);
        }

        Ok(())
    }

    pub async fn write(&mut self) {
        let mut count: usize = 0;

        //info!("Player sends count: {}", self.sends.len());
        // lets only send 25 packets per socket each loop.
        while count < 25 {
            let mut packet = match self.sends.pop_front() {
                Some(packet) => packet,
                None => {
                    if self.sends.capacity() > 100 && self.sends.len() < 50 {
                        warn!(
                            "Socket write: sends Buffer Strink to 100, Current Capacity {}, Current len {}.",
                            self.sends.capacity(),
                            self.sends.len()
                        );
                        self.sends.shrink_to(100);
                    }
                    return;
                }
            };

            match self.stream.write_all(packet.as_slice()) {
                Ok(()) => count += 1,
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                    //Operation would block so we insert it back in to try again later.
                    self.sends.push_front(packet);
                    break;
                }
                Err(e) => {
                    trace!("stream.write_all error in socket write: {}", e);
                    self.state = SocketState::Closing;
                    return;
                }
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

#[inline]
pub async fn send_to_client(
    storage: &mut Storage,
    token: mio::Token,
    buf: MByteBuffer,
) -> Result<()> {
    if let Some(client) = storage.server.read().await.clients.get(&token) {
        client.lock().await.send(&*storage.poll.read().await, buf)
    } else {
        Ok(())
    }
}

pub async fn get_length(storage: &Storage, game_server: &mut GameServer) -> Result<Option<u64>> {
    if game_server.buffer.length() - game_server.buffer.cursor() >= 8 {
        let length = game_server.buffer.read::<u64>()?;

        if !(1..=8192).contains(&length) {
            trace!("Player was disconnected on get_length LENGTH: {:?}", length);
            game_server.set_to_closing(storage).await?;
            return Ok(None);
        }

        Ok(Some(length))
    } else {
        game_server.poll_state.add(SocketPollState::Read);
        game_server.reregister(&*storage.poll.read().await)?;

        Ok(None)
    }
}

pub const MAX_PROCESSED_PACKETS: i32 = 25;

pub async fn process_packets(storage: &mut Storage) -> Result<()> {
    let mut packet = MByteBuffer::new()?;
    let server_ids = storage.servers_ids.clone();

    'user_loop: for token in &server_ids {
        let game_server = storage.server.read().await.game_servers.get(token).cloned();

        if let Some(game_server) = game_server {
            let mut game_server = game_server.lock().await;
            let mut count = 0;

            loop {
                packet.move_cursor_to_start();
                let length = match get_length(storage, &mut game_server).await? {
                    Some(n) => n,
                    None => {
                        socket_update(storage, &mut game_server, false).await?;
                        break;
                    }
                };

                if length == 0 {
                    trace!(
                        "Length was Zero. Bad or malformed packet from IP: {}",
                        game_server.addr
                    );

                    socket_update(storage, &mut game_server, true).await?;
                    continue 'user_loop;
                }

                if length > BUFFER_SIZE as u64 {
                    trace!(
                        "Length was {} greater than the max packet size of {}. Bad or malformed packet from IP: {}",
                        length,
                        game_server.addr,
                        BUFFER_SIZE
                    );

                    socket_update(storage, &mut game_server, true).await?;
                    continue 'user_loop;
                }

                if length <= (game_server.buffer.length() - game_server.buffer.cursor()) as u64 {
                    let mut errored = false;

                    if let Ok(bytes) = game_server.buffer.read_slice(length as usize) {
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
                            game_server.addr
                        );
                        socket_update(storage, &mut game_server, true).await?;
                        continue 'user_loop;
                    }

                    if game_server_handle_data(storage, &mut packet, &mut game_server)
                        .await
                        .is_err()
                    {
                        warn!(
                            "IP: {} was disconnected due to invalid packets",
                            game_server.addr
                        );
                        socket_update(storage, &mut game_server, true).await?;
                        continue 'user_loop;
                    }

                    count += 1
                } else {
                    let cursor = game_server.buffer.cursor() - 8;
                    game_server.buffer.move_cursor(cursor)?;
                    socket_update(storage, &mut game_server, false).await?;
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
    game_server: &mut GameServer,
    should_close: bool,
) -> Result<()> {
    storage.servers_ids.swap_remove(&game_server.token);

    if should_close {
        game_server.set_to_closing(storage).await?;
    }

    Ok(())
}
