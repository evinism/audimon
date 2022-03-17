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
status_tone(cpu) = 
  os.osc(lo_freq(cpu)) * 0.15 +
  os.osc(hi_freq(cpu)) * cpu;

process = _ : status_tone : _ * 0.5 <: volume : _,_;

