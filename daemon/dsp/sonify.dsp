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

// Status tone!
base_freq = 110;
lo_freq(cpu) = base_freq * (1 + cpu);
hi_freq(cpu) = base_freq * (1 + 3 * cpu);
status_tone(cpu_load, mem_load, _packet_stream, _process_stream) = 
  os.osc(lo_freq(cpu_load)) * 0.125 +
  os.osc(hi_freq(cpu_load)) * cpu_load;

packet_sounder(_cpu_load, _mem_load, packet_stream, _process_stream) = packet_stream * 0.05;

process_sounder(cpu_load, _mem_load, packet_stream, process_stream) = 
  sy.combString(lo_freq(cpu_load) * 1.5 * 2, 0.5, process_stream) * 0.5;

process = _, _, _, _ <: status_tone, process_sounder, packet_sounder :> _ * 0.25 <: volume : _,_;

