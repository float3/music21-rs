#[derive(Clone, Debug)]
pub(crate) enum Specifier {
    PERFECT = 1,
    MAJOR = 2,
    MINOR = 3,
    AUGMENTED = 4,
    DIMINISHED = 5,
    DBLAUG = 6,
    DBLDIM = 7,
    TRPAUG = 8,
    TRPDIM = 9,
    QUADAUG = 10,
    QUADDIM = 11,
}
