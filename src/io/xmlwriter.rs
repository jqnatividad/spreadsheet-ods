use std::fmt;
use std::fmt::{Display, Formatter};
use std::io::{self, Write};
#[cfg(not(feature = "check_xml"))]
use std::marker::PhantomData;

#[derive(PartialEq)]
enum Open {
    None,
    Elem,
    Empty,
}

impl Display for Open {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Open::None => f.write_str("None")?,
            Open::Elem => f.write_str("Elem")?,
            Open::Empty => f.write_str("Empty")?,
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Stack {
    #[cfg(feature = "check_xml")]
    stack: Vec<String>,
    #[cfg(not(feature = "check_xml"))]
    stack: PhantomData<String>,
}

#[cfg(feature = "check_xml")]
impl Stack {
    fn new() -> Self {
        Self { stack: Vec::new() }
    }

    fn push(&mut self, name: &str) {
        self.stack.push(name.to_string());
    }

    fn pop(&mut self) -> Option<String> {
        self.stack.pop()
    }

    fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

#[cfg(not(feature = "check_xml"))]
impl Stack {
    fn new() -> Self {
        Self {
            stack: PhantomData {},
        }
    }

    fn push(&mut self, _name: &str) {}

    fn pop(&mut self) -> Option<String> {
        None
    }

    fn is_empty(&self) -> bool {
        true
    }
}

/// The XmlWriter himself
pub(crate) struct XmlWriter<W: Write> {
    writer: Box<W>,
    buf: String,
    stack: Stack,
    open: Open,
}

impl<W: Write> fmt::Debug for XmlWriter<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "XmlWriter {{ stack: {:?}, opened: {} }}",
            self.stack, self.open
        )
    }
}

impl<W: Write> XmlWriter<W> {
    /// Create a new writer, by passing an `io::Write`
    pub(crate) fn new(writer: W) -> XmlWriter<W> {
        XmlWriter {
            stack: Stack::new(),
            buf: String::new(),
            writer: Box::new(writer),
            open: Open::None,
        }
    }

    /// Write the DTD. You have to take care of the encoding
    /// on the underlying Write yourself.
    pub(crate) fn dtd(&mut self, encoding: &str) -> io::Result<()> {
        self.buf.push_str("<?xml version=\"1.0\" encoding=\"");
        self.buf.push_str(encoding);
        self.buf.push_str("\" ?>\n");

        Ok(())
    }

    /// Write an element with inlined text (not escaped)
    pub(crate) fn elem_text<S: AsRef<str>>(&mut self, name: &str, text: S) -> io::Result<()> {
        self.close_elem()?;

        self.buf.push('<');
        self.buf.push_str(name);
        self.buf.push('>');

        self.buf.push_str(text.as_ref());

        self.buf.push('<');
        self.buf.push('/');
        self.buf.push_str(name);
        self.buf.push('>');

        Ok(())
    }

    /// Write an element with inlined text (not escaped)
    pub(crate) fn opt_elem_text<S: AsRef<str>>(&mut self, name: &str, text: S) -> io::Result<()> {
        if !text.as_ref().is_empty() {
            self.close_elem()?;

            self.buf.push('<');
            self.buf.push_str(name);
            self.buf.push('>');

            self.buf.push_str(text.as_ref());

            self.buf.push('<');
            self.buf.push('/');
            self.buf.push_str(name);
            self.buf.push('>');
        }

        Ok(())
    }

    /// Write an optional element with inlined text (escaped).
    /// If text.is_empty() the element is not written at all.
    #[allow(dead_code)]
    pub(crate) fn opt_elem_text_esc<S: AsRef<str>>(
        &mut self,
        name: &str,
        text: S,
    ) -> io::Result<()> {
        if !text.as_ref().is_empty() {
            self.close_elem()?;

            self.buf.push('<');
            self.buf.push_str(name);
            self.buf.push('>');

            self.escape(text.as_ref(), false);

            self.buf.push('<');
            self.buf.push('/');
            self.buf.push_str(name);
            self.buf.push('>');
        }

        Ok(())
    }

    /// Write an element with inlined text (escaped)
    #[allow(dead_code)]
    pub(crate) fn elem_text_esc<S: AsRef<str>>(&mut self, name: &str, text: S) -> io::Result<()> {
        self.close_elem()?;

        self.buf.push('<');
        self.buf.push_str(name);
        self.buf.push('>');

        self.escape(text.as_ref(), false);

        self.buf.push('<');
        self.buf.push('/');
        self.buf.push_str(name);
        self.buf.push('>');

        Ok(())
    }

    /// Begin an elem, make sure name contains only allowed chars
    pub(crate) fn elem(&mut self, name: &str) -> io::Result<()> {
        self.close_elem()?;

        self.stack.push(name);

        self.buf.push('<');
        self.open = Open::Elem;
        self.buf.push_str(name);
        Ok(())
    }

    /// Begin an empty elem
    pub(crate) fn empty(&mut self, name: &str) -> io::Result<()> {
        self.close_elem()?;

        self.buf.push('<');
        self.open = Open::Empty;
        self.buf.push_str(name);
        Ok(())
    }

    /// Close an elem if open, do nothing otherwise
    fn close_elem(&mut self) -> io::Result<()> {
        match self.open {
            Open::None => {}
            Open::Elem => {
                self.buf.push('>');
            }
            Open::Empty => {
                self.buf.push('/');
                self.buf.push('>');
            }
        }
        self.open = Open::None;
        self.write_buf()?;
        Ok(())
    }

    /// Write an attr, make sure name and value contain only allowed chars.
    /// For an escaping version use `attr_esc`
    pub(crate) fn attr<S: AsRef<str>>(&mut self, name: &str, value: S) -> io::Result<()> {
        if cfg!(feature = "check_xml") && self.open == Open::None {
            panic!(
                "Attempted to write attr to elem, when no elem was opened, stack {:?}",
                self.stack
            );
        }
        self.buf.push(' ');
        self.buf.push_str(name);
        self.buf.push('=');
        self.buf.push('"');
        self.buf.push_str(value.as_ref());
        self.buf.push('"');
        Ok(())
    }

    /// Write an attr, make sure name contains only allowed chars
    pub(crate) fn attr_esc<S: AsRef<str>>(&mut self, name: &str, value: S) -> io::Result<()> {
        if cfg!(feature = "check_xml") && self.open == Open::None {
            panic!(
                "Attempted to write attr to elem, when no elem was opened, stack {:?}",
                self.stack
            );
        }
        self.buf.push(' ');
        self.escape(name, true);
        self.buf.push('=');
        self.buf.push('"');
        self.escape(value.as_ref(), false);
        self.buf.push('"');
        Ok(())
    }

    /// Escape identifiers or text
    fn escape(&mut self, text: &str, ident: bool) {
        for c in text.chars() {
            match c {
                '"' => self.buf.push_str("&quot;"),
                '\'' => self.buf.push_str("&apos;"),
                '&' => self.buf.push_str("&amp;"),
                '<' => self.buf.push_str("&lt;"),
                '>' => self.buf.push_str("&gt;"),
                '\\' if ident => {
                    self.buf.push('\\');
                    self.buf.push('\\');
                }
                _ => {
                    self.buf.push(c);
                }
            };
        }
    }

    /// Write a text, doesn't escape the text.
    pub(crate) fn text<S: AsRef<str>>(&mut self, text: S) -> io::Result<()> {
        self.close_elem()?;
        self.buf.push_str(text.as_ref());
        Ok(())
    }

    /// Write a text, escapes the text automatically
    pub(crate) fn text_esc<S: AsRef<str>>(&mut self, text: S) -> io::Result<()> {
        self.close_elem()?;
        self.escape(text.as_ref(), false);
        Ok(())
    }

    /// End and elem
    pub(crate) fn end_elem(&mut self, name: &str) -> io::Result<()> {
        self.close_elem()?;

        if cfg!(feature = "check_xml") {
            match self.stack.pop() {
                Some(test) => {
                    if name != test {
                        panic!(
                            "Attempted to close elem {} but the open was {}, stack {:?}",
                            name, test, self.stack
                        )
                    }
                }
                None => panic!(
                    "Attempted to close an elem, when none was open, stack {:?}",
                    self.stack
                ),
            }
        }

        self.buf.push('<');
        self.buf.push('/');
        self.buf.push_str(name);
        self.buf.push('>');

        Ok(())
    }

    fn write_buf(&mut self) -> io::Result<()> {
        self.writer.write_all(self.buf.as_bytes())?;
        self.buf.clear();
        Ok(())
    }

    /// Fails if there are any open elements.
    pub(crate) fn close(&mut self) -> io::Result<()> {
        self.write_buf()?;

        if cfg!(feature = "check_xml") && !self.stack.is_empty() {
            panic!(
                "Attempted to close the xml, but there are open elements on the stack {:?}",
                self.stack
            )
        }
        Ok(())
    }
}
