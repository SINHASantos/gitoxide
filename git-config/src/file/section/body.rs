use crate::parse::section::Key;
use crate::parse::Event;
use crate::value::{normalize, normalize_bstr, normalize_bstring};
use bstr::{BStr, BString, ByteVec};
use std::borrow::Cow;
use std::iter::FusedIterator;
use std::ops::Range;

/// A opaque type that represents a section body.
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Debug, Default)]
pub struct SectionBody<'event>(pub(crate) crate::parse::section::Events<'event>);

/// Access
impl<'event> SectionBody<'event> {
    /// Retrieves the last matching value in a section with the given key, if present.
    #[must_use]
    pub fn value(&self, key: impl AsRef<str>) -> Option<Cow<'_, BStr>> {
        let key = Key::from_str_unchecked(key.as_ref());
        let (_, range) = self.key_and_value_range_by(&key)?;
        let mut concatenated = BString::default();

        for event in &self.0[range] {
            match event {
                Event::Value(v) => {
                    return Some(normalize_bstr(v.as_ref()));
                }
                Event::ValueNotDone(v) => {
                    concatenated.push_str(v.as_ref());
                }
                Event::ValueDone(v) => {
                    concatenated.push_str(v.as_ref());
                    return Some(normalize_bstring(concatenated));
                }
                _ => (),
            }
        }
        None
    }

    /// Retrieves all values that have the provided key name. This may return
    /// an empty vec, which implies there were no values with the provided key.
    #[must_use]
    pub fn values(&self, key: impl AsRef<str>) -> Vec<Cow<'_, BStr>> {
        let key = &Key::from_str_unchecked(key.as_ref());
        let mut values = Vec::new();
        let mut expect_value = false;
        let mut concatenated_value = BString::default();

        for event in &self.0 {
            match event {
                Event::SectionKey(event_key) if event_key == key => expect_value = true,
                Event::Value(v) if expect_value => {
                    expect_value = false;
                    values.push(normalize_bstr(v.as_ref()));
                }
                Event::ValueNotDone(v) if expect_value => {
                    concatenated_value.push_str(v.as_ref());
                }
                Event::ValueDone(v) if expect_value => {
                    expect_value = false;
                    concatenated_value.push_str(v.as_ref());
                    values.push(normalize_bstring(std::mem::take(&mut concatenated_value)));
                }
                _ => (),
            }
        }

        values
    }

    /// Returns an iterator visiting all keys in order.
    pub fn keys(&self) -> impl Iterator<Item = &Key<'event>> {
        self.0
            .iter()
            .filter_map(|e| if let Event::SectionKey(k) = e { Some(k) } else { None })
    }

    /// Returns true if the section containss the provided key.
    #[must_use]
    pub fn contains_key(&self, key: impl AsRef<str>) -> bool {
        let key = &Key::from_str_unchecked(key.as_ref());
        self.0.iter().any(|e| {
            matches!(e,
                Event::SectionKey(k) if k == key
            )
        })
    }

    /// Returns the number of values in the section.
    #[must_use]
    pub fn num_values(&self) -> usize {
        self.0.iter().filter(|e| matches!(e, Event::SectionKey(_))).count()
    }

    /// Returns if the section is empty.
    /// Note that this may count whitespace, see [`num_values()`][Self::num_values()] for
    /// another way to determine semantic emptiness.
    #[must_use]
    pub fn is_void(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'event> SectionBody<'event> {
    pub(crate) fn as_ref(&self) -> &[Event<'_>] {
        &self.0
    }

    /// Returns the the range containing the value events for the `key`.
    /// If the value is not found, then this returns an empty range.
    pub(crate) fn key_and_value_range_by(&self, key: &Key<'_>) -> Option<(Range<usize>, Range<usize>)> {
        let mut value_range = Range::default();
        let mut key_start = None;
        for (i, e) in self.0.iter().enumerate().rev() {
            match e {
                Event::SectionKey(k) => {
                    if k == key {
                        key_start = Some(i);
                        break;
                    }
                    value_range = Range::default();
                }
                Event::Value(_) => {
                    (value_range.start, value_range.end) = (i, i);
                }
                Event::ValueNotDone(_) | Event::ValueDone(_) => {
                    if value_range.end == 0 {
                        value_range.end = i
                    } else {
                        value_range.start = i
                    };
                }
                _ => (),
            }
        }
        key_start.map(|key_start| {
            // value end needs to be offset by one so that the last value's index
            // is included in the range
            let value_range = value_range.start..value_range.end + 1;
            (key_start..value_range.end, value_range)
        })
    }
}

/// An owning iterator of a section body. Created by [`SectionBody::into_iter`], yielding
/// un-normalized (`key`, `value`) pairs.
// TODO: tests
pub struct SectionBodyIter<'event>(smallvec::IntoIter<[Event<'event>; 64]>);

impl<'event> IntoIterator for SectionBody<'event> {
    type Item = (Key<'event>, Cow<'event, BStr>);

    type IntoIter = SectionBodyIter<'event>;

    fn into_iter(self) -> Self::IntoIter {
        SectionBodyIter(self.0.into_iter())
    }
}

impl<'event> Iterator for SectionBodyIter<'event> {
    type Item = (Key<'event>, Cow<'event, BStr>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut key = None;
        let mut partial_value = BString::default();
        let mut value = None;

        for event in self.0.by_ref() {
            match event {
                Event::SectionKey(k) => key = Some(k),
                Event::Value(v) => {
                    value = Some(v);
                    break;
                }
                Event::ValueNotDone(v) => partial_value.push_str(v.as_ref()),
                Event::ValueDone(v) => {
                    partial_value.push_str(v.as_ref());
                    value = Some(partial_value.into());
                    break;
                }
                _ => (),
            }
        }

        key.zip(value.map(normalize))
    }
}

impl FusedIterator for SectionBodyIter<'_> {}
