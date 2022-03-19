use tokio::time::Duration;
use sysinfo::{NetworkExt, ProcessorExt, System, SystemExt,  PidExt};
use faust_state::DspHandle;
use smallvec::SmallVec;
use rand::Rng;
use std::collections::HashSet;



mod faust {
    include!(concat!(env!("OUT_DIR"), "/dsp.rs"));
}

const SMEAR_RATIO: f32 = 0.1;
const FRAME_SIZE: usize = 960;

type AudioThreadChannel = tokio::sync::mpsc::Sender<Vec<(i16, i16)>>;
type AudioFrame = [f32; FRAME_SIZE];


fn mount_positive_samples_in_buffer(num: usize) -> AudioFrame {
    let mut samples_buffer: AudioFrame = [0f32; FRAME_SIZE];
    let mut rng = rand::thread_rng();
    for _ in 0..num {
        // at max, i want the packet buffer to be alternating 1s and 0s
        let position = rng.gen::<usize>() % (FRAME_SIZE / 2);
        samples_buffer[position * 2] += 1.;
    };
    samples_buffer
}

fn cpu_buf(sys: &mut System, cpu_usage_smooth: &mut f32) -> AudioFrame {
    // Smoothed
    sys.refresh_cpu();
    let old_cpu_usage_smooth = *cpu_usage_smooth;
    let total_cpu_usage = sys.processors().into_iter().map(|x| x.cpu_usage()).sum::<f32>();
    let normed_cpu_usage_raw = total_cpu_usage / (sys.processors().len() as f32);
    if normed_cpu_usage_raw.is_normal() {
        *cpu_usage_smooth = (1. - SMEAR_RATIO) * (*cpu_usage_smooth)  + SMEAR_RATIO * (normed_cpu_usage_raw / 100.0);
    };
    let mut new_buf: AudioFrame = [0.0; FRAME_SIZE];
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
        num_inc_packets += data.packets_received() as usize;
        num_out_packets += data.packets_transmitted() as usize;
    };
    (
        mount_positive_samples_in_buffer(num_inc_packets),
        mount_positive_samples_in_buffer(num_out_packets)
    )
}

fn get_process_set(sys: &System) -> HashSet<sysinfo::Pid> {
    sys.processes().keys().cloned().collect()
}

// Going to return: 
fn mount_processes_in_buffer(set: &HashSet<&sysinfo::Pid>, prev_pan: &mut f32) -> (AudioFrame, AudioFrame) {
    let mut samples_buffer: AudioFrame = [0f32; FRAME_SIZE];
    // 1   0   0   0   1 
    // 0.4 0.4 0.4 0.4 -.9
    let mut panning_buffer: AudioFrame = [*prev_pan; FRAME_SIZE];
    let mut rng = rand::thread_rng();
    for pid in set.into_iter() {
        // at max, i want the packet buffer to be alternating 1s and 0s
        let position = rng.gen::<usize>() % (FRAME_SIZE / 2);
        samples_buffer[position * 2] += 1.;
        let mut current = (position * 2) + 1;

        // Really really bad hash function that doesn't really actually matter
        let pan = ((pid.as_u32() * 1337  % 256) as f32) / 128.0 - 1.;


        while (current < FRAME_SIZE) & (samples_buffer[current] != 1.) {
            panning_buffer[current] = pan;
            current += 1;
            if current >= FRAME_SIZE {
                *prev_pan = pan;
                break;
            }
        }
    };
    (samples_buffer, panning_buffer)
}

// spawned, spawned_pan, dropped, dropped_pan
fn process_buf(
    sys: &mut System, 
    prev_process_set: &mut HashSet<sysinfo::Pid>, 
    prev_pan_spawned: &mut f32,
    prev_pan_dropped: &mut f32,
) -> (AudioFrame, AudioFrame, AudioFrame, AudioFrame) {
    sys.refresh_processes();
    let new_processs_set = get_process_set(&sys);

    let spawned = new_processs_set.difference(&prev_process_set).collect::<HashSet<&sysinfo::Pid>>();
    let dropped = prev_process_set.difference(&new_processs_set).collect::<HashSet<&sysinfo::Pid>>();

    let (pos_process_buffer, pos_pan_buffer) = mount_processes_in_buffer(&spawned, prev_pan_spawned);
    let (neg_process_buffer, neg_pan_buffer) = mount_processes_in_buffer(&dropped, prev_pan_dropped);

    // 1000000000000100000000100000001000000100001000000000100.
    // -.8,-.8,-.8,-.3,-.3,-.3,

    *prev_process_set = new_processs_set;
    (pos_process_buffer, pos_pan_buffer, neg_process_buffer, neg_pan_buffer)
}

async fn audio(sink: AudioThreadChannel) {
    // DSP Init
    let mut dsp = Box::new(DspHandle::<faust::Sonify>::new().0);
    dsp.init(48000);
    let num_inputs = dsp.num_inputs();
    let num_outputs = dsp.num_outputs();
    println!("inputs: {}", num_inputs);
    println!("outputs: {}", num_outputs);

    //
    let mut sys = System::new_all();
    sys.refresh_all();
    let mut cpu_usage_smooth = 0.0; // range [0, 1]
    let mut mem_usage_smooth = 0.0; // range [0, 1]
    let mut prev_pan_spawned = 0f32;
    let mut prev_pan_dropped = 0f32;
    let mut process_set = get_process_set(&sys);

    let mut ticker = tokio::time::interval(Duration::from_millis(20));
    loop {
        // Create and populate buffers
        let cpu_buffer: AudioFrame = cpu_buf(&mut sys, &mut cpu_usage_smooth);
        let mem_buffer: AudioFrame = mem_buf(&mut sys, &mut mem_usage_smooth);

        let (inc_packet_buffer, out_packet_buffer) = packet_buf(&mut sys);
        let (
            pos_process_buffer,
            pos_pan_buffer,
            neg_process_buffer,
            neg_pan_buffer,
        ) = process_buf(
            &mut sys,
            &mut process_set,
            &mut prev_pan_spawned,
            &mut prev_pan_dropped
        );

        let inputs = SmallVec::from([
            &cpu_buffer[..],
            &mem_buffer[..],
            &inc_packet_buffer[..],
            &out_packet_buffer[..],
            &pos_process_buffer[..],
            &pos_pan_buffer[..],
            &neg_process_buffer[..],
            &neg_pan_buffer[..],
        ]);

        //print!("{:?}", pos_pan_buffer);

        let mut one: AudioFrame = [0.0; FRAME_SIZE];
        let mut two: AudioFrame = [0.0; FRAME_SIZE];
        let mut outputs = SmallVec::<[&mut [f32]; 2]>::from([
            &mut one[..],
            &mut two[..]
        ]);

        dsp.update_and_compute(FRAME_SIZE as i32, &inputs[..], &mut outputs[..]);
        let left_out_vec = outputs[0].to_vec();
        let right_out_vec = outputs[1].to_vec();

        let left_out_samples = left_out_vec.iter().map(|sample| {
            dasp::sample::Sample::to_sample(*sample)
        });

        let right_out_samples = right_out_vec.iter().map(|sample| {
            dasp::sample::Sample::to_sample(*sample)
        });

        let out_samples = left_out_samples.zip(right_out_samples).collect();

        sink.send(out_samples).await.expect("Oh no! Sending didn't work!");
        ticker.tick().await;
    }
}

pub fn spawn_audio_thread(sink: AudioThreadChannel){
    tokio::spawn(audio(sink));
}