declare name        "volumecontrol";
declare version     "1.0";
declare author      "Franz Heinzmann";
declare license     "BSD";
declare options     "[osc:on]";

import("stdfaust.lib");


stereo(func) = _,_ : func(_),func(_) : _,_;

volumeM = *(vslider("volume", 0, -70, +4, 0.1) : ba.db2linear : si.smoo);
volume = stereo(volumeM);

/*
  Process has several inputs:
  1: CPU usage (0 to 1)
*/


derivative = _ : an.abs_envelope_rect(0.2) <: _ , @(100) : _ - _ : abs : _;

// Status tone!
base_freq = 110;
lo_freq(cpu) = base_freq * (1 + cpu);
hi_freq(cpu) = base_freq * (1 + 3 * cpu);
status_tone(
  cpu_load,
  mem_load,
  packet_stream,
  pos_process_stream,
  neg_process_stream
) = (
        os.osc(lo_freq(cpu_load)) / 16 + 
        os.osc(hi_freq(cpu_load)) * cpu_load
    ) * (derivative(cpu_load) * 400  + 0.1);

packet_sounder(
  cpu_load,
  mem_load,
  packet_stream,
  pos_process_stream,
  neg_process_stream
) = packet_stream * 0.05;

process_sounder(
  cpu_load, 
  mem_load,
  packet_stream,
  pos_process_stream,
  neg_process_stream
) = sy.combString(hi_freq(cpu_load) * 2, 0.5, pos_process_stream) * 0.2 +
    sy.combString(hi_freq(cpu_load), 0.5, neg_process_stream) * 0.2;

process = _, _, _, _, _ <: status_tone, process_sounder, packet_sounder :> _ * 0.25 <: volume : _,_;

