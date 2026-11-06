//! Reads the parts of a fuzz input.

/// A cursor over a fuzz input. When the input ends, each read gives zero or an empty slice.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub(crate) fn byte(&mut self) -> u8 {
        match self.data.split_first() {
            Some((&first, rest)) => {
                self.data = rest;
                first
            }
            None => 0,
        }
    }

    pub(crate) fn take(&mut self, len: usize) -> &'a [u8] {
        let (taken, rest) = self.data.split_at(len.min(self.data.len()));
        self.data = rest;
        taken
    }

    /// Reads a length byte and then that number of bytes.
    pub(crate) fn chunk(&mut self) -> &'a [u8] {
        let len = usize::from(self.byte());
        self.take(len)
    }

    /// Reads a two-byte length and then that number of bytes.
    pub(crate) fn long_chunk(&mut self) -> &'a [u8] {
        let len = usize::from(u16::from_be_bytes([self.byte(), self.byte()]));
        self.take(len)
    }

    pub(crate) fn rest(self) -> &'a [u8] {
        self.data
    }
}
