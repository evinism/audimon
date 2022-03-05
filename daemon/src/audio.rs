use dasp::Signal;
use tokio::time::Duration;
use sysinfo::{NetworkExt, NetworksExt, ProcessExt, System, SystemExt};
use sysinfo::ProcessorExt;



pub fn spawn_audio_thread(sink: tokio::sync::mpsc::Sender<Vec<i16>>){
  tokio::spawn(async move {
      let mut sys = System::new_all();
      sys.refresh_all();
      let mut freqz = 440.0;
      let mut ctr = 0;
      let cpu_usage = dasp::signal::gen_mut(||{
        if ctr % 960 == 0 {
          sys.refresh_cpu();
          let total_cpu_usage: f32 = sys.processors().into_iter().map(|x| x.cpu_usage()).sum();
          let normed_cpu_usage = total_cpu_usage / (sys.processors().len() as f32);
          if normed_cpu_usage.is_normal() {
            freqz = freqz * 0.9 + 0.1 * ((normed_cpu_usage + 100.0) * 440.0 / 100.0) as f64;
          }
          ctr = 0;
        }
        ctr = ctr + 1;
        return freqz;
      });
      let mut audio_sine_wave = dasp::signal::rate(48000.0).hz(cpu_usage).sine();
      let mut ticker = tokio::time::interval(Duration::from_millis(20));
      let sample_count = 960;
      loop {
          let samples = audio_sine_wave.by_ref().take(sample_count).map(dasp::sample::Sample::to_sample).collect::<Vec<i16>>();
          sink.send(samples).await.unwrap();
          let _ = ticker.tick().await;
      }
  });
}