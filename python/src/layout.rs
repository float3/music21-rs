//! music21's `layout`: how a score, a page, a system and a staff are laid
//! out on paper.
//!
//! These are measurements and nothing else, which is why the crate keeps
//! them as plain values and this carries them as plain attributes. A
//! length is written back the way music21 writes it -- `120` stays an
//! `int` and `120.5` a `float` (`layout::Tenths`) -- and a measurement
//! nobody gave is `None`, which is what inherits whatever the score says.
//!
//! `LayoutScore`, `Page`, `System` and `Staff` are not here: those are
//! streams music21 builds a *picture* of a score out of, and what they
//! hold is the page rather than the music.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::layout::{
    PageLayout as RsPageLayout, ScoreLayout as RsScoreLayout, StaffLayout as RsStaffLayout,
    StaffType, SystemLayout as RsSystemLayout, Tenths,
};

pyo3::create_exception!(music21_rs_facade, LayoutException, crate::Music21Exception);

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "LayoutBase",
    "ScoreLayout",
    "PageLayout",
    "SystemLayout",
    "StaffLayout",
    "LayoutException",
];

/// A length as music21 writes it: a whole number where it has no fraction.
fn tenths(value: Option<Tenths>, py: Python<'_>) -> Py<PyAny> {
    match value {
        None => py.None(),
        Some(Tenths::Whole(whole)) => whole
            .into_pyobject(py)
            .map(|value| value.into_any().unbind())
            .unwrap_or_else(|_| py.None()),
        Some(Tenths::Fraction(value)) => value
            .into_pyobject(py)
            .map(|value| value.into_any().unbind())
            .unwrap_or_else(|_| py.None()),
    }
}

/// A length read from what a caller wrote: a whole number stays whole, as
/// music21 keeps it.
fn read_tenths(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Tenths>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    if let Ok(whole) = value.extract::<i64>() {
        return Ok(Some(Tenths::Whole(whole)));
    }
    let number: f64 = value.extract()?;
    Ok(Some(if number.fract() == 0.0 && number.is_finite() {
        Tenths::Whole(number as i64)
    } else {
        Tenths::Fraction(number)
    }))
}

/// music21's `layout.LayoutBase`: what every layout object is, which says
/// nothing of itself.
#[pyclass(
    name = "LayoutBase",
    module = "music21.layout",
    subclass,
    skip_from_py_object
)]
pub struct LayoutBase;

#[pymethods]
impl LayoutBase {
    #[new]
    #[pyo3(signature = (**_keywords))]
    fn new(_keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self
    }

    fn __repr__(&self) -> &'static str {
        "<music21.layout.LayoutBase>"
    }
}

/// music21's `layout.ScoreLayout`: the scaling and the layouts a score
/// starts with, which MusicXML writes in its `<defaults>`.
#[pyclass(
    name = "ScoreLayout",
    module = "music21.layout",
    subclass,
    skip_from_py_object
)]
pub struct ScoreLayout {
    /// The scaling, which is what a score layout says of itself.
    pub(crate) inner: RsScoreLayout,
    /// The page the score starts on, as the object music21 holds: a
    /// caller who takes it and writes into it is writing into the score's,
    /// which is what music21's own layout code does.
    #[pyo3(get, set)]
    pageLayout: Option<Py<PageLayout>>,
    /// The systems it starts with, likewise.
    #[pyo3(get, set)]
    systemLayout: Option<Py<SystemLayout>>,
    /// The staves it starts with, in order: the same list every time, as
    /// music21's is, so appending to what the getter answers appends to
    /// the score's.
    #[pyo3(get, set)]
    staffLayoutList: Py<pyo3::types::PyList>,
}

#[pymethods]
impl ScoreLayout {
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(py: Python<'_>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut made = Self {
            inner: RsScoreLayout::default(),
            pageLayout: None,
            systemLayout: None,
            staffLayoutList: pyo3::types::PyList::empty(py).unbind(),
        };
        if let Some(keywords) = keywords {
            for (key, value) in keywords {
                let name: String = key.extract()?;
                match name.as_str() {
                    "scalingMillimeters" => {
                        made.inner.scaling_millimeters = read_tenths(Some(&value))?;
                    }
                    "scalingTenths" => made.inner.scaling_tenths = read_tenths(Some(&value))?,
                    other => {
                        return Err(LayoutException::new_err(format!(
                            "a score layout here says nothing of {other}"
                        )));
                    }
                }
            }
        }
        Ok(made)
    }

    /// How many millimetres the tenths below are.
    #[getter]
    fn scalingMillimeters(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.scaling_millimeters, py)
    }

    #[setter]
    fn set_scalingMillimeters(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.scaling_millimeters = read_tenths(value)?;
        Ok(())
    }

    /// How many tenths the millimetres above are.
    #[getter]
    fn scalingTenths(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.scaling_tenths, py)
    }

    #[setter]
    fn set_scalingTenths(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.scaling_tenths = read_tenths(value)?;
        Ok(())
    }

    /// How many millimetres a number of tenths is, rounded to six places,
    /// or nought where the scaling is not known: music21's
    /// `tenthsToMillimeters`.
    fn tenthsToMillimeters(&self, tenths: f64) -> f64 {
        self.inner.tenths_to_millimeters(tenths)
    }

    /// music21's `LayoutBase._reprInternal`, which says nothing: a score
    /// layout is `<music21.layout.ScoreLayout>`.
    fn __repr__(&self) -> &'static str {
        "<music21.layout.ScoreLayout>"
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copied = slf.get_type().call0()?;
        let copier = py.import("copy")?.getattr("deepcopy")?;
        let deep = |object: &Bound<'py, PyAny>| -> PyResult<Py<PyAny>> {
            match memo {
                Some(memo) => copier.call1((object, memo)).map(Bound::unbind),
                None => copier.call1((object,)).map(Bound::unbind),
            }
        };
        let (inner, page, system, staves) = {
            let me = slf.borrow();
            (
                me.inner.clone(),
                me.pageLayout.as_ref().map(|held| held.clone_ref(py)),
                me.systemLayout.as_ref().map(|held| held.clone_ref(py)),
                me.staffLayoutList.clone_ref(py),
            )
        };
        let page = match page {
            Some(held) => Some(deep(held.bind(py).as_any())?.extract(py)?),
            None => None,
        };
        let system = match system {
            Some(held) => Some(deep(held.bind(py).as_any())?.extract(py)?),
            None => None,
        };
        let staves = deep(staves.bind(py).as_any())?.extract(py)?;
        {
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = inner;
            copy.pageLayout = page;
            copy.systemLayout = system;
            copy.staffLayoutList = staves;
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    /// music21 freezes a score by pickling it; the layouts this one holds
    /// are Python objects of their own and go with it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        let me = slf.borrow();
        extra.set_item("pageLayout", me.pageLayout.as_ref())?;
        extra.set_item("systemLayout", me.systemLayout.as_ref())?;
        extra.set_item("staffLayoutList", &me.staffLayoutList)?;
        crate::pickled_extra(slf, &me.inner, Some(&extra))
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (inner, extra) = crate::unpickled_extra::<_, RsScoreLayout>(slf, state)?;
        if let Some(inner) = inner {
            slf.borrow_mut().inner = inner;
        }
        let Some(extra) = extra else {
            return Ok(());
        };
        let extra = extra.bind(py);
        if let Ok(page) = extra.get_item("pageLayout") {
            slf.borrow_mut().pageLayout = page.extract().ok();
        }
        if let Ok(system) = extra.get_item("systemLayout") {
            slf.borrow_mut().systemLayout = system.extract().ok();
        }
        if let Ok(staves) = extra.get_item("staffLayoutList")
            && let Ok(staves) = staves.extract()
        {
            slf.borrow_mut().staffLayoutList = staves;
        }
        Ok(())
    }
}

/// A measurement as music21 writes it in a `repr`, `None` where there is
/// none.
fn said(value: Option<Tenths>) -> String {
    value.map_or_else(|| "None".to_string(), |value| value.to_string())
}

/// music21's `layout.PageLayout`: how a page is laid out, and whether one
/// starts here.
#[pyclass(
    name = "PageLayout",
    module = "music21.layout",
    subclass,
    skip_from_py_object
)]
pub struct PageLayout {
    pub(crate) inner: RsPageLayout,
}

#[pymethods]
impl PageLayout {
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut made = Self {
            inner: RsPageLayout::default(),
        };
        if let Some(keywords) = keywords {
            for (key, value) in keywords {
                let name: String = key.extract()?;
                match name.as_str() {
                    "pageNumber" => made.inner.page_number = value.extract()?,
                    "leftMargin" => made.inner.left_margin = read_tenths(Some(&value))?,
                    "rightMargin" => made.inner.right_margin = read_tenths(Some(&value))?,
                    "topMargin" => made.inner.top_margin = read_tenths(Some(&value))?,
                    "bottomMargin" => made.inner.bottom_margin = read_tenths(Some(&value))?,
                    "pageHeight" => made.inner.page_height = read_tenths(Some(&value))?,
                    "pageWidth" => made.inner.page_width = read_tenths(Some(&value))?,
                    "isNew" => made.inner.is_new = value.extract()?,
                    other => {
                        return Err(LayoutException::new_err(format!(
                            "a page layout here says nothing of {other}"
                        )));
                    }
                }
            }
        }
        Ok(made)
    }

    /// `leftMargin`, where the score says.
    #[getter]
    fn leftMargin(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.left_margin, py)
    }

    #[setter]
    fn set_leftMargin(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.left_margin = read_tenths(value)?;
        Ok(())
    }

    /// `rightMargin`, where the score says.
    #[getter]
    fn rightMargin(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.right_margin, py)
    }

    #[setter]
    fn set_rightMargin(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.right_margin = read_tenths(value)?;
        Ok(())
    }

    /// `topMargin`, where the score says.
    #[getter]
    fn topMargin(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.top_margin, py)
    }

    #[setter]
    fn set_topMargin(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.top_margin = read_tenths(value)?;
        Ok(())
    }

    /// `bottomMargin`, where the score says.
    #[getter]
    fn bottomMargin(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.bottom_margin, py)
    }

    #[setter]
    fn set_bottomMargin(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.bottom_margin = read_tenths(value)?;
        Ok(())
    }

    /// `pageHeight`, where the score says.
    #[getter]
    fn pageHeight(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.page_height, py)
    }

    #[setter]
    fn set_pageHeight(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.page_height = read_tenths(value)?;
        Ok(())
    }

    /// `pageWidth`, where the score says.
    #[getter]
    fn pageWidth(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.page_width, py)
    }

    #[setter]
    fn set_pageWidth(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.page_width = read_tenths(value)?;
        Ok(())
    }

    /// The page's number, where it says.
    #[getter]
    fn pageNumber(&self) -> Option<i64> {
        self.inner.page_number
    }

    #[setter]
    fn set_pageNumber(&mut self, value: Option<i64>) {
        self.inner.page_number = value;
    }

    /// Whether a new page starts here.
    #[getter]
    fn isNew(&self) -> Option<bool> {
        self.inner.is_new
    }

    #[setter]
    fn set_isNew(&mut self, value: Option<bool>) {
        self.inner.is_new = value;
    }

    fn __repr__(&self) -> &'static str {
        "<music21.layout.PageLayout>"
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsPageLayout>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }
}

/// music21's `layout.SystemLayout`: how a system is laid out, and whether
/// one starts here.
#[pyclass(
    name = "SystemLayout",
    module = "music21.layout",
    subclass,
    skip_from_py_object
)]
pub struct SystemLayout {
    pub(crate) inner: RsSystemLayout,
}

#[pymethods]
impl SystemLayout {
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut made = Self {
            inner: RsSystemLayout::default(),
        };
        if let Some(keywords) = keywords {
            for (key, value) in keywords {
                let name: String = key.extract()?;
                match name.as_str() {
                    "leftMargin" => made.inner.left_margin = read_tenths(Some(&value))?,
                    "rightMargin" => made.inner.right_margin = read_tenths(Some(&value))?,
                    "topMargin" => made.inner.top_margin = read_tenths(Some(&value))?,
                    "bottomMargin" => made.inner.bottom_margin = read_tenths(Some(&value))?,
                    "distance" => made.inner.distance = read_tenths(Some(&value))?,
                    "topDistance" => made.inner.top_distance = read_tenths(Some(&value))?,
                    "isNew" => made.inner.is_new = value.extract()?,
                    other => {
                        return Err(LayoutException::new_err(format!(
                            "a system layout here says nothing of {other}"
                        )));
                    }
                }
            }
        }
        Ok(made)
    }

    /// `leftMargin`, where the score says.
    #[getter]
    fn leftMargin(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.left_margin, py)
    }

    #[setter]
    fn set_leftMargin(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.left_margin = read_tenths(value)?;
        Ok(())
    }

    /// `rightMargin`, where the score says.
    #[getter]
    fn rightMargin(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.right_margin, py)
    }

    #[setter]
    fn set_rightMargin(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.right_margin = read_tenths(value)?;
        Ok(())
    }

    /// `topMargin`, where the score says.
    #[getter]
    fn topMargin(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.top_margin, py)
    }

    #[setter]
    fn set_topMargin(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.top_margin = read_tenths(value)?;
        Ok(())
    }

    /// `bottomMargin`, where the score says.
    #[getter]
    fn bottomMargin(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.bottom_margin, py)
    }

    #[setter]
    fn set_bottomMargin(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.bottom_margin = read_tenths(value)?;
        Ok(())
    }

    /// How far the system stands from the one before.
    #[getter]
    fn distance(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.distance, py)
    }

    #[setter]
    fn set_distance(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.distance = read_tenths(value)?;
        Ok(())
    }

    /// How far the first system of a page stands from its top.
    #[getter]
    fn topDistance(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.top_distance, py)
    }

    #[setter]
    fn set_topDistance(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.top_distance = read_tenths(value)?;
        Ok(())
    }

    /// Whether a new system starts here.
    #[getter]
    fn isNew(&self) -> Option<bool> {
        self.inner.is_new
    }

    #[setter]
    fn set_isNew(&mut self, value: Option<bool>) {
        self.inner.is_new = value;
    }

    fn __repr__(&self) -> &'static str {
        "<music21.layout.SystemLayout>"
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsSystemLayout>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }
}

/// music21's `layout.StaffLayout`: how a staff is laid out, how big it is
/// and what it is for.
#[pyclass(
    name = "StaffLayout",
    module = "music21.layout",
    subclass,
    skip_from_py_object
)]
pub struct StaffLayout {
    pub(crate) inner: RsStaffLayout,
}

#[pymethods]
impl StaffLayout {
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut made = Self {
            inner: RsStaffLayout::default(),
        };
        if let Some(keywords) = keywords {
            for (key, value) in keywords {
                let name: String = key.extract()?;
                match name.as_str() {
                    "distance" => made.inner.distance = read_tenths(Some(&value))?,
                    "staffNumber" => made.inner.staff_number = value.extract()?,
                    "staffSize" => made.inner.staff_size = value.extract()?,
                    "staffLines" => made.inner.staff_lines = value.extract()?,
                    "hidden" => made.inner.hidden = value.extract()?,
                    "staffType" => {
                        made.inner.staff_type = read_staff_type(&value)?;
                    }
                    other => {
                        return Err(LayoutException::new_err(format!(
                            "a staff layout here says nothing of {other}"
                        )));
                    }
                }
            }
        }
        Ok(made)
    }

    /// How far the staff stands from the one above.
    #[getter]
    fn distance(&self, py: Python<'_>) -> Py<PyAny> {
        tenths(self.inner.distance, py)
    }

    #[setter]
    fn set_distance(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.distance = read_tenths(value)?;
        Ok(())
    }

    /// Which staff of its part it is, where it says.
    #[getter]
    fn staffNumber(&self) -> Option<i64> {
        self.inner.staff_number
    }

    #[setter]
    fn set_staffNumber(&mut self, value: Option<i64>) {
        self.inner.staff_number = value;
    }

    /// Its size, as a percentage of the ordinary staff's.
    #[getter]
    fn staffSize(&self) -> Option<f64> {
        self.inner.staff_size
    }

    #[setter]
    fn set_staffSize(&mut self, value: Option<f64>) {
        self.inner.staff_size = value;
    }

    /// How many lines it has, where not five.
    #[getter]
    fn staffLines(&self) -> Option<i64> {
        self.inner.staff_lines
    }

    #[setter]
    fn set_staffLines(&mut self, value: Option<i64>) {
        self.inner.staff_lines = value;
    }

    /// Whether it is hidden, or nothing to inherit whatever the part says.
    #[getter]
    fn hidden(&self) -> Option<bool> {
        self.inner.hidden
    }

    #[setter]
    fn set_hidden(&mut self, value: Option<bool>) {
        self.inner.hidden = value;
    }

    /// music21's `_reprInternal`, which writes each measurement as Python
    /// writes it: a size is a float, so 113 is `113.0`.
    fn __repr__(&self) -> String {
        format!(
            "<music21.layout.StaffLayout distance {}, staffNumber {}, staffSize {}, staffLines {}>",
            said(self.inner.distance),
            self.inner
                .staff_number
                .map_or_else(|| "None".to_string(), |number| number.to_string()),
            self.inner
                .staff_size
                .map_or_else(|| "None".to_string(), |size| format!("{size:?}")),
            self.inner
                .staff_lines
                .map_or_else(|| "None".to_string(), |lines| lines.to_string())
        )
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsStaffLayout>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }
}

/// What a staff is for, by music21's name for it.
fn read_staff_type(value: &Bound<'_, PyAny>) -> PyResult<StaffType> {
    if value.is_none() {
        return Ok(StaffType::default());
    }
    let said = value
        .getattr("value")
        .and_then(|value| value.extract::<String>())
        .or_else(|_| value.extract::<String>())?;
    StaffType::from_name(&said)
        .ok_or_else(|| LayoutException::new_err(format!("no staff is of the type {said}")))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<LayoutBase>()?;
    m.add_class::<ScoreLayout>()?;
    m.add_class::<PageLayout>()?;
    m.add_class::<SystemLayout>()?;
    m.add_class::<StaffLayout>()?;
    Ok(())
}
