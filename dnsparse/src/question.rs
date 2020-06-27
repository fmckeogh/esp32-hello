use core::fmt;
use core::mem::size_of;
use core::str;

use crate::{QueryKind, QueryClass};

/// A DNS question.
#[repr(C)]
pub struct Question<'a> {
  buf: &'a [u8],
  start: usize,
  end: usize,
}

impl fmt::Debug for Question<'_> {
  fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
    fmt.debug_struct("Question")
      .field("name", &self.name())
      .field("kind", &self.kind())
      .field("class", &self.class())
      .finish()
  }
}

/// A DNS question name.
#[derive(Debug)]
pub struct QuestionName<'a> {
  buf: &'a [u8],
  start: usize,
  end: usize,
}

const fn is_pointer(len: u8) -> bool {
  (len >> 6) == 0b11
}

const fn mask_len(len: u8) -> usize {
  (len & 0b00111111) as usize
}

impl fmt::Display for QuestionName<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut i = self.start;

    loop {
      let pointer_or_len = self.buf[i];

      let len = mask_len(pointer_or_len);

      if is_pointer(pointer_or_len) {
        i = (len << 8) + self.buf[i + 1] as usize;
        continue;
      }

      if len == 0 {
        return Ok(())
      }

      if i != self.start {
        ".".fmt(f)?;
      }

      i += 1;

      let s = unsafe { str::from_utf8_unchecked(&self.buf[i..(i + len)]) };

      s.fmt(f)?;

      i += len;
    }
  }
}

impl PartialEq<&str> for QuestionName<'_> {
  fn eq(&self, other: &&str) -> bool {
    let mut i = self.start;
    let mut other_i = 0;

    let other = other.as_bytes();

    loop {
      let pointer_or_len = self.buf[i];

      let len = mask_len(pointer_or_len);

      if is_pointer(pointer_or_len) {
        i = (len << 8) + self.buf[i + 1] as usize;
        continue;
      }

      if len == 0 {
        return other_i == other.len()
      }

      if other_i != 0 {
        if other.get(other_i) != Some(&b'.') {
          return false
        } else {
          other_i += 1;
        }
      }

      i += 1;

      if let Some(substring) = other.get(other_i..(other_i + len)) {
        if !self.buf[i..(i + len)].eq_ignore_ascii_case(substring) {
          return false
        }
      } else {
        return false
      }

      i += len;
      other_i += len;
    }
  }
}

impl<'a> Question<'a> {
  pub fn name(&self) -> QuestionName<'a> {
    QuestionName { buf: self.buf, start: self.start, end: self.end - 5 }
  }

  pub fn kind(&self) -> QueryKind {
    let b0 = self.end - 4;
    let b1 = b0 + 1;
    QueryKind::from(u16::from_be_bytes([self.buf[b0], self.buf[b1]]))
  }

  pub fn class(&self) -> QueryClass {
    let b0 = self.end - 2;
    let b1 = b0 + 1;
    QueryClass::from(u16::from_be_bytes([self.buf[b0], self.buf[b1]]))
  }

  pub fn as_bytes(&self) -> &'a [u8] {
    &self.buf[self.start..self.end]
  }
}

/// Iterator over [`Question`](struct.Question.html)s contained in a [`DnsFrame`](struct.DnsFrame.html).
pub struct Questions<'a> {
  pub(crate) question_count: usize,
  pub(crate) current_question: usize,
  pub(crate) buf: &'a [u8],
  pub(crate) buf_i: usize,
}

pub(crate) fn read_question(buf: &[u8], i: &mut usize) -> bool {
  loop {
    match read_label(buf, i) {
      None => return false,
      Some(false) => continue,
      Some(true) => return read_query_class_and_kind(buf, i),
    }
  }
}

// Return whether a label was read and whether it was the last label.
fn read_label(buf: &[u8], i: &mut usize) -> Option<bool> {
  if let Some(&pointer_or_len) = buf.get(*i) {
    // Check for pointer:
    // https://tools.ietf.org/rfc/rfc1035#section-4.1.4
    if is_pointer(pointer_or_len) {
      if *i + 1 + 1 < buf.len() {
        *i += 1 + 1;
        Some(true)
      } else {
        None
      }
    } else {
      let part_len = mask_len(pointer_or_len);
      *i += 1 + part_len;
      Some(part_len == 0)
    }
  } else {
    None
  }
}

#[inline]
fn read_query_class_and_kind(buf: &[u8], i: &mut usize) -> bool {
  if *i + size_of::<QueryClass>() + size_of::<QueryKind>() <= buf.len() {
    *i += size_of::<QueryClass>() + size_of::<QueryKind>();
    true
  } else {
    false
  }
}

impl<'a> Iterator for Questions<'a> {
  type Item = Question<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.current_question >= self.question_count {
      return None
    }

    let mut i = self.buf_i;

    assert!(read_question(&self.buf, &mut i));
    let question = Question { buf: &self.buf, start: self.buf_i, end: i };

    self.current_question += 1;
    self.buf_i = i;

    return Some(question)
  }
}

