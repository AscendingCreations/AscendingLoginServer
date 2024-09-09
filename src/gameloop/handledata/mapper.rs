use super::routes;
use crate::{containers::Storage, gametypes::*, socket::*};

pub async fn client_packet_mapper(
    storage: &mut Storage,
    data: &mut MByteBuffer,
    client: &mut Client,
    id: ClientPacket,
) -> Result<()> {
    match id {
        ClientPacket::Register => routes::handle_register(storage, data, client).await,
        ClientPacket::Login => routes::handle_login(storage, data, client).await,
        ClientPacket::RequestServers => {
            routes::handle_server_list_request(storage, data, client).await
        }
        ClientPacket::OnlineCheck => Ok(()),
        _ => Ok(()),
    }
}

pub async fn game_server_packet_mapper(
    storage: &mut Storage,
    data: &mut MByteBuffer,
    game_server: &mut GameServer,
    id: GameServerPacket,
) -> Result<()> {
    match id {
        GameServerPacket::Verification => {
            routes::handle_verification(storage, data, game_server).await
        }
        GameServerPacket::UpdateInfo => {
            routes::handle_update_server_info(storage, data, game_server).await
        }
        GameServerPacket::UpdateCount => {
            routes::handle_update_server_count(storage, data, game_server).await
        }
        GameServerPacket::OnlineCheck => Ok(()),
    }
}
