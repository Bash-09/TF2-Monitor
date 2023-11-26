use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::writer::MakeWriterExt, prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt, EnvFilter, Layer,
};

pub fn init_tracing() -> Option<WorkerGuard> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var(
            "RUST_LOG",
            "info,hyper::proto=warn,wgpu_core=warn,wgpu_hal=warn,iced_wgpu=warn,fontdb=error",
        );
    }

    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(EnvFilter::from_default_env()),
    );

    match std::fs::File::create("./macclient.log") {
        Ok(latest_log) => {
            let (file_writer, guard) = tracing_appender::non_blocking(latest_log);
            subscriber
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_writer(file_writer.with_max_level(tracing::Level::TRACE)),
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
