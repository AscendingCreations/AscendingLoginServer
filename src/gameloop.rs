mod handledata;
mod mainloop;

pub use handledata::{client_handle_data, game_server_handle_data};
pub use mainloop::game_loop;
