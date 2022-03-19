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

positive_only(sig) = select2(sig >= 0, 0, sig);
derivative = _ <: _, @(1)  : _ - _ : positive_only : an.abs_envelope_rect(0.2) : _;
power(sig, num) = prod(i, num, sig);


// Status tone!
base_freq = 110;
lo_freq(cpu) = base_freq * (1 + cpu);
hi_freq(cpu) = base_freq * (1 + 3 * cpu);
status_tone(
  cpu_load,
  mem_load,
  incoming_packet_stream,
  outgoing_packet_stream,
  pos_process_stream,
  pos_process_pan,
  neg_process_stream,
  neg_process_pan
) = (
        os.osc(lo_freq(cpu_load)) / 2 + 
        os.osc(hi_freq(cpu_load))
    ) * (derivative(cpu_load) * 10 * 960  +   cpu_load * 0.1) <: _, _;


neg_respecting_square = _ <: _ * _ * _;


randompan(sig) = no.noise : (neg_respecting_square(_) / 2)  + 0.5 <: _ * sig, (1 - _) * sig;

packet_sounder(
  cpu_load,
  mem_load,
  incoming_packet_stream,
  outgoing_packet_stream,
  pos_process_stream,
  pos_process_pan,
  neg_process_stream,
  neg_process_pan
) = incoming_packet_stream * 0.05, outgoing_packet_stream * 0.05: _ , _ ;


// panning signal expected -1 to 1 inclusive
pan_by(sig, panning) = sig <: _ * (panning + 1) / 2, _ * (-panning + 1) / 2;

panned_process(
  freq,
  process_stream,
  process_pan
) = sy.combString(freq, 0.1, process_stream) * 0.2 : pan_by(_, process_pan);


process_sounder(
  cpu_load, 
  mem_load,
  incoming_packet_stream,
  outgoing_packet_stream,
  pos_process_stream,
  pos_process_pan,
  neg_process_stream,
  neg_process_pan
) = 
  panned_process(hi_freq(cpu_load) * 2, pos_process_stream, pos_process_pan),
  panned_process(hi_freq(cpu_load), neg_process_stream, neg_process_pan) :> _, _;


memory_pressure_aleter(
  cpu_load, 
  mem_load,
  incoming_packet_stream,
  outgoing_packet_stream,
  pos_process_stream,
  pos_process_pan,
  neg_process_stream,
  neg_process_pan
) = 
  os.lf_squarewavepos((2 / (1.2 - power(mem_load, 10)))) : _ * 0.5 + 1 : hi_freq(cpu_load) * _ : os.square : _ * power(mem_load, 25) * 0.05 <: _, _;

process = _, _, _, _, _, _, _, _ <: status_tone, process_sounder, packet_sounder :> _ * 0.25, _ * 0.25 : volume : _,_;

