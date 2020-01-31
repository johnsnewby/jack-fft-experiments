use crossbeam_channel::Receiver;

use basic_dsp::*;

use crate::sample::Sample;

pub fn matcher(receiver: Receiver<Sample>) -> std::thread::JoinHandle<()> {
    let mut input: Vec<f32> = vec![];
    let mut output: Vec<f32> = vec![];
    let mut _matches = 0;
    let mut _since_last_match = 0;

    std::thread::spawn(move || loop {
        match receiver.recv().unwrap() {
            Sample::In(mut data) => {
                input.append(&mut data);
            }
            Sample::Out(mut data) => {
                output.append(&mut data);
            }
            Sample::Done => break,
        }
        if input.len() > crate::SAMPLE_RATE {
            /*
            let mut comparisons = vec![];
            for i in 0..4000 {
                comparisons.push(compare(&input, &output[i..].to_vec()).unwrap());
            }
            println!("Compare returned {:?}", comparisons);
            println!("Input {:?}", squish(&input));
            println!("Output {:?}", squish(&output));
             */
            cross_correlate(&input, &output);
            input.truncate(0);
            output.truncate(0);
        }
    })
}

fn cross_correlate(input: &Vec<f32>, output: &Vec<f32>) {
    let mut vector1 = input.clone().to_complex_time_vec();
    let vector2 = output.clone().to_complex_time_vec();
    let mut buffer = SingleBuffer::new();
    let argument = vector2.prepare_argument_padded(&mut buffer);
    vector1.correlate(&mut buffer, &argument).unwrap();
    let mut max = 0f32;
    let mut max_pos = 0;
    println!("vec len: {}", input.len());
    let data = vector1.data(..);
    for i in 0..data.len() {
        let val = data[i];
        if val > max {
            max = val;
            max_pos = i;
        }
    }
    println!("Max {} at {}", max, max_pos);
    println!("{:?}", vector1);
}
