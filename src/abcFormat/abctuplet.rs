pub(crate) struct ABCTuplet {
    abctoken: ABCToken,

}

impl ABCTuplet {
    pub(crate) fn new() -> ABCTuplet {
        ABCTuplet {
            abctoken: ABCToken::new(),

        }
    }
    
    pub(crate) fn new(src: String) {
        todo!()
    }
    pub(crate) fn updateRatio(&self, timeSignatureObj: ) {
        todo!()
    }
    pub(crate) fn updateNoteCount(&self) {
        todo!()
    }
}