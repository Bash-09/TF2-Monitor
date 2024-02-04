use std::str::FromStr;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::Directive, fmt::writer::MakeWriterExt, prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt, EnvFilter, Layer,
};

pub fn init_tracing() -> Option<WorkerGuard> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    let hyper = Directive::from_str("hyper=warn").expect("Bad directive");
    let demo_parser = Directive::from_str("tf_demo_parser=warn").expect("Bad directive");
    let wgpu_hal = Directive::from_str("wgpu_hal=warn").expect("Bad directive");
    let wgpu_core = Directive::from_str("wgpu_core=warn").expect("Bad directive");
    let iced_wgpu = Directive::from_str("iced_wgpu=warn").expect("Bad directive");
    let fontdb = Directive::from_str("fontdb=error").expect("Bad directive");
    let naga = Directive::from_str("naga=warn").expect("Bad directive");
    let cosmic_text = Directive::from_str("cosmic_text=warn").expect("Bad directive");
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(
                EnvFilter::from_default_env()
                    .add_directive(hyper.clone())
                    .add_directive(demo_parser.clone())
                    .add_directive(wgpu_hal.clone())
                    .add_directive(wgpu_core.clone())
                    .add_directive(iced_wgpu.clone())
                    .add_directive(fontdb.clone())
                    .add_directive(naga.clone())
                    .add_directive(cosmic_text.clone()),
            ),
    );

    match std::fs::File::create("./macclient.log") {
        Ok(latest_log) => {
            let (file_writer, guard) = tracing_appender::non_blocking(latest_log);
            subscriber
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_writer(file_writer.with_max_level(tracing::Level::TRACE))
                        .with_filter(
                            EnvFilter::builder()
                                .parse("debug")
                                .expect("Bad env")
                                .add_directive(hyper)
                                .add_directive(demo_parser)
                                .add_directive(wgpu_hal)
                                .add_directive(wgpu_core)
                                .add_directive(iced_wgpu)
                                .add_directive(fontdb)
                                .add_directive(naga)
                                .add_directive(cosmic_text),
                        ),
                )
                .init();
            Some(guard)
        }
        Err(e) => {
            subscriber.init();
            tracing::error!(
                "Failed to create log file, continuing without persistent logs: {}",
                e
            );
            None
        }
    }
}
