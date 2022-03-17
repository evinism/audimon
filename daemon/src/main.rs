mod webrtc_sink;
mod local_sink;
mod audio;

use anyhow::Result;
use clap::{Command, AppSettings, Arg};

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = Command::new("audimon")
        .version("0.0.1")
        .author("Evin Sellin <evinism@gmail.com>")
        .about("Sonification of various server parameters")
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(
            Arg::new("FULLHELP")
                .help("Prints more detailed help information")
                .long("fullhelp"),
        )
        .arg(
            Arg::new("local")
                .long("local")
                .short('l')
                .help("Output to the local computer")
        );

    let matches = app.clone().get_matches();

    if matches.is_present("FULLHELP") {
        app.print_long_help().unwrap();
        std::process::exit(0);
    }

    let (audio_buf_tx, audio_buf_rx) = tokio::sync::mpsc::channel::<Vec<(i16, i16)>>(1);
    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    audio::spawn_audio_thread(audio_buf_tx);
    if matches.is_present("local") {
        local_sink::local_sink(audio_buf_rx, done_tx).await.expect("Failed to start local audio.");
    } else {
        webrtc_sink::webrtc_sink(audio_buf_rx, done_tx).await.expect("Failed to start webrtc audio.");
    }

    println!("Press ctrl-c to stop");
    tokio::select! {
        //_ = timeout.as_mut() => {
        //    println!("received timeout signal!");
        //}
        _ = done_rx.recv() => {
            println!("received done signal!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!("");
        }
    };

    Ok(())
}