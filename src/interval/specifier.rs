#[derive(Clone, Debug)]
pub(crate) enum Specifier {
    Perfect = 1,
    Major = 2,
    Minor = 3,
    Augmented = 4,
    Diminished = 5,
    DoubleAugmented = 6,
    DoubleDiminished = 7,
    TripleAugmented = 8,
    TripleDiminished = 9,
    QuadrupleAugmented = 10,
    QuadrupleDiminished = 11,
}
