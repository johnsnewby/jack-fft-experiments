use clap::{App, Arg};
use hound;
#[macro_use]
extern crate lazy_static;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::FFTplanner;
use std::io::Write;
use std::sync::Mutex;
use three;
use twang::Sound;

#[derive(Debug)]
struct State {
    sound_values: Vec<f32>,
    scene_meshes: Vec<three::Mesh>,
}

lazy_static! {
    static ref SAMPLE_COUNT: Mutex<std::sync::atomic::AtomicUsize> =
        Mutex::new(std::sync::atomic::AtomicUsize::new(0));
}

const MAX_SAMPLES: usize = 240000; // 5s @ 48k

fn record(receiver: multiqueue::BroadcastReceiver<Option<Vec<f32>>>) {
    let mut writer = hound::WavWriter::create(
        "test.wav",
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for samples in receiver {
        match samples {
            Some(x) => {
                for sample in x {
                    writer.write_sample(sample).unwrap();
                }
            }
            None => break,
        }
    }
    println!("Writing WAV");
    writer.finalize().unwrap();
    println!("WAV done");
}

fn fft(receiver: multiqueue::BroadcastReceiver<Option<Vec<f32>>>) {
    let mut samples: Vec<f32> = vec![];
    for received in receiver {
        match received {
            Some(newsamples) => {
                println!(
                    "samples len: {}, newsamples len: {}",
                    samples.len(),
                    newsamples.len()
                );
                samples.extend(newsamples.iter());
            }
            None => break,
        }
    }
    println!("Falling falling");
    let mut planner: FFTplanner<f32> = FFTplanner::new(false);
    let mut input: Vec<Complex<f32>> = samples[0..48000]
        .iter()
        .map(|val| Complex::new(*val, 0f32))
        .collect();
    let fft = planner.plan_fft(input.len());
    let mut output: Vec<Complex<f32>> = Vec::new();
    output.resize(input.len(), Zero::zero());
    fft.process(&mut input, &mut output);
    output.truncate(output.len() / 2);
    let mut file = std::fs::File::create(format!(
        "sample.csv",
        //                std::time::SystemTime::now()
        //                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
        //                    .unwrap()
        //                    .as_secs()
    ))
    .unwrap();
    for ele in output {
        file.write(format!("{}\n", power(&ele)).as_bytes()).unwrap();
    }
    println!("FFT Exit");
}

// some magic which I do not currently understand
fn power(complex: &Complex<f32>) -> f32 {
    20f32
        * (complex.re * complex.re + complex.im * complex.im)
            .sqrt()
            .log10()
}

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

    let jack_mic = client
        .register_port("microphone", jack::AudioIn::default())
        .expect("Error getting input device");

    let mut speaker = client
        .register_port("speaker", jack::AudioOut::default())
        .expect("Error getting output device");

    for port in client.ports(None, None, jack::PortFlags::empty()) {
        println!("{}", port);
    }

    let (sender, receiver) = multiqueue::broadcast_queue(100);
    let safe_sender = std::sync::Arc::new(std::sync::Mutex::new(sender));
    let recv_stream = receiver.add_stream();

    let mut pink = twang::Pink::new(None);
    let mut snds = Sound::new(None, 440.0);
    let sine = matches.is_present("sine");
    let fft_handle = std::thread::spawn(move || {
        fft(recv_stream);
    });
    let recv_stream = receiver.add_stream();
    let recv_clone = recv_stream.clone();
    let record_handle = std::thread::spawn(move || {
        record(recv_clone);
    });

    let sender_clone = safe_sender.clone();
    let jack_callback = move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
        let out = speaker.as_mut_slice(ps);
        for v in out.iter_mut() {
            let val: i16 = if sine {
                snds.next().unwrap().sin().into()
            } else {
                pink.next().unwrap().into()
            };
            *v = val as f32;
        }
        let data = jack_mic.as_slice(ps);
        let mut u = SAMPLE_COUNT
            .lock()
            .unwrap()
            .load(std::sync::atomic::Ordering::Relaxed);
        u = u + data.len();
        *(SAMPLE_COUNT.lock().unwrap()).get_mut() = u;
        println!(
            "count is {}",
            SAMPLE_COUNT
                .lock()
                .unwrap()
                .load(std::sync::atomic::Ordering::Relaxed)
        );
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

    let capture = active_client
        .as_client()
        .port_by_name("system:capture_1")
        .unwrap();
    let playback = active_client
        .as_client()
        .port_by_name("system:playback_1")
        .unwrap();

    active_client
        .as_client()
        .connect_ports_by_name("system:capture_1", "microphone_test:microphone")
        .unwrap();
    active_client
        .as_client()
        .connect_ports_by_name("microphone_test:speaker", "system:playback_1")
        .unwrap();

    println!("After activate async");
    let clone = recv_stream.clone();
    receiver.unsubscribe();
    while let Ok(received) = clone.recv() {
        match received {
            Some(audio_buffer) => {
                println!("Read {}", audio_buffer.len());
                if SAMPLE_COUNT
                    .lock()
                    .unwrap()
                    .load(std::sync::atomic::Ordering::Relaxed)
                    > MAX_SAMPLES
                {
                    println!("Stopping");
                    safe_sender.lock().unwrap().try_send(None).unwrap();
                    break;
                }
            }
            None => break,
        }
    }
    active_client.deactivate().unwrap();
    drop(safe_sender.lock());
    println!("Waiting for threads to exit");
    record_handle.join().unwrap();
    fft_handle.join().unwrap();
    println!("Done");
}
