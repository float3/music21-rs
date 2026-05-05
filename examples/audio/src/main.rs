use music21_rs::Polyrhythm;
use rodio::{
    DeviceSinkBuilder,
    source::{SineWave, Source},
};
use std::{thread::sleep, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut poly = Polyrhythm::from_time_signature(4, 120, &[6, 8])?;
    let tick_duration = Duration::from_secs_f32(poly.tick_duration()? as f32);

    let sink_handle = DeviceSinkBuilder::open_default_sink()?;
    loop {
        let (tick, triggers) = poly.next().unwrap();
        println!("Tick: {tick} - Triggers: {triggers:?}");
        for (i, &trigger) in triggers.iter().enumerate() {
            if trigger {
                let freq = 100.0 * (i + 1) as f32;
                let beep = SineWave::new(freq).take_duration(Duration::from_millis(100));
                sink_handle.mixer().add(beep);
            }
        }
        sleep(tick_duration);
    }
}
