use dasp::Signal;
use tokio::time::Duration;
use sysinfo::{NetworkExt, NetworksExt, ProcessExt, System, SystemExt};
use sysinfo::ProcessorExt;
use faust_state::DspHandle;

mod faust {
    include!(concat!(env!("OUT_DIR"), "/dsp.rs"));
}


async fn audio(sink: tokio::sync::mpsc::Sender<Vec<i16>>) {
    // DSP Init
    let (mut dsp, mut state) = DspHandle::<faust::Volume>::new();
    dsp.init(44800 as i32);
    let num_inputs = dsp.num_inputs();
    let num_outputs = dsp.num_inputs();
    println!("inputs: {}", num_inputs);
    println!("outputs: {}", num_inputs);

    //
    let mut sys = System::new_all();
    sys.refresh_all();
    let mut freq = 440.0;
    let mut ctr = 0;
    let smear_ratio = 0.1;
    let average_cpu_usage = dasp::signal::gen_mut(||{
        if ctr % 960 == 0 {
            sys.refresh_cpu();
            let total_cpu_usage: f32 = sys.processors().into_iter().map(|x| x.cpu_usage()).sum();
            let normed_cpu_usage = total_cpu_usage / (sys.processors().len() as f32);
            if normed_cpu_usage.is_normal() {
                freq = (1. - smear_ratio) * freq  + smear_ratio * ((normed_cpu_usage + 100.0) * 440.0 / 100.0) as f64;
            }
            ctr = 0;
        }
        ctr = ctr + 1;
        return freq;
    });

    let mut audio_sine_wave = dasp::signal::rate(48000.0).hz(average_cpu_usage).sine();
    let mut ticker = tokio::time::interval(Duration::from_millis(20));
    let sample_count = 960;
    loop {
        let samples = audio_sine_wave.by_ref().take(sample_count).map(dasp::sample::Sample::to_sample).collect::<Vec<i16>>();
        sink.send(samples).await;
        let _ = ticker.tick().await;
    }
}

pub fn spawn_audio_thread(sink: tokio::sync::mpsc::Sender<Vec<i16>>){
    tokio::spawn(async move {
        audio(sink).await;
    });
}