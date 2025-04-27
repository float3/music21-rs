use music21_rs::polyrhythm::Polyrhythm;
use rodio::{
    OutputStream, Sink,
    source::{SineWave, Source},
};
use std::{thread::sleep, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut poly = Polyrhythm::new_with_time_signature(4, 120, &[6, 8])?;
    let tick_duration = Duration::from_secs_f32(poly.tick_duration()? as f32);

    let (_stream, stream_handle) = OutputStream::try_default()?;
    loop {
        let (tick, triggers) = poly.next().unwrap();
        println!("Tick: {tick} - Triggers: {triggers:?}");
        for (i, &trigger) in triggers.iter().enumerate() {
            if trigger {
                let freq = 100.0 * (i + 1) as f32;
                let sink = Sink::try_new(&stream_handle)?;
                let beep = SineWave::new(freq).take_duration(Duration::from_millis(100));
                sink.append(beep);
                sink.detach();
            }
        }
        sleep(tick_duration);
    }
}
