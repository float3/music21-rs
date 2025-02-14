#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct SlottedObjectMixin {}

impl SlottedObjectMixin {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

pub(crate) trait SlottedObjectMixinTrait {}
