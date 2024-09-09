use mmap_bytey::{MByteBufferRead, MByteBufferWrite};
use serde::{Deserialize, Serialize};

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Default,
    MByteBufferRead,
    MByteBufferWrite,
    sqlx::Type,
)]
#[sqlx(type_name = "user_access")]
pub enum UserAccess {
    #[default]
    None,
    Monitor,
    Admin,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    MByteBufferRead,
    MByteBufferWrite,
    sqlx::Type,
)]
#[sqlx(type_name = "log_type")]
pub enum LogType {
    Login,
    Logout,
    Item,
    Warning,
    Error,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Default,
    Serialize,
    Deserialize,
    MByteBufferRead,
    MByteBufferWrite,
)]
pub enum DeathType {
    #[default]
    Alive,
    Spirit,
    Dead,
    Spawning,
}

impl DeathType {
    pub fn is_dead(self) -> bool {
        !matches!(self, DeathType::Alive)
    }

    pub fn is_spirit(self) -> bool {
        matches!(self, DeathType::Spirit)
    }

    pub fn is_alive(self) -> bool {
        matches!(self, DeathType::Alive)
    }

    pub fn is_spawning(self) -> bool {
        matches!(self, DeathType::Spawning)
    }
}

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, MByteBufferRead, MByteBufferWrite,
)]
pub enum FtlType {
    Message,
    Error,
    Item,
    Quest,
    Level,
    Money,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Default,
    MByteBufferRead,
    MByteBufferWrite,
)]
pub enum VitalTypes {
    Hp,
    Mp,
    Sp,
    #[default]
    Count,
}
