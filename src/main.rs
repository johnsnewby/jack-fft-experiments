use clap::{App, Arg};
use crossbeam_channel::unbounded;
use hound;

//const SAMPLE_RATE: usize = 48000;

fn main() {
    let _matches = App::new("Jack FFT test")
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

    let _jack_mic = client
        .register_port("microphone", jack::AudioIn::default())
        .expect("Error getting input device");

    let mut speaker_l = client
        .register_port("speaker_l", jack::AudioOut::default())
        .expect("Error getting output device");
    let mut speaker_r = client
        .register_port("speaker_r", jack::AudioOut::default())
        .expect("Error getting output device");

    let (sender, receiver) = unbounded::<f32>();
    //let safe_sender = std::sync::Arc::new(std::sync::Mutex::new(sender));

    let jack_callback = move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
        let out_l = speaker_l.as_mut_slice(ps);
        let out_r = speaker_r.as_mut_slice(ps);

        let stereo_out = out_l.iter_mut().zip(out_r.iter_mut());
        for (left, right) in stereo_out {
            *left = receiver.recv().unwrap();
            *right = receiver.recv().unwrap();
        }
        jack::Control::Continue
    };
    /*
        let data = jack_mic.as_slice(ps);
        let d2 = data.to_vec();
        match sender_clone.lock().unwrap().try_send(Some(d2)) {
            Ok(_) => jack::Control::Continue,
            Err(_) => {
                println!("QUIT, send fgailed");
                jack::Control::Quit
            }
        }
    */
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

    let wav_reader = std::thread::spawn(move || {
        for sample in hound::WavReader::new(std::io::stdin())
            .unwrap()
            .samples::<f32>()
        {
            sender.send(sample.unwrap()).unwrap();
        }
    });

    println!("After activate async");
    wav_reader.join().unwrap();
    active_client.deactivate().unwrap();
    println!("Done");
}
