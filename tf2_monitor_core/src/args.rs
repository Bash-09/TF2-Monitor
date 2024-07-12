use std::path::PathBuf;

use clap::{ArgAction, Parser};

#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Override the port to host the web-ui and API on
    #[arg(long)]
    pub web_port: Option<u16>,
    /// Serve web-ui files from this directory
    #[arg(long)]
    pub web_dir: Option<PathBuf>,

    /// Override the default tf2 directory
    #[arg(long)]
    pub tf2_dir: Option<String>,
    /// Override the Steam User
    #[arg(long)]
    pub steam_user: Option<String>,

    /// Only parse the bare minimum to allow demo uploads (may improve
    /// performance)
    #[arg(long, action=ArgAction::SetTrue, default_value_t=false)]
    pub minimal_demo_parsing: bool,
    /// Don't monitor or parse demos (may improve performance, but also prevents
    /// demo uploads)
    #[arg(long, action=ArgAction::SetTrue, default_value_t=false)]
    pub dont_parse_demos: bool,
    /// Don't upload demos to the masterbase
    #[arg(long, action = ArgAction::SetTrue, default_value_t=false)]
    pub dont_upload_demos: bool,
    /// Use http (inscure) connections to the masterbase
    #[arg(long, action=ArgAction::SetTrue, default_value_t=false)]
    pub masterbase_http: bool,
}
