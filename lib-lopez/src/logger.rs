use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::config::{Appender, Config, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;

pub fn init_logger(is_verbose: bool) -> log4rs::Handle {
    let pattern = PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S%.3f)} [{M}:{L} {T}] {h({l})} {m}{n}");

    let console = ConsoleAppender::builder()
        .target(Target::Stderr)
        .encoder(Box::new(pattern))
        .build();

    let config = if is_verbose {
        Config::builder()
            .appender(Appender::builder().build("stderr", Box::new(console)))
            .logger(Logger::builder().build("rustls", log::LevelFilter::Error))
            .logger(Logger::builder().build("html5ever", log::LevelFilter::Error))
            .logger(Logger::builder().build("tarpc", log::LevelFilter::Error))
            .build(
                Root::builder()
                    .appender("stderr")
                    .build(log::LevelFilter::Debug),
            )
            .expect("could not config logger")
    } else {
        Config::builder()
            .appender(Appender::builder().build("stderr", Box::new(console)))
            .logger(Logger::builder().build("rustls", log::LevelFilter::Off))
            .logger(Logger::builder().build("html5ever", log::LevelFilter::Error))
            .logger(Logger::builder().build("tarpc", log::LevelFilter::Error))
            .build(
                Root::builder()
                    .appender("stderr")
                    .build(log::LevelFilter::Info),
            )
            .expect("could not config logger")
    };

    log4rs::init_config(config).expect("could not start logger")
}
