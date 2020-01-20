use clap::{App, Arg};
use hound;
#[macro_use]
extern crate lazy_static;
use parking_lot::Mutex;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::FFTplanner;
use std::cell::RefCell;
use std::io::Write;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use twang::Sound;

struct SenderReceiver {
    pub sender: Sender<f32>,
    pub receiver: Receiver<f32>,
}

impl SenderReceiver {
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        Self { sender, receiver }
    }
}

lazy_static! {
    static ref SAMPLE_CHANNEL: Mutex<SenderReceiver> = Mutex::new(SenderReceiver::new());
}

const SAMPLE_RATE: usize = 48000;

fn main() {
    let matches = App::new("Jack FFT test")
        .arg(
            Arg::with_name("sine")
                .short("s")
                .long("sine")
                .required(false),
        )
        .get_matches();
    let (client, _status) =
        jack::Client::new("microphone_test", jack::ClientOptions::NO_START_SERVER)
            .expect("Couldn't connect to jack");

    let portspec = jack::AudioIn::default();
    println!("Portspec: {:?}", portspec);

    let jack_mic = client
        .register_port("microphone", jack::AudioIn::default())
        .expect("Error getting input device");

    let mut speaker_l = client
        .register_port("speaker_l", jack::AudioOut::default())
        .expect("Error getting output device");
    let mut speaker_r = client
        .register_port("speaker_r", jack::AudioOut::default())
        .expect("Error getting output device");

    let (sender, receiver) = multiqueue::broadcast_queue(100);
    let safe_sender = std::sync::Arc::new(std::sync::Mutex::new(sender));
    let recv_stream = receiver.add_stream();

    let sender_clone = safe_sender.clone();

    let jack_callback = move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
        let out = speaker_l.as_mut_slice(ps);
        {
            let receiver = &SAMPLE_CHANNEL.lock().receiver;
            for v in out.iter_mut() {
                if let Ok(sample) = receiver.recv() {
                    *v = sample;
                } else {
                    sender_clone.lock().unwrap().try_send(None);
                }
            }
        }
        let data = jack_mic.as_slice(ps);
        let d2 = data.to_vec();
        match sender_clone.lock().unwrap().try_send(Some(d2)) {
            Ok(_) => jack::Control::Continue,
            Err(_) => {
                println!("QUIT, send fgailed");
                jack::Control::Quit
            }
        }
    };

    let _process = jack::ClosureProcessHandler::new(jack_callback);
    let active_client = client.activate_async((), _process).unwrap();

    active_client
        .as_client()
        .connect_ports_by_name("system:capture_1", "microphone_test:microphone")
        .unwrap();
    active_client
        .as_client()
        .connect_ports_by_name("microphone_test:speaker_l", "system:playback_1")
        .unwrap();
    active_client
        .as_client()
        .connect_ports_by_name("microphone_test:speaker_r", "system:playback_2")
        .unwrap();

    std::thread::spawn(|| {
        for sample in hound::WavReader::new(std::io::stdin())
            .unwrap()
            .samples::<f32>()
        {
            SAMPLE_CHANNEL.lock().sender.send(sample.unwrap() as f32);
        }
    });

    println!("After activate async");
    let clone = recv_stream.clone();
    receiver.unsubscribe();
    while let Ok(received) = clone.recv() {
        match received {
            Some(audio_buffer) => {
                println!("Read {}", audio_buffer.len());
            }
            None => break,
        }
    }
    active_client.deactivate().unwrap();
    drop(safe_sender.lock());
    println!("Waiting for threads to exit");
    println!("Done");
}
