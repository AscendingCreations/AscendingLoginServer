use crate::{containers::Storage, gametypes::*, socket::*};

#[inline]
pub async fn send_infomsg(
    storage: &mut Storage,
    client: &mut Client,
    message: String,
    close_socket: bool,
) -> Result<()> {
    let mut buf = MByteBuffer::new_packet()?;

    buf.write(ServerToClientPackets::AlertMsg)?;
    buf.write(message)?;
    buf.write(close_socket)?;
    buf.finish()?;

    client.send(&*storage.poll.read().await, buf)
}

#[inline]
pub async fn send_fltalert(
    storage: &mut Storage,
    client: &mut Client,
    message: String,
    ftltype: FtlType,
) -> Result<()> {
    let mut buf = MByteBuffer::new_packet()?;

    buf.write(ServerToClientPackets::FltAlert)?;
    buf.write(ftltype)?;
    buf.write(message)?;
    buf.finish()?;

    client.send(&*storage.poll.read().await, buf)
}

#[inline]
pub async fn send_verification(
    storage: &mut Storage,
    game_server: &mut GameServer,
    id: i64,
    verified: bool,
) -> Result<()> {
    let mut buf = MByteBuffer::new_packet()?;

    buf.write(ServerToServerPackets::Verification)?;
    buf.write(id)?;
    buf.write(verified)?;
    buf.finish()?;

    game_server.send(&*storage.poll.read().await, buf)
}

#[inline]
pub async fn send_server_list(storage: &mut Storage, client: &mut Client) -> Result<()> {
    let per_packet = 5;

    for i in 0..(storage.servers.len() / per_packet) + 1 {
        let mut buf = MByteBuffer::new_packet_with_count(ServerToClientPackets::ServerList as u16)?;
        let mut count = 0;

        for id in i * per_packet..i * per_packet + per_packet {
            if let Some((_, server_info)) = storage.servers.get_index(id) {
                count += 1;
                buf.write(&server_info.name)?;
                buf.write(&server_info.ip)?;
                buf.write(server_info.port)?;
                buf.write(server_info.players_on)?;
                buf.write(server_info.max_players)?;
            } else {
                // we reached the end.
                buf.finish_with_count(count)?;
                return client.send(&*storage.poll.read().await, buf);
            }
        }

        buf.finish_with_count(count)?;
        client.send(&*storage.poll.read().await, buf)?;
    }

    Ok(())
}

#[inline]
pub async fn send_login(
    storage: &mut Storage,
    client: &mut Client,
    token: String,
    relogin_code: &str,
) -> Result<()> {
    let mut buf = MByteBuffer::new_packet()?;

    buf.write(ServerToClientPackets::Login)?;
    buf.write(token)?;
    buf.write(relogin_code)?;
    buf.finish()?;

    client.send(&*storage.poll.read().await, buf)
}

pub async fn send_game_server_online_check(
    storage: &mut Storage,
    game_server: &mut GameServer,
) -> Result<()> {
    let mut buf = MByteBuffer::new_packet()?;

    buf.write(ServerToServerPackets::OnlineCheck)?;
    buf.finish()?;

    game_server.send(&*storage.poll.read().await, buf)
}

pub async fn send_kill_client(
    storage: &mut Storage,
    game_server: &mut GameServer,
    uid: i64,
) -> Result<()> {
    let mut buf = MByteBuffer::new_packet()?;

    buf.write(ServerToServerPackets::KillClient)?;
    buf.write(uid)?;
    buf.finish()?;

    game_server.send(&*storage.poll.read().await, buf)
}
