use std::{io::Cursor, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use bitbuffer::BitRead;
use chrono::{Datelike, Timelike};
use filenamify::filenamify;
use iced::widget;
use image::{io::Reader, DynamicImage, GenericImage, GenericImageView, ImageFormat};
use tf2_monitor_core::state::MonitorState;
use tf_demo_parser::{
    demo::{self, header::Header},
    Demo,
};

use crate::{gui::replay::main_window, App, IcedElement, Message};

const DEFAULT_THUMBNAIL: &[u8] = include_bytes!("default.png");

const TEMPLATE_DMX: &str = include_str!("template_dmx.txt");
const TEMPLATE_VMT: &str = include_str!("template_vmt.txt");
const DIR_THUMBNAIL: &str = "tf/materials/vgui/replay/thumbnails";
const DIR_REPLAY: &str = "tf/replay/client/replays";
const DEMO_PATH: &str = "tf/demos";

const SUB_NAME: &str = "%replay_name%";
const SUB_MAP: &str = "%map%";
const SUB_LENGTH: &str = "%length%";
const SUB_TITLE: &str = "%title%";
const SUB_DEMO: &str = "%demo%";
const SUB_SCREENSHOT: &str = "%screenshot%";
const SUB_DATE: &str = "%date%";
const SUB_TIME: &str = "%time%";
const SUB_HANDLE: &str = "%handle%";

#[allow(clippy::module_name_repetitions)]
pub struct ReplayState {
    pub demo_path: Option<PathBuf>,
    pub thumbnail_path: Option<PathBuf>,
    pub demo: Result<demo::header::Header, String>,
    pub status: String,

    pub replay_name: String,
    pub thumbnail: DynamicImage,
    pub thumbnail_handle: widget::image::Handle,
}

#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub enum ReplayMessage {
    SetDemoPath(PathBuf),
    BrowseDemoPath,
    BrowseThumbnailPath,
    ClearThumbnail,
    CreateReplay,
    SetReplayName(String),
}

impl ReplayState {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new() -> Self {
        let thumbnail = DynamicImage::new(0, 0, image::ColorType::Rgb8);
        let mut image_bytes = Vec::new();
        thumbnail
            .write_to(&mut Cursor::new(&mut image_bytes), ImageFormat::Bmp)
            .expect("Couldn't write to vector???");
        let thumbnail_handle = widget::image::Handle::from_memory(image_bytes);

        let mut state = Self {
            demo_path: None,
            thumbnail_path: None,
            demo: Err(String::from("None chosen")),
            replay_name: String::new(),
            thumbnail,
            thumbnail_handle,
            status: String::new(),
        };

        state
            .load_thumbnail(None)
            .expect("Couldn't load default thumbnail");
        state
    }

    pub fn handle_message(
        &mut self,
        message: ReplayMessage,
        mac: &MonitorState,
    ) -> iced::Command<Message> {
        match message {
            ReplayMessage::BrowseThumbnailPath => {
                if let Some(new_thumbnail_path) = rfd::FileDialog::new().pick_file() {
                    if let Err(e) = self.load_thumbnail(Some(new_thumbnail_path)) {
                        self.status = format!("Failed to set thumbnail: {e:?}");
                    }
                };
            }
            ReplayMessage::BrowseDemoPath => {
                let mut picker = rfd::FileDialog::new();
                if let Some(tf2_dir) = &mac.settings.tf2_directory {
                    picker = picker.set_directory(tf2_dir.join(DEMO_PATH));
                }

                if let Some(new_demo_path) = picker.pick_file() {
                    self.set_demo_path(new_demo_path);
                };
            }
            ReplayMessage::ClearThumbnail => {
                if let Err(e) = self.load_thumbnail(None) {
                    self.status = format!("Failed to set thumbnail: {e:?}");
                }
            }
            ReplayMessage::CreateReplay => {
                if let Err(e) = self.create_replay(mac) {
                    self.status = format!("Error creating replay: {e}");
                } else {
                    self.status = String::from("Successfully created replay!");
                }
            }
            ReplayMessage::SetReplayName(name) => self.replay_name = name,
            ReplayMessage::SetDemoPath(demo_path) => self.set_demo_path(demo_path),
        }

        iced::Command::none()
    }

    pub fn view<'a>(&'a self, state: &'a App) -> IcedElement<'a> {
        main_window(state).into()
    }

    pub fn set_demo_path(&mut self, path: PathBuf) {
        self.demo_path = Some(path);

        let Some(demo_path) = &self.demo_path else {
            return;
        };

        let bytes = match std::fs::read(demo_path) {
            Ok(b) => b,
            Err(e) => {
                self.demo = Err(format!("{e}"));
                return;
            }
        };

        let demo = Demo::new(&bytes);
        let mut stream = demo.get_stream();

        let header: Header = match Header::read(&mut stream) {
            Ok(header) => header,
            Err(e) => {
                self.demo = Err(format!("Couldn't parse demo header ({e})"));
                return;
            }
        };

        let datetime = chrono::offset::Local::now();
        self.replay_name = format!(
            "{}-{}-{} {}:{} - {} on {}",
            datetime.year(),
            datetime.month(),
            datetime.day(),
            datetime.hour(),
            datetime.minute(),
            &header.nick,
            &header.map,
        );

        self.demo = Ok(header);
        self.status = String::new();
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn load_thumbnail(&mut self, new_thumbnail_path: Option<PathBuf>) -> Result<()> {
        let thumbnail_bytes = new_thumbnail_path.as_ref().map_or_else(
            || Ok(Vec::from(DEFAULT_THUMBNAIL)),
            |p| std::fs::read(p).context("Reading thumbnail file"),
        )?;

        let thumbnail_original = Reader::new(Cursor::new(&thumbnail_bytes))
            .with_guessed_format()
            .context("Determining file format")?
            .decode()
            .context("Decoding image")?
            .resize(512, 512, image::imageops::FilterType::Triangle);

        let mut thumbnail = DynamicImage::new(512, 512, image::ColorType::Rgb8);
        for (x, y, p) in thumbnail_original.pixels() {
            thumbnail.put_pixel(x, y, p);
        }

        let mut image_bytes = Vec::new();
        thumbnail
            .write_to(&mut Cursor::new(&mut image_bytes), ImageFormat::Bmp)
            .context("Writing file to buffer")?;

        let thumbnail_handle = widget::image::Handle::from_memory(image_bytes);

        self.thumbnail_path = new_thumbnail_path;
        self.thumbnail = thumbnail;
        self.thumbnail_handle = thumbnail_handle;

        Ok(())
    }

    /// Returns the create replay of this [`App`].
    ///
    /// # Errors
    /// If not all the required fields are present, or some IO error prevented file writeback.
    ///
    /// This function will return an error if .
    pub fn create_replay(&self, mac: &MonitorState) -> Result<()> {
        let Ok(header) = &self.demo else {
            return Err(anyhow!("No valid demo"));
        };
        let Some(tf2_dir) = &mac.settings.tf2_directory else {
            return Err(anyhow!("No TF2 directory set"));
        };
        let Some(demo_path) = &self.demo_path else {
            return Err(anyhow!("No demo provided"));
        };

        let file_name = filenamify(&self.replay_name);
        if file_name.trim().is_empty() {
            return Err(anyhow!("Replay name is not valid"));
        }

        let handle = &mut std::fs::read_dir(tf2_dir.join(DIR_REPLAY))
            .context("Reading replay folder")?
            .filter_map(std::result::Result::ok)
            .filter(|d| d.path().extension().is_some_and(|e| e == "dmx"))
            .count();

        let datetime = chrono::offset::Local::now();

        #[allow(clippy::cast_sign_loss)]
        let date: u32 = (datetime.year() as u32 - 2009) << 9
            | (datetime.month() - 1) << 5
            | (datetime.day() - 1);
        let time: u32 = datetime.minute() << 5 | datetime.hour();

        let vtf = vtf::vtf::VTF::create(self.thumbnail.clone(), vtf::ImageFormat::Rgb888)
            .context("Creating thumbnail VTF")?;

        // Write replay DMX
        let mut dmx_contents = String::from(TEMPLATE_DMX);
        dmx_contents = dmx_contents.replace(SUB_NAME, &file_name);
        dmx_contents = dmx_contents.replace(SUB_MAP, &header.map);
        dmx_contents = dmx_contents.replace(SUB_LENGTH, &format!("{}", header.duration));
        dmx_contents = dmx_contents.replace(SUB_TITLE, &self.replay_name);
        dmx_contents = dmx_contents.replace(SUB_DEMO, &format!("{file_name}.dem"));
        dmx_contents = dmx_contents.replace(SUB_SCREENSHOT, &file_name);
        dmx_contents = dmx_contents.replace(SUB_DATE, &format!("{date}"));
        dmx_contents = dmx_contents.replace(SUB_TIME, &format!("{time}"));
        dmx_contents = dmx_contents.replace(SUB_HANDLE, &format!("{handle}"));

        std::fs::write(
            tf2_dir.join(DIR_REPLAY).join(format!("{file_name}.dmx")),
            dmx_contents,
        )
        .context("Writing demo DMX")?;

        std::fs::copy(
            demo_path,
            tf2_dir.join(DIR_REPLAY).join(format!("{file_name}.dem")),
        )
        .context("Copying demo file")?;

        // Write thumbnail stuff
        let mut thumbnail_vmt = String::from(TEMPLATE_VMT);
        thumbnail_vmt = thumbnail_vmt.replace(SUB_SCREENSHOT, &file_name);

        std::fs::write(
            tf2_dir.join(DIR_THUMBNAIL).join(format!("{file_name}.vmt")),
            thumbnail_vmt,
        )
        .context("Writing thumbnail VMT")?;

        std::fs::write(
            tf2_dir.join(DIR_THUMBNAIL).join(format!("{file_name}.vtf")),
            vtf,
        )
        .context("Writing thumbnail VTF")?;

        Ok(())
    }
}

impl Default for ReplayState {
    fn default() -> Self {
        Self::new()
    }
}
