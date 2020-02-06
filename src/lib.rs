#[derive(Debug)]
pub struct Insufficient;

#[derive(Debug)]
pub enum CopyError {
    Insufficient(Insufficient),
    Overflow,
}

impl From<Insufficient> for CopyError {
    fn from(input: Insufficient) -> Self {
        CopyError::Insufficient(input)
    }
}

#[derive(Debug)]
pub struct BitBuf<'a> {
    data: &'a [u8],
    prefix: u8,
}

impl<'a> BitBuf<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitBuf { data, prefix: 0 }
    }

    pub fn advance(&mut self, bits: usize) -> Result<(), Insufficient> {
        self.prefix += (bits & 7) as u8;
        if self.prefix >= 8 {
            self.prefix -= 8;
            self.data = self.data.get((bits / 8) + 1..).ok_or(Insufficient)?;
        } else {
            self.data = self.data.get(bits / 8..).ok_or(Insufficient)?;
        }
        Ok(())
    }

    pub fn byte_at_offset(&self, offset: usize) -> Option<u8> {
        let len = self.len();
        if offset == 0 {
            if len == 0 {
                return None;
            }
            Some(self.data[0])
        } else if len < 8 || offset > len - 8 {
            None
        } else {
            let offset_bytes = offset / 8;
            let offset_rem = offset & 7;
            if offset_rem == 0 {
                Some(self.data[offset_bytes])
            } else {
                let offset_rem_inv = 8 - offset_rem;
                Some(
                    ((self.data[offset_bytes] & (255 >> offset_rem)) << offset_rem)
                        + ((self.data[(offset_bytes) + 1] & (255 << offset_rem_inv))
                            >> offset_rem_inv),
                )
            }
        }
    }

    pub fn copy_aligned(&mut self, dst: &mut [u8]) -> Result<(), Insufficient> {
        Ok(for i in 0..dst.len() {
            dst[i] = self.byte_at_offset(i * 8).ok_or(Insufficient)?;
        })
    }

    pub fn copy_to_slice(&mut self, dst: &mut [u8], bits: usize) -> Result<(), CopyError> {
        let bytes = bits / 8;
        let len = dst.len();
        if len < bytes {
            return Err(CopyError::Overflow);
        }
        for i in 0..bytes {
            dst[i] = self
                .byte_at_offset(i * 8)
                .ok_or(CopyError::Insufficient(Insufficient))?;
        }
        let rem = bits & 7;
        if rem > 0 {
            if len < bytes + 1 {
                return Err(CopyError::Overflow);
            }
            let byte = self
                .byte_at_offset(bytes * 8)
                .ok_or(CopyError::Insufficient(Insufficient))?
                & (255 << (8 - rem));
            dst[bytes] |= byte;
            dst[bytes] &= byte;
        }
        self.advance(bits)?;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<bool> {
        let byte = self.byte_at_offset(0)?;
        self.advance(1).unwrap();
        Some(byte & 1 != 0)
    }

    pub fn len(&self) -> usize {
        self.data.len() * 8 - self.prefix as usize
    }
}

#[derive(Debug)]
pub struct BitBufMut<'a> {
    data: &'a mut [u8],
    prefix: u8,
}

impl<'a> BitBufMut<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        BitBufMut { data, prefix: 0 }
    }

    pub fn skip(&'a mut self, bits: usize) -> Result<(), Insufficient> {
        self.prefix += (bits & 7) as u8;
        if self.prefix >= 8 {
            self.prefix -= 8;
            self.data = self.data.get_mut((bits / 8) + 1..).ok_or(Insufficient)?;
        } else {
            self.data = self.data.get_mut(bits / 8..).ok_or(Insufficient)?;
        }
        Ok(())
    }
}
