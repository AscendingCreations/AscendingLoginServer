use crate::gametypes::*;
use mmap_bytey::{MByteBufferRead, MByteBufferWrite};
use serde::{Deserialize, Serialize};
use sqlx::Postgres;

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Default,
    Deserialize,
    Serialize,
    Hash,
    MByteBufferRead,
    MByteBufferWrite,
)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub map: MapPosition,
}

impl sqlx::Type<Postgres> for Position {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("location")
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        *ty == Self::type_info()
    }
}

impl<'r> sqlx::Decode<'r, Postgres> for Position {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> sqlx::Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let mut decoder = sqlx::postgres::types::PgRecordDecoder::new(value)?;
        let x = decoder.try_decode::<i32>()?;
        let y = decoder.try_decode::<i32>()?;
        let map = decoder.try_decode::<MapPosition>()?;
        Ok(Self { x, y, map })
    }
}

impl<'q> sqlx::Encode<'q, Postgres> for Position {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> std::result::Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let mut encoder = sqlx::postgres::types::PgRecordEncoder::new(buf);
        encoder
            .encode(self.x)?
            .encode(self.y)?
            .encode(self.map)?
            .finish();

        Ok(sqlx::encode::IsNull::No)
    }
}

impl Position {
    #[inline(always)]
    pub fn new(x: i32, y: i32, map: MapPosition) -> Position {
        Position { x, y, map }
    }
}
