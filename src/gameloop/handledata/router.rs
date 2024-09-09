use super::{client_packet_mapper, game_server_packet_mapper};
use crate::{containers::Storage, gametypes::Result, socket::*};

pub async fn client_handle_data(
    storage: &mut Storage,
    data: &mut MByteBuffer,
    client: &mut Client,
) -> Result<()> {
    let id: ClientPacket = data.read()?;

    client_packet_mapper(storage, data, client, id).await
}

pub async fn game_server_handle_data(
    storage: &mut Storage,
    data: &mut MByteBuffer,
    server: &mut GameServer,
) -> Result<()> {
    let id: GameServerPacket = data.read()?;

    game_server_packet_mapper(storage, data, server, id).await
}
