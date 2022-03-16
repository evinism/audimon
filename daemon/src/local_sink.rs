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
use dasp::ring_buffer::Fixed;
use dasp::{interpolate::sinc::Sinc, ring_buffer, signal, Signal};


pub fn local_sink(mut audio_pipe: tokio::sync::mpsc::Receiver<Vec<(i16, i16)>>) -> anyhow::Result<()> {
    let buffer =  RB::from([0f32; 2048]);
    let buf_ref_1 = Arc::new(Mutex::new(buffer));
    let buf_ref_2 = buf_ref_1.clone();

    let (_host, device, config) = host_device_setup()?;

    let sample_rate = config.sample_rate().0 as f32;
    let nchannels = config.channels() as usize;
    let mut request = SampleRequestOptions {
        sample_rate,
        nchannels,
        sample_counter: 0,
        prev: 0.
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
        let rb = Fixed::from([0i16; 100]);
        loop {
            let frames = audio_pipe.recv().await;
            let sinc = Sinc::new(rb);
            //let new_signal = signal.from_hz_to_hz(sinc, 48000f64, sample_rate as f64);
        
            if let Ok(mut guard) = buf_ref_2.try_lock() {
                for frame in frames.iter() {
                    for msg in frame.iter() {
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

fn sampler(o: &mut SampleRequestOptions, buf_ref: &Arc<Mutex<RB<[f32;2048]>>>) -> f32 {
    o.tick();
    let res;
    if let Ok(mut guard) = buf_ref.try_lock() {
        res = guard.pop().unwrap_or(o.prev)
    } else {
        res = o.prev
    }
    o.prev = res;
    res
}

pub struct SampleRequestOptions {
    pub sample_rate: f32,
    pub sample_counter: usize,
    pub nchannels: usize,
    pub prev: f32,
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

    //let config = device.default_output_config()?;
    let mut config = device.default_output_config()?;
    for config_range in device.supported_output_configs()? {
        let target = cpal::SampleRate(48000);
        if (config_range.min_sample_rate() <= target) && (config_range.max_sample_rate() >= target) {
            config = config_range.with_sample_rate(cpal::SampleRate(48000));
        }
    }

    println!("Default output config : {:?}", config);

    Ok((host, device, config))
}
