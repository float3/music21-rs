use music21_rs::chord::Chord;

fn c_e_g() {
    let chord = Chord::new(Some("C E G"));
    println!("{:?}", chord.pitched_common_name());
}

fn main() {
    c_e_g();
}
