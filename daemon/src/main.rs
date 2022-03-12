mod webrtc_sink;
mod audio;
mod cpal_sink;

use anyhow::Result;
use clap::{Command, AppSettings, Arg};
use std::io::Write;


#[tokio::main]
async fn main() -> Result<()> {
    let mut app = Command::new("reflect")
        .version("0.1.0")
        .author("Rain Liu <yliu@webrtc.rs>")
        .about("An example of how to send back to the user exactly what it receives using the same PeerConnection.")
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(
            Arg::new("FULLHELP")
                .help("Prints more detailed help information")
                .long("fullhelp"),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .short('d')
                .help("Prints debug log information"),
        )
        .arg(
            Arg::new("localaudio")
                .long("localaudio")
                .short('l')
                .help("Output to the local computer")
        );

    let matches = app.clone().get_matches();

    if matches.is_present("FULLHELP") {
        app.print_long_help().unwrap();
        std::process::exit(0);
    }

    let debug = matches.is_present("debug");
    if debug {
        env_logger::Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{}:{} [{}] {} - {}",
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.level(),
                    chrono::Local::now().format("%H:%M:%S.%6f"),
                    record.args()
                )
            })
            .filter(None, log::LevelFilter::Trace)
            .init();
    }

    let (audio_buf_tx, audio_buf_rx) = tokio::sync::mpsc::channel::<Vec<(i16, i16)>>(1);
    audio::spawn_audio_thread(audio_buf_tx);
    webrtc_sink::init_webrtc_audio_destination(audio_buf_rx).await.unwrap();

    Ok(())
}