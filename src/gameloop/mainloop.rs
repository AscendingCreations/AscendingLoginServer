use crate::{containers::Storage, socket::*, time_ext::MyInstant};
use chrono::{Duration, Utc};

pub async fn game_loop(storage: &mut Storage) {
    let mut _tick: MyInstant;
    // let mut ping_timer: MyInstant = MyInstant::now();

    loop {
        let updated_at = storage.keys.last_updated();

        if updated_at + Duration::try_hours(8).unwrap_or_default() < Utc::now() {
            storage.keys.rotate();
        }

        poll_events(storage).await.unwrap();
        process_packets(storage).await.unwrap();
        process_client_packets(storage).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
}
