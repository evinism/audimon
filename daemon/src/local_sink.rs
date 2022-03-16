/* This example expose parameter to pass generator of sample.
Good starting point for integration of cpal into your application.
*/

extern crate anyhow;
extern crate clap;
extern crate cpal;

use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dasp::sample::Sample;

pub fn local_sink(mut audio_pipe: tokio::sync::mpsc::Receiver<Vec<(i16, i16)>>) -> anyhow::Result<()> {
    let buffer = Vec::<f32>::new();
    let buf_ref_1 = Arc::new(Mutex::new(buffer));
    let buf_ref_2 = buf_ref_1.clone();

    let (_host, device, config) = host_device_setup()?;

    let sample_rate = config.sample_rate().0 as f32;
    let nchannels = config.channels() as usize;
    let mut request = SampleRequestOptions {
        sample_rate,
        nchannels,
        sample_counter: 0
    };
    let err_fn = |err| eprintln!("Error building output sound stream: {}", err);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in output.chunks_mut(request.nchannels) {
                    let value: f32 = cpal::Sample::from::<f32>(&sampler(&mut request, &buf_ref_1));
                    for sample in frame.iter_mut() {
                        *sample = value;
                    }
                }
            },
            err_fn,
        )?,
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [i16], _: &cpal::OutputCallbackInfo| {
                for frame in output.chunks_mut(request.nchannels) {
                    let value: i16 = cpal::Sample::from::<f32>(&sampler(&mut request, &buf_ref_1));
                    for sample in frame.iter_mut() {
                        *sample = value;
                    }
                }
            },
            err_fn,
        )?,
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [u16], _: &cpal::OutputCallbackInfo| {
                for frame in output.chunks_mut(request.nchannels) {
                    let value: u16 = cpal::Sample::from::<f32>(&sampler(&mut request, &buf_ref_1));
                    for sample in frame.iter_mut() {
                        *sample = value;
                    }
                }
            },
            err_fn,
        )?,
    };

    stream.play()?;
    tokio::spawn(async move {
        loop {
            let samples = audio_pipe.recv().await;
            if let Ok(mut guard) = buf_ref_2.try_lock() {
                for sample in samples.iter() {
                    for msg in sample.iter() {
                        let out = Sample::to_sample::<f32>(
                            (msg.0 + msg.1) / 2
                        );
                        guard.push(Sample::from_sample(out));
                    }
                }
            }
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(300000));
    Ok(())
}

fn sampler(o: &mut SampleRequestOptions, buf_ref: &Arc<Mutex<Vec<f32>>>) -> f32 {
    o.tick();
    if let Ok(guard) = buf_ref.try_lock() {
        guard[o.sample_counter]
    } else {
        0.
    }
}

pub struct SampleRequestOptions {
    pub sample_rate: f32,
    pub sample_counter: usize,
    pub nchannels: usize,
}

impl SampleRequestOptions {
    fn tick(&mut self) {
        self.sample_counter = self.sample_counter + 1;
    }
}

pub fn host_device_setup(
) -> Result<(cpal::Host, cpal::Device, cpal::SupportedStreamConfig), anyhow::Error> {
    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
    println!("Output device : {}", device.name()?);

    let config = device.default_output_config()?;
    println!("Default output config : {:?}", config);

    Ok((host, device, config))
}
