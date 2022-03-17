use tokio::time::Duration;
use sysinfo::{NetworkExt, NetworksExt, ProcessorExt,  ProcessExt, System, SystemExt};
use faust_state::DspHandle;
use smallvec::SmallVec;
use rand::Rng;


mod faust {
    include!(concat!(env!("OUT_DIR"), "/dsp.rs"));
}

type AudioThreadChannel = tokio::sync::mpsc::Sender<Vec<(i16, i16)>>;


async fn audio(sink: AudioThreadChannel) {
    // DSP Init
    let (mut dsp, _state) = DspHandle::<faust::Sonify>::new();
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


    let mut ticker = tokio::time::interval(Duration::from_millis(20));
    loop {
        // Gather Stats
        sys.refresh_cpu();
        sys.refresh_memory();
        sys.refresh_networks();

        let total_cpu_usage: f32 = sys.processors().into_iter().map(|x| x.cpu_usage()).sum();
        let normed_cpu_usage_raw = total_cpu_usage / (sys.processors().len() as f32);
        if normed_cpu_usage_raw.is_normal() {
            cpu_usage_smooth = (1. - smear_ratio) * cpu_usage_smooth  + smear_ratio * (normed_cpu_usage_raw / 100.0);
        }

        let normed_mem_usage_raw = sys.used_memory() as f32 / sys.total_memory() as f32;
        if normed_mem_usage_raw.is_normal() {
            mem_usage_smooth = (1. - smear_ratio) * cpu_usage_smooth  + smear_ratio * normed_mem_usage_raw;
        }

        // calculate num of packets
        // Network interfaces name, data received and data transmitted:
        let mut num_packets = 0;
        for (interface_name, data) in sys.networks() {
            num_packets += data.packets_received();
        }

        // Create and populate buffers
        let cpu_buffer: [f32; 960] = [cpu_usage_smooth; 960];
        let mem_buffer: [f32; 960] = [mem_usage_smooth; 960];

        let num_packets = if num_packets > 960 / 2 {960 / 2} else { num_packets };
        let mut packet_buffer: [f32; 960] = [0f32; 960];
        {
            let mut rng = rand::thread_rng();
            
            for _ in 0..(num_packets) {
                // at max, i want the packet buffer to be alternating 1s and 0s
                let position: usize = rng.gen::<usize>() % (960 / 2);
                packet_buffer[position * 2] += 1.;
            }
        }


        let mut inputs = SmallVec::<[&[f32]; 64]>::with_capacity(num_inputs as usize);
        inputs.push(&cpu_buffer[..]);
        inputs.push(&mem_buffer[..]);
        inputs.push(&packet_buffer[..]);
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
        sink.send(out_samples).await.expect("Oh no! Sending didn't work!");
        ticker.tick().await;
    }
}

pub fn spawn_audio_thread(sink: AudioThreadChannel){
    tokio::spawn(async move {
        audio(sink).await;
    });
}