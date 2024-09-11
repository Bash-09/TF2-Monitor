pub mod console;
pub mod demos;
pub mod events;
pub mod masterbase;
pub mod players;
pub mod server;
pub mod settings;
pub mod steam;

use console::ConsoleOutput;
use players::Players;
use server::Server;
use settings::Settings;

pub use bitbuffer;
pub use event_loop;
pub use md5;
pub use rcon;
pub use serde_json;
pub use steamid_ng;
pub use tf_demo_parser;

#[allow(clippy::module_name_repetitions)]
pub struct MonitorState {
    pub server: Server,
    pub settings: Settings,
    pub players: Players,
}

impl MonitorState {
    pub fn handle_console_output(&mut self, output: ConsoleOutput) {
        use ConsoleOutput::{
            Chat, DemoStop, Hostname, Kill, Map, PlayerCount, ServerIP, Status, G15,
        };
        match output {
            Status(inner) => self.players.handle_status_line(inner),
            G15(inner) => self.players.handle_g15(inner),
            DemoStop(_) => {}
            Chat(_) | Kill(_) | Hostname(_) | ServerIP(_) | Map(_) | PlayerCount(_) => {
                self.server.handle_console_output(output);
            }
        }
    }
}
