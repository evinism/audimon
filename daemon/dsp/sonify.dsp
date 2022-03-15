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

base_freq = 110;


process = _ <: 
  os.osc(base_freq * (1 + _)) * 0.15 +
  os.osc(base_freq * (1 + 3 * _)) * _ <: volume : _,_;

