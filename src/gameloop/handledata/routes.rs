use crate::{
    containers::{Storage, UserClaim},
    gametypes::*,
    players::*,
    socket::*,
    sql::*,
};
use jsonwebtoken::{Algorithm, Header, Validation};
use log::info;
use rand::distributions::{Alphanumeric, DistString};
use regex::Regex;

pub async fn handle_register(
    storage: &mut Storage,
    data: &mut MByteBuffer,
    client: &mut Client,
) -> Result<()> {
    let username = data.read::<String>()?;
    let password = data.read::<String>()?;
    let email = data.read::<String>()?;
    let sprite_id = data.read::<u8>()?;
    let appmajor = data.read::<u16>()? as usize;
    let appminior = data.read::<u16>()? as usize;
    let apprevision = data.read::<u16>()? as usize;
    let server_name = data.read::<String>()?;

    if APP_MAJOR > appmajor && APP_MINOR > appminior && APP_REVISION > apprevision {
        return send_infomsg(storage, client, "Client needs to be updated.".into(), true).await;
    }

    let email_regex = Regex::new(
        r"^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,6})",
    )?;

    if !username.chars().all(is_name_acceptable) || !password.chars().all(is_password_acceptable) {
        return send_infomsg(
            storage,
            client,
            "Username or Password contains unaccepted Characters".into(),
            true,
        )
        .await;
    }

    if username.len() >= 64 {
        return send_infomsg(
            storage,
            client,
            "Username has too many Characters, 64 Characters Max".into(),
            true,
        )
        .await;
    }

    if password.len() >= 128 {
        return send_infomsg(
            storage,
            client,
            "Password has too many Characters, 128 Characters Max".into(),
            true,
        )
        .await;
    }

    if !email_regex.is_match(&email) || sprite_id >= 6 {
        return send_infomsg(
            storage,
            client,
            "Email must be an actual email.".into(),
            true,
        )
        .await;
    }

    match check_existance(storage, &username, &email).await {
        Ok(i) => match i {
            0 => {}
            1 => {
                return send_infomsg(
                    storage,
                    client,
                    "Username Exists. Please try Another.".into(),
                    true,
                )
                .await;
            }
            2 => {
                return send_infomsg(
                    storage,
                    client,
                    "Email Already Exists. Please Try Another.".into(),
                    true,
                )
                .await;
            }
            _ => return Err(AscendingError::RegisterFail),
        },
        Err(_) => return Err(AscendingError::UserNotFound),
    }

    let code = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

    let mut player = Player::default();
    player.username.clone_from(&username);
    player.email.clone_from(&email);
    player.sprite = sprite_id as u16;
    player.code.clone_from(&code);

    info!(
        "New Player {} with IP {}, sending Login Token.",
        &username, &client.addr
    );

    match new_player(storage, client, &player, password).await {
        Ok(uid) => {
            let claim = UserClaim { server_name, uid };

            let token = storage
                .keys
                .encode(&Header::new(Algorithm::HS512), &claim)?;

            send_login(storage, client, token, &code).await
        }
        Err(_) => {
            send_infomsg(
                storage,
                client,
                "There was an Issue Creating the player account. Please Contact Support.".into(),
                true,
            )
            .await
        }
    }
}

pub async fn handle_login(
    storage: &mut Storage,
    data: &mut MByteBuffer,
    client: &mut Client,
) -> Result<()> {
    let email = data.read::<String>()?;
    let password = data.read::<String>()?;
    let appmajor = data.read::<u16>()? as usize;
    let appminior = data.read::<u16>()? as usize;
    let apprevision = data.read::<u16>()? as usize;
    let reconnect_code = data.read::<String>()?;
    let server_name = data.read::<String>()?;

    if APP_MAJOR > appmajor && APP_MINOR > appminior && APP_REVISION > apprevision {
        return send_infomsg(storage, client, "Client needs to be updated.".into(), true).await;
    }

    if email.len() >= 64 || password.len() >= 128 {
        return send_infomsg(
            storage,
            client,
            "Account does not Exist or Password is not Correct.".into(),
            true,
        )
        .await;
    }

    let player: PlayerWithPassword = match find_player(storage, &email, &password).await? {
        Some(player) => player,
        None => {
            return send_infomsg(
                storage,
                client,
                "Account does not Exist or Password is not Correct.".into(),
                true,
            )
            .await;
        }
    };

    // we need to Add all the player types creations in a sub function that Creates the Defaults and then adds them to World.
    let code = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

    if let Some(current) = player.current_server {
        if let Some(old_reconnect_code) = player.reconnect_code {
            if old_reconnect_code == reconnect_code {
                let token = match storage.server_names.get(&current) {
                    Some(token) => token,
                    None => {
                        return send_infomsg(
                            storage,
                            client,
                            "Account logged in. Could not Verify Relogin Code.".into(),
                            true,
                        )
                        .await;
                    }
                };

                let game_server = storage.server.read().await.game_servers.get(token).cloned();

                let game_server = match game_server {
                    Some(v) => v,
                    None => {
                        return send_infomsg(
                            storage,
                            client,
                            "Account logged in. Could not Verify Relogin Code.".into(),
                            true,
                        )
                        .await;
                    }
                };

                let mut lock = game_server.lock().await;

                send_kill_client(storage, &mut lock, player.uid).await?;
            } else {
                return send_infomsg(
                    storage,
                    client,
                    "Account logged in. Could not Verify Relogin Code.".into(),
                    true,
                )
                .await;
            }
        } else {
            return send_infomsg(
                storage,
                client,
                "Account logged in. Could not Verify Relogin Code.".into(),
                true,
            )
            .await;
        }
    }

    info!(
        "Player {} with IP: {}, Logging in to Server: {}",
        &player.username, &client.addr, &server_name
    );

    let claim = UserClaim {
        server_name,
        uid: player.uid,
    };

    let token = storage
        .keys
        .encode(&Header::new(Algorithm::HS512), &claim)?;

    update_reconnect_code(storage, player.uid, Some(code.clone())).await?;
    send_login(storage, client, token, &code).await
}

pub async fn handle_server_list_request(
    storage: &mut Storage,
    _data: &mut MByteBuffer,
    client: &mut Client,
) -> Result<()> {
    send_server_list(storage, client).await
}

pub async fn handle_verification(
    storage: &mut Storage,
    data: &mut MByteBuffer,
    game_server: &mut GameServer,
) -> Result<()> {
    let token = data.read::<String>()?;

    if let Some((_index, data)) = storage
        .keys
        .decode::<UserClaim>(&token, &Validation::new(Algorithm::HS512))
    {
        let claim: UserClaim = data.claims;
        if let Some(server) = storage.servers.get(&game_server.token) {
            if claim.server_name == server.name {
                return send_verification(storage, game_server, claim.uid, true).await;
            }
        }
    }

    send_verification(storage, game_server, 0, false).await
}

pub async fn handle_update_server_info(
    storage: &mut Storage,
    data: &mut MByteBuffer,
    game_server: &mut GameServer,
) -> Result<()> {
    let name = data.read::<String>()?;
    let ip = data.read::<String>()?;
    let port = data.read::<u16>()?;
    let players_on = data.read::<u64>()?;
    let max_players = data.read::<u64>()?;

    if let Some(server) = storage.servers.get_mut(&game_server.token) {
        server.ip = ip;
        server.port = port;
        server.name = name;
        server.players_on = players_on;
        server.max_players = max_players;
    }

    Ok(())
}

pub async fn handle_update_server_count(
    storage: &mut Storage,
    data: &mut MByteBuffer,
    game_server: &mut GameServer,
) -> Result<()> {
    let players_on = data.read::<u64>()?;
    let max_players = data.read::<u64>()?;

    if let Some(server) = storage.servers.get_mut(&game_server.token) {
        server.players_on = players_on;
        server.max_players = max_players;
    }

    Ok(())
}
