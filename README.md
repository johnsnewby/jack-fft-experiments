# Test DSP, Jack etc. for Rust

The basis of this borrowed from https://github.com/maniflames/MicViz

## Getting started

### Environment setup

You need Jack, v2 seems to work better at the moment.

The three library released on crates.io doesn't seem to work. Clone one into the ../three like this:
```
git clone https://github.com/three-rs/three
```
You need to make patchbay connections so that some input source and some output source are connected to this app.
