use hound;
#[macro_use]
extern crate lazy_static;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::FFTplanner;
use std::io;
use std::io::Write;
use std::sync::mpsc::*;
use std::sync::Mutex;
use three;
use twang::Sound;

#[derive(Debug)]
struct State {
    sound_values: Vec<f32>,
    scene_meshes: Vec<three::Mesh>,
}

const WINDOW_SIZE: usize = 44000;

lazy_static! {
    static ref WRITER: Mutex<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = {
        Mutex::new(
            hound::WavWriter::create(
                "test.wav",
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 48000,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                },
            )
            .unwrap(),
        )
    };
}

fn fft(receiver: Receiver<Vec<f32>>) {
    let mut samples: Vec<f32> = vec![];
    while let Ok(newsamples) = receiver.recv() {
        println!(
            "samples len: {}, newsamples len: {}",
            samples.len(),
            newsamples.len()
        );
        samples.extend(newsamples.iter());
        println!("Samples count: {}", samples.len());
        if samples.len() >= WINDOW_SIZE {
            let mut planner: FFTplanner<f32> = FFTplanner::new(false);
            let mut input: Vec<Complex<f32>> =
                samples.iter().map(|val| Complex::new(*val, 0f32)).collect();
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
            panic!("Run once!");
            samples = vec![];
        }
    }
    println!("Exiting");
}

// some magic which I do not currently understand
fn power(complex: &Complex<f32>) -> f32 {
    20f32
        * (complex.re * complex.re + complex.im * complex.im)
            .sqrt()
            .log10()
}

fn main() {
    let (client, status) =
        jack::Client::new("microphone_test", jack::ClientOptions::NO_START_SERVER)
            .expect("Couldn't connect to jack");

    let jack_mic = client
        .register_port("microphone", jack::AudioIn::default())
        .expect("Error getting input device");

    let mut speaker = client
        .register_port("speaker", jack::AudioOut::default())
        .expect("Error getting output device");

    let (sender, receiver) = channel();
    let (fftsender, fftreceiver) = channel();

    // don't know why the sanders are not moved into the closure, this
    // gets around that they are not. TODO: fix.
    let safe_sender = Mutex::new(sender);
    let safe_fftsender = Mutex::new(fftsender);
    let mut pink = twang::Pink::new(None);

    std::thread::spawn(move || {
        fft(fftreceiver);
    });

    let jack_callback = move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
        let out = speaker.as_mut_slice(ps);
        let testvec: Vec<f32> = vec![];
        for v in out.iter_mut() {
            let val: i16 = pink.next().unwrap().into();
            *v = val as f32;
            // uncomment below (and comment the corresponding line in the
            // window function) to log the source pink noise
            //  (*WRITER.lock().unwrap()).write_sample(val as f32);
        }

        let data = jack_mic.as_slice(ps);
        print!("{} ", data.len());

        let d2 = data.to_vec();
        let d3 = d2.clone();
        safe_fftsender.lock().unwrap().send(d3).unwrap();

        match safe_sender.lock().unwrap().send(d2) {
            Ok(_) => jack::Control::Continue,
            Err(_) => jack::Control::Quit,
        }
    };

    let _process = jack::ClosureProcessHandler::new(jack_callback);
    let _active_client = client.activate_async((), _process).unwrap();

    let mut builder = three::Window::builder("A window Imani built");
    builder.fullscreen(false);
    let mut win = builder.build();
    win.scene.background = three::Background::Color(0x000000);
    let mut state = State {
        sound_values: Vec::new(),
        scene_meshes: Vec::new(),
    };

    let camera = win.factory.orthographic_camera([0.0, 0.0], 1.0, -1.0..1.0);
    let mut sample_count = 0;
    while win.update() && !win.input.hit(three::KEY_ESCAPE) {
        update_lines(&mut win, &mut state);
        win.render(&camera);
        remove_lines(&mut win, &mut state);

        while let Ok(audio_buffer) = receiver.try_recv() {
            sample_count += 1;
            update_sound_values(&audio_buffer, &mut state);
            for sample in audio_buffer.iter() {
                // uncomment this line (and comment the line above in the
                // callback) to log what the microphone receives
                (*WRITER.lock().unwrap()).write_sample(*sample);
            }
        }
        if sample_count > 1000 {
            break;
        }
    }
    _active_client.deactivate();
    (*WRITER.lock().unwrap()).flush().unwrap();
}

fn update_sound_values(samples: &[f32], state: &mut State) {
    state.sound_values = samples.to_vec();
}

fn update_lines(win: &mut three::window::Window, state: &mut State) {
    for (index, y_position) in state.sound_values.iter().enumerate() {
        let i = index as f32;
        let num_samples = state.sound_values.len() as f32;
        let scale = 3.0;
        let x_position = (i / (num_samples / scale)) - (0.5 * scale);

        let geometry = three::Geometry::with_vertices(vec![
            [x_position, y_position.clone(), 0.0].into(),
            [x_position, -y_position.clone(), 0.0].into(),
        ]);

        let material = three::material::Line { color: 0xFFFFFF };

        let mesh = win.factory.mesh(geometry, material);
        win.scene.add(&mesh);
        state.scene_meshes.push(mesh);
    }
}

fn remove_lines(win: &mut three::window::Window, state: &mut State) {
    for mesh in &state.scene_meshes {
        win.scene.remove(&mesh);
    }

    state.scene_meshes.clear();
}
