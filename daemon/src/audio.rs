use dasp::Signal;
use tokio::time::Duration;
use sysinfo::{System, SystemExt};
use sysinfo::ProcessorExt;
use faust_state::DspHandle;
use smallvec::SmallVec;


mod faust {
    include!(concat!(env!("OUT_DIR"), "/dsp.rs"));
}

type AudioThreadChannel = tokio::sync::mpsc::Sender<Vec<(i16, i16)>>;


async fn audio(sink: AudioThreadChannel) {
    // DSP Init
    let (mut dsp, mut state) = DspHandle::<faust::Volume>::new();
    dsp.init(48000 as i32);
    let num_inputs = dsp.num_inputs();
    let num_outputs = dsp.num_outputs();
    println!("inputs: {}", num_inputs);
    println!("outputs: {}", num_outputs);

    //
    let mut sys = System::new_all();
    sys.refresh_all();
    let smear_ratio = 0.1;
    let mut cpu_usage_smooth = 0.0; // range [0, 1]
    let mut mem_usage_smooth = 0.0; // range [0, 1]
    let mut ctr: i64 = 0;

    let mut ticker = tokio::time::interval(Duration::from_millis(20));
    loop {
        // Gather Stats
        sys.refresh_cpu();
        sys.refresh_memory();

        let total_cpu_usage: f32 = sys.processors().into_iter().map(|x| x.cpu_usage()).sum();
        let normed_cpu_usage_raw = total_cpu_usage / (sys.processors().len() as f32);
        if normed_cpu_usage_raw.is_normal() {
            cpu_usage_smooth = (1. - smear_ratio) * cpu_usage_smooth  + smear_ratio * (normed_cpu_usage_raw / 100.0);
        }

        let normed_mem_usage_raw = sys.used_memory() as f32 / sys.total_memory() as f32;
        if normed_mem_usage_raw.is_normal() {
            mem_usage_smooth = (1. - smear_ratio) * cpu_usage_smooth  + smear_ratio * normed_mem_usage_raw;
        }

        // Process
        let samples: [f32; 960] = [cpu_usage_smooth; 960];
        let mut inputs = SmallVec::<[&[f32]; 64]>::with_capacity(num_inputs as usize);
        inputs.push(&samples[..]);
        let mut one: [f32; 960] = [0.0; 960];
        let mut two: [f32; 960] = [0.0; 960];
        let mut outputs = SmallVec::<[&mut [f32]; 64]>::with_capacity(num_outputs as usize);
        outputs.push(&mut one);
        outputs.push(&mut two);
        let len = 960;
        dsp.update_and_compute(len, &inputs[..], &mut outputs[..]);
        let out_samples = outputs[0].to_vec().iter().map(|sample| {
            (
                dasp::sample::Sample::to_sample(*sample),
                dasp::sample::Sample::to_sample(*sample)
            )
        }).collect::<Vec<(i16, i16)>>();
        sink.send(out_samples).await;
        let _ = ticker.tick().await;
        ctr += 1;
    }
}

pub fn spawn_audio_thread(sink: AudioThreadChannel){
    tokio::spawn(async move {
        audio(sink).await;
    });
}