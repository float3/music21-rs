//! music21's virtual corpus: works kept elsewhere, each with the addresses
//! it can be downloaded from, best first.

use crate::common::formats::find_format_ext_url;

/// A work kept elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualWork {
    /// The name music21 gives its class.
    pub class: &'static str,
    /// Who wrote it.
    pub composer: &'static str,
    /// Its title.
    pub title: &'static str,
    /// The path in the corpus that names it, unique among the works.
    pub corpus_path: &'static str,
    /// Where it can be downloaded from, best first.
    pub urls: &'static [&'static str],
}

/// The works, in music21's alphabetical order.
pub const VIRTUAL_WORKS: [VirtualWork; 9] = [
    VirtualWork {
        class: "BachBWV1007Prelude",
        composer: "Johann Sebastian Bach",
        title: "Prelude from Cello Suite No. 1 in G Major, BWV 1007",
        corpus_path: "bach/bwv1007/prelude",
        urls: &[
            "https://kern.ccarh.org/cgi-bin/ksdata?l=cc/bach/cello&file=bwv1007-01.krn&f=xml",
            "https://kern.ccarh.org/cgi-bin/ksdata?l=cc/bach/cello&file=bwv1007-01.krn&f=kern",
        ],
    },
    VirtualWork {
        class: "BachBWV772",
        composer: "Johann Sebastian Bach",
        title: "Invention No. 1 in C Major, BWV 772",
        corpus_path: "bach/bwv772",
        urls: &[
            "https://kern.ccarh.org/cgi-bin/ksdata?l=osu/classical/bach/inventions&file=inven01.krn&f=xml",
        ],
    },
    VirtualWork {
        class: "BachBWV773",
        composer: "Johann Sebastian Bach",
        title: "Invention No. 2 in C Minor, BWV 773",
        corpus_path: "bach/bwv773",
        urls: &[
            "https://kern.ccarh.org/cgi-bin/ksdata?l=osu/classical/bach/inventions&file=inven02.krn&f=xml",
            "https://kern.ccarh.org/cgi-bin/ksdata?l=osu/classical/bach/inventions&file=inven02.krn&f=kern",
        ],
    },
    VirtualWork {
        class: "ColtraneGiantSteps",
        composer: "John Coltrane",
        title: "Giant Steps",
        corpus_path: "coltrane/giantSteps",
        urls: &[
            "http://impromastering.com/uploads/transcription_file/file/196/Giant_Steps__John_Coltrane_C.xml",
        ],
    },
    VirtualWork {
        class: "SchubertD576",
        composer: "Franz Schubert",
        title: "13 Variations on a Theme by Anselm Hüttenbrenner",
        corpus_path: "schubert/d576-1",
        urls: &[
            "https://kern.ccarh.org/cgi-bin/ksdata?l=cc/schubert/piano/d0576&file=d0576-06.krn&f=xml",
        ],
    },
    VirtualWork {
        class: "SchubertD5762",
        composer: "Franz Schubert",
        title: "13 Variations on a Theme by Anselm Hüttenbrenner",
        corpus_path: "schubert/d576-2",
        urls: &[
            "https://kern.ccarh.org/cgi-bin/ksdata?l=users/craig/classical/schubert/piano/d0576&file=d0576-02.krn&f=xml",
        ],
    },
    VirtualWork {
        class: "SchubertD5763",
        composer: "Franz Schubert",
        title: "13 Variations on a Theme by Anselm Hüttenbrenner",
        corpus_path: "schubert/d576-3",
        urls: &[
            "https://kern.ccarh.org/cgi-bin/ksdata?l=users/craig/classical/schubert/piano/d0576&file=d0576-03.krn&f=xml",
        ],
    },
    VirtualWork {
        class: "SchubertD5764",
        composer: "Franz Schubert",
        title: "13 Variations on a Theme by Anselm Hüttenbrenner",
        corpus_path: "schubert/d576-4",
        urls: &[
            "https://kern.ccarh.org/cgi-bin/ksdata?l=users/craig/classical/schubert/piano/d0576&file=d0576-04.krn&f=xml",
        ],
    },
    VirtualWork {
        class: "PachelbelCanonD",
        composer: "Johann Pachelbel",
        title: "Canon in D Major",
        corpus_path: "pachelbel/canon",
        urls: &["https://kern.ccarh.org/cgi-bin/ksdata?l=cc/pachelbel&file=canon.krn&f=xml"],
    },
];

/// The addresses whose format has one of these extensions, by extension
/// and then in the order given: music21's `getUrlByExt` given extensions.
pub fn urls_by_ext<'a>(urls: &[&'a str], extensions: &[&str]) -> Vec<&'a str> {
    let mut found = Vec::new();
    for extension in extensions {
        for url in urls {
            let (_, found_extension) = find_format_ext_url(url);
            if found_extension.as_deref() == Some(*extension) {
                found.push(*url);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_found_by_its_format() {
        let prelude = VIRTUAL_WORKS[0];
        assert_eq!(urls_by_ext(prelude.urls, &[".xml"]), [prelude.urls[0]]);
        assert_eq!(urls_by_ext(prelude.urls, &[".krn"]), [prelude.urls[1]]);
        assert!(urls_by_ext(prelude.urls, &[".mid"]).is_empty());
    }
}
