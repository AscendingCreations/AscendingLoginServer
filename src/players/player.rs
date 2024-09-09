use crate::{gametypes::*, items::*, time_ext::*};
use educe::Educe;

#[derive(Clone, Debug, Educe)]
#[educe(Default)]
pub struct Player {
    pub username: String,
    pub email: String,
    pub level_exp: u64,
    pub pvp_on: bool,
    pub pk: bool,
    pub code: String,
    pub vals: u64,
    pub sprite: u16,
    #[educe(Default = (0..MAX_EQPT).map(|_| Item::default()).collect())]
    pub equipment: Vec<Item>,
    #[educe(Default = (0..MAX_STORAGE).map(|_| Item::default()).collect())]
    pub storage: Vec<Item>,
    #[educe(Default = (0..MAX_INV).map(|_| Item::default()).collect())]
    pub inventory: Vec<Item>,
    #[educe(Default = MyInstant::now())]
    pub item_timer: MyInstant,
    #[educe(Default = [25, 2, 100])]
    pub vital: [i32; VITALS_MAX],
    #[educe(Default = [25, 2, 100])]
    pub vital_max: [i32; VITALS_MAX],
    pub spawn: Position,
    pub dir: u8,
    #[educe(Default = MyInstant::now())]
    pub death_timer: MyInstant,
    #[educe(Default = 1)]
    pub level: i32,
    pub data: [i64; 10],
    pub access: UserAccess,
    pub pos: Position,
    pub death_type: DeathType,
    pub current_server: Option<String>,
    pub reset_count: i64,
}
