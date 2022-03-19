use tokio::time::Duration;
use sysinfo::{NetworkExt, ProcessorExt, System, SystemExt};
use faust_state::DspHandle;
use smallvec::SmallVec;
use rand::Rng;


mod faust {
    include!(concat!(env!("OUT_DIR"), "/dsp.rs"));
}

const SMEAR_RATIO: f32 = 0.1;
const FRAME_SIZE: usize = 960;

type AudioThreadChannel = tokio::sync::mpsc::Sender<Vec<(i16, i16)>>;
type AudioFrame = [f32; FRAME_SIZE];


fn mount_positive_samples_in_buffer(num: isize) -> AudioFrame {
    let mut samples_buffer: AudioFrame = [0f32; FRAME_SIZE];
    let mut rng = rand::thread_rng();
    for _ in 0..(num) {
        // at max, i want the packet buffer to be alternating 1s and 0s
        let position: usize = rng.gen::<usize>() % (FRAME_SIZE / 2);
        samples_buffer[position * 2] += 1.;
    };
    samples_buffer
}


fn cpu_buf(sys: &mut System, cpu_usage_smooth: &mut f32) -> AudioFrame {
    // Smoothed
    sys.refresh_cpu();
    let old_cpu_usage_smooth = *cpu_usage_smooth;
    let total_cpu_usage: f32 = sys.processors().into_iter().map(|x| x.cpu_usage()).sum();
    let normed_cpu_usage_raw = total_cpu_usage / (sys.processors().len() as f32);
    if normed_cpu_usage_raw.is_normal() {
        *cpu_usage_smooth = (1. - SMEAR_RATIO) * (*cpu_usage_smooth)  + SMEAR_RATIO * (normed_cpu_usage_raw / 100.0);
    };
    let mut new_buf: AudioFrame = [0.; FRAME_SIZE];
    for i in 0..FRAME_SIZE {
        let ratio = i as f32 / FRAME_SIZE as f32;
        new_buf[i] = (old_cpu_usage_smooth) * (1. - ratio) + *cpu_usage_smooth * ratio
    }
    new_buf
}

fn mem_buf(sys: &mut System, mem_usage_smooth: &mut f32) -> AudioFrame {
    sys.refresh_memory();
    let normed_mem_usage_raw = sys.used_memory() as f32 / sys.total_memory() as f32;
    if normed_mem_usage_raw.is_normal() {
        *mem_usage_smooth = (1. - SMEAR_RATIO) * (*mem_usage_smooth)  + SMEAR_RATIO * normed_mem_usage_raw;
    }
    [*mem_usage_smooth; FRAME_SIZE]
}

fn packet_buf(sys: &mut System) -> (AudioFrame, AudioFrame) {
    sys.refresh_networks();
    let mut num_inc_packets = 0;
    let mut num_out_packets = 0;
    for (_, data) in sys.networks() {
        num_inc_packets += data.packets_received();
        num_out_packets += data.packets_transmitted();
    };
    (
        mount_positive_samples_in_buffer(num_inc_packets as isize),
        mount_positive_samples_in_buffer(num_out_packets as isize)
    )
}

fn process_buf(sys: &mut System, prev_num_of_processes: &mut isize) -> (AudioFrame, AudioFrame) {
    sys.refresh_processes();

    // TODO: What happens when we get a process added and removed at the same time?
    // Do we maintain a list of processes and try to match current list to prev to tell
    // if we got a new one?
    let current_processes = sys.processes().len() as isize;
    let process_delta = current_processes - *prev_num_of_processes;
    *prev_num_of_processes = current_processes;

    let pos_process_buffer = mount_positive_samples_in_buffer(if process_delta > 0 { process_delta } else { 0 });
    let neg_process_buffer = mount_positive_samples_in_buffer(if process_delta < 0 { -process_delta } else { 0 });
    (pos_process_buffer, neg_process_buffer)
}

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
    let mut cpu_usage_smooth = 0.0; // range [0, 1]
    let mut mem_usage_smooth = 0.0; // range [0, 1]
    let mut num_processes = sys.processes().len() as isize;

    let mut ticker = tokio::time::interval(Duration::from_millis(20));
    loop {
        // Create and populate buffers
        let cpu_buffer: AudioFrame = cpu_buf(&mut sys, &mut cpu_usage_smooth);
        let mem_buffer: AudioFrame = mem_buf(&mut sys, &mut mem_usage_smooth);

        let (inc_packet_buffer, out_packet_buffer) = packet_buf(&mut sys);
        let (pos_process_buffer, neg_process_buffer) = process_buf(&mut sys, &mut num_processes);


        let mut inputs = SmallVec::<[&[f32]; 64]>::with_capacity(num_inputs as usize);
        inputs.push(&cpu_buffer[..]);
        inputs.push(&mem_buffer[..]);
        inputs.push(&inc_packet_buffer[..]);
        inputs.push(&out_packet_buffer[..]);
        inputs.push(&pos_process_buffer[..]);
        inputs.push(&neg_process_buffer[..]);
        let mut one: AudioFrame = [0.0; FRAME_SIZE];
        let mut two: AudioFrame = [0.0; FRAME_SIZE];
        let mut outputs = SmallVec::<[&mut [f32]; 64]>::with_capacity(num_outputs as usize);

        outputs.push(&mut one);
        outputs.push(&mut two);

        let len = FRAME_SIZE;
        dsp.update_and_compute(len as i32, &inputs[..], &mut outputs[..]);
        let left_out_vec = outputs[0].to_vec();
        let right_out_vec = outputs[1].to_vec();

        let left_out_samples = left_out_vec.iter().map(|sample| {
            dasp::sample::Sample::to_sample(*sample)
        });
        let right_out_samples = right_out_vec.iter().map(|sample| {
            dasp::sample::Sample::to_sample(*sample)
        });

        let out_samples = itertools::izip!(
            left_out_samples,
            right_out_samples
        ).collect::<Vec<(i16, i16)>>();

        sink.send(out_samples).await.expect("Oh no! Sending didn't work!");
        ticker.tick().await;
    }
}

pub fn spawn_audio_thread(sink: AudioThreadChannel){
    tokio::spawn(async move {
        audio(sink).await;
    });
}