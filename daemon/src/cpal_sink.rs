use cpal::{Data, Sample, SampleFormat};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};


pub fn init_local_sink(mut audio_buf_rx: tokio::sync::mpsc::Receiver<Vec<i16>>) {
    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available");
    let mut supported_configs_range = device.supported_output_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range.next()
        .expect("no supported config?!")
        .with_max_sample_rate();

    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
    let config = supported_config.into();
    let write_silence = move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
        for sample in data.iter_mut() {
            audio_buf_rx.blocking_recv().unwrap().iter();
            *sample = Sample::from(&0.0);
        }
    };
    let stream =  device.build_output_stream(&config, write_silence, err_fn).unwrap();

    stream.play().unwrap();
    loop {}
}