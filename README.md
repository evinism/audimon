# Audimon

Audimon is a *heavily* WIP project to do sonification of system metrics. It's very cool!

Requirements: Rust and Faust.

To get running locally:
```sh
git submodule init
git submodule update
cd daemon
cargo run -- --local
```

`./daemon/` is the daemon that actually collects and performs sonification.
`./web/` is the web interface for when you're running in webrtc mode.

## Hopes and Dreams
* Placing processes in sonic space (e.g. left, right, maybe forward / back). Should probably be based on hash of process path, with small-scale deviations based on hash of PID.
* Adjusting tone based on process memory?? I could imagine either constant sounds emanating from all processes taking CPU, or maybe memory, or something. Could also imagine on process exit, we encode the memory usage of that process.
* Distinguishing local vs. nonlocal network devices. How do i tell if a packet went to loopback? Could do this via lowpass on internal network devices, if that can be distinguished.
* Somehow sonifying memory usage. Originally considered via harmonics on top of the status (cpu) tone.
* SWAP ALARM

