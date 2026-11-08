//! The exceptions music21 keeps in `exceptions21` that no class here
//! raises, so that code catching them by name finds them.

pyo3::create_exception!(
    music21_rs_facade,
    AnalysisException,
    crate::Music21Exception
);
pyo3::create_exception!(music21_rs_facade, TreeException, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    SubConverterException,
    crate::Music21Exception
);
pyo3::create_exception!(
    music21_rs_facade,
    ImmutableStreamException,
    crate::stream::StreamException
);
