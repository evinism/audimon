/* This example expose parameter to pass generator of sample.
Good starting point for integration of cpal into your application.
*/

extern crate anyhow;
extern crate clap;
extern crate cpal;

use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dasp::sample::Sample;
use dasp::ring_buffer::Bounded as RB;

pub async fn local_sink(
    mut audio_pipe: tokio::sync::mpsc::Receiver<Vec<(i16, i16)>>,
    _done_tx: tokio::sync::mpsc::Sender<()>
) -> Result<(), anyhow::Error> {
    let buffer =  (RB::from([0f32; 2048]), RB::from([0f32; 2048]));
    let buf_ref_1 = Arc::new(Mutex::new(buffer));
    let buf_ref_2 = buf_ref_1.clone();

    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
    println!("Output device : {}", device.name()?);

    //let config = device.default_output_config()?;
    let mut config = device.default_output_config()?;
    for config_range in device.supported_output_configs()? {
        let target = cpal::SampleRate(48000);
        if (config_range.min_sample_rate() <= target) && (config_range.max_sample_rate() >= target) {
            config = config_range.with_sample_rate(cpal::SampleRate(48000));
        }
    }

    println!("Default output config : {:?}", config);

    let nchannels = config.channels() as usize;

    let err_fn = |err| eprintln!("Error building output sound stream: {}", err);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                sampler(output, nchannels, &buf_ref_1);
            },
            err_fn,
        )?,
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [i16], _: &cpal::OutputCallbackInfo| {
                sampler(output, nchannels, &buf_ref_1);
            },
            err_fn,
        )?,
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [u16], _: &cpal::OutputCallbackInfo| {
                sampler(output, nchannels, &buf_ref_1);
            },
            err_fn,
        )?,
    };

    stream.play()?;
    tokio::spawn(async move {
        loop {
            let frames = audio_pipe.recv().await;
            if let Ok(mut guard) = buf_ref_2.lock() {
                for frame in frames.iter() {
                    for msg in frame.iter() {
                        let out0 = Sample::to_sample::<f32>(
                            msg.0
                        );
                        let out1 = Sample::to_sample::<f32>(
                            msg.1
                        );
                        guard.0.push(Sample::from_sample(out0));
                        guard.1.push(Sample::from_sample(out1));
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("");
        }
    };
    Ok(())
}

type SharedBufReference = Arc<Mutex<(RB<[f32;2048]>, RB<[f32;2048]>)>>;


fn sampler<T: cpal::Sample>(output: &mut [T], channels: usize, buf_ref: &SharedBufReference) {
    let mut sample_count = 0;
    for stereo_sample in output.chunks_mut(channels) {
        let left: f32;
        let right: f32;
        if let Ok(mut guard) = buf_ref.lock() {
            left = guard.0.pop().unwrap_or(0.);
            right = guard.1.pop().unwrap_or(0.);
        } else {
            left = 0.;
            right = 0.;
        }
        for sample in stereo_sample.iter_mut() {
            let res = if (sample_count % 2) == 0 { left } else { right };
            *sample = cpal::Sample::from::<f32>(&res);
            sample_count += 1;
        }
    }
}

