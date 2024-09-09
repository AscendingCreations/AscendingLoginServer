use crate::{
    containers::*,
    gametypes::*,
    players::*,
    socket::Client,
    sql::{integers::Shifting, *},
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use password_hash::SaltString;
use sqlx::{FromRow, PgPool};

#[derive(Debug, PartialEq, Eq, FromRow)]
pub struct Check {
    pub username_check: bool,
    pub email_check: bool,
}

pub async fn initiate(conn: &PgPool) -> Result<()> {
    let queries = [
        LOGTYPE_SCHEMA,
        LOGTYPE_SCHEMA_ALTER,
        USERACCESS_SCHEMA,
        USERACCESS_SCHEMA_ALTER,
        MAP_POSITION_SCHEMA,
        MAP_POSITION_SCHEMA_ALTER,
        POSITION_SCHEMA,
        POSITION_SCHEMA_ALTER,
        PLAYER_SEQ_SCHEMA,
        PLAYER_SEQ_SCHEMA_ALTER,
        PLAYER_SCHEMA,
        PLAYER_SCHEMA_ALTER,
        EQUIPMENT_SCHEMA,
        EQUIPMENT_SCHEMA_ALTER,
        INVENTORY_SCHEMA,
        INVENTORY_SCHEMA_ALTER,
        STORAGE_SCHEMA,
        STORAGE_SCHEMA_ALTER,
        LOGS_SCHEMA,
        LOGS_SCHEMA_ALTER,
    ];

    for quere in queries {
        sqlx::query(quere).execute(conn).await?;
    }

    Ok(())
}

pub async fn find_player(
    storage: &mut Storage,
    email: &str,
    password: &str,
) -> Result<Option<PlayerWithPassword>> {
    let userdata: Option<PlayerWithPassword> = sqlx::query_as(
        r#"
        SELECT uid, username, current_server, reconnect_code, password FROM player
        WHERE email = $1
    "#,
    )
    .bind(email)
    .fetch_optional(&storage.pgconn)
    .await?;

    if let Some(userdata) = userdata {
        let hash = match PasswordHash::new(&userdata.password[..]) {
            Ok(v) => v,
            Err(_) => return Err(AscendingError::IncorrectPassword),
        };

        if Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok()
        {
            Ok(Some(userdata))
        } else {
            Err(AscendingError::IncorrectPassword)
        }
    } else {
        Ok(None)
    }
}

pub async fn check_existance(storage: &mut Storage, username: &str, email: &str) -> Result<i64> {
    let check: Check =
        sqlx::query_as(r#"SELECT EXISTS(SELECT 1 FROM player WHERE username=$1) as username_check, EXISTS(SELECT 1 FROM player WHERE email=$2) as email_check"#)
            .bind(username)
            .bind(email)
            .fetch_one(&storage.pgconn)
            .await?;

    if check.username_check {
        return Ok(1);
    } else if check.email_check {
        return Ok(2);
    }

    Ok(0)
}

pub async fn new_player(
    storage: &mut Storage,
    client: &mut Client,
    player: &Player,
    password: String,
) -> Result<i64> {
    let argon = Argon2::default();
    let hashed_password = if let Ok(salt) = SaltString::encode_b64(SALT) {
        if let Ok(hash) = argon.hash_password(password.as_bytes(), &salt) {
            hash.to_string()
        } else {
            String::from("FailedPasswordHash")
        }
    } else {
        String::from("FailedPasswordHash")
    };

    let (uid, ): (i64,) =  sqlx::query_as(r#"
        INSERT INTO public.player(
            username, address, password, itemtimer, deathtimer, vals, spawn, pos, email, sprite, indeath, level, levelexp, resetcount, pk, data, vital, vital_max, access)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19) RETURNING uid;
        "#)
            .bind(&player.username)
            .bind(&client.addr)
            .bind(hashed_password)
            .bind(player.item_timer)
            .bind(player.death_timer)
            .bind(i64::unshift_signed(&player.vals))
            .bind(player.spawn)
            .bind(player.pos)
            .bind(&player.email)
            .bind(i16::unshift_signed(&player.sprite))
            .bind(player.death_type.is_spirit())
            .bind(player.level)
            .bind(i64::unshift_signed(&player.level_exp))
            .bind(player.reset_count)
            .bind(player.pk)
            .bind(player.data)
            .bind(player.vital)
            .bind(player.vital_max)
            .bind(player.access)
            .fetch_one(&storage.pgconn).await?;

    let inv_insert = PGInvItem::into_insert_all(PGInvItem::new(&player.inventory, uid));

    for script in inv_insert {
        sqlx::query(&script).execute(&storage.pgconn).await?;
    }

    let storage_insert = PGStorageItem::into_insert_all(PGStorageItem::new(&player.storage, uid));

    for script in storage_insert {
        sqlx::query(&script).execute(&storage.pgconn).await?;
    }

    let equip_insert = PGEquipItem::into_insert_all(PGEquipItem::new(&player.equipment, uid));

    for script in equip_insert {
        sqlx::query(&script).execute(&storage.pgconn).await?;
    }

    Ok(uid)
}

pub async fn update_address(storage: &mut Storage, user_id: i64, address: String) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE public.player
        SET address=$2
        WHERE uid = $1;
    "#,
    )
    .bind(user_id)
    .bind(&address)
    .execute(&storage.pgconn)
    .await?;

    Ok(())
}

pub async fn update_passreset(
    storage: &mut Storage,
    user_id: i64,
    resetpassword: Option<String>,
) -> Result<()> {
    sqlx::query(
        r#"
                UPDATE public.player
                SET passresetcode=$2
                WHERE uid = $1;
            "#,
    )
    .bind(user_id)
    .bind(resetpassword)
    .execute(&storage.pgconn)
    .await?;

    Ok(())
}

pub async fn update_reconnect_code(
    storage: &mut Storage,
    user_id: i64,
    reconnect_code: Option<String>,
) -> Result<()> {
    sqlx::query(
        r#"
                UPDATE public.player
                SET reconnect_code=$2
                WHERE uid = $1;
            "#,
    )
    .bind(user_id)
    .bind(reconnect_code)
    .execute(&storage.pgconn)
    .await?;

    Ok(())
}
