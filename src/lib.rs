use core::mem::replace;

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

pub trait BitBuf {
    fn advance(&mut self, bits: usize) -> Result<(), Insufficient>;
    fn copy_to_slice(&mut self, dst: &mut [u8], bits: usize) -> Result<(), CopyError>;
    fn copy_aligned(&mut self, dst: &mut [u8]) -> Result<(), CopyError> {
        self.copy_to_slice(dst, dst.len() * 8)
    }
    fn pop(&mut self) -> Option<bool>;
    fn pop_byte(&mut self) -> Option<u8> {
        let mut data = [0];
        self.copy_aligned(&mut data).ok()?;
        Some(data[0])
    }
    fn remaining(&self) -> usize;
    fn len(&self) -> usize;
}

#[derive(Debug)]
pub struct BitSlice<'a> {
    data: &'a [u8],
    prefix: u8,
    len: usize,
}

impl<'a> BitBuf for BitSlice<'a> {
    fn advance(&mut self, bits: usize) -> Result<(), Insufficient> {
        self.prefix += (bits & 7) as u8;
        if self.prefix >= 8 {
            self.prefix -= 8;
            self.data = self.data.get((bits / 8) + 1..).ok_or(Insufficient)?;
        } else {
            self.data = self.data.get(bits / 8..).ok_or(Insufficient)?;
        }
        self.len += bits;
        Ok(())
    }

    fn copy_aligned(&mut self, dst: &mut [u8]) -> Result<(), CopyError> {
        Ok(for i in 0..dst.len() {
            dst[i] = self.byte_at_offset(i * 8).ok_or(Insufficient)?;
        })
    }

    fn len(&self) -> usize {
        self.len
    }

    fn copy_to_slice(&mut self, dst: &mut [u8], bits: usize) -> Result<(), CopyError> {
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
        if rem != 0 {
            if len < bytes + 1 {
                return Err(CopyError::Overflow);
            }
            let byte = self
                .data_at_offset(bytes * 8, rem)
                .ok_or(CopyError::Insufficient(Insufficient))?
                & (255 << (8 - rem));
            dst[bytes] |= byte;
            dst[bytes] &= byte;
        }
        self.advance(bits)?;
        Ok(())
    }

    fn pop(&mut self) -> Option<bool> {
        let byte = self.byte_at_offset(0)?;
        self.advance(1).unwrap();
        Some(byte & 128 != 0)
    }

    fn pop_byte(&mut self) -> Option<u8> {
        let byte = self.byte_at_offset(0)?;
        self.advance(8).unwrap();
        Some(byte)
    }

    fn remaining(&self) -> usize {
        self.data.len() * 8 - self.prefix as usize
    }
}

impl<'a> BitSlice<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitSlice {
            data,
            prefix: 0,
            len: 0,
        }
    }

    fn data_at_offset(&self, offset: usize, size: usize) -> Option<u8> {
        let len = self.remaining();
        let offset = self.prefix as usize + offset;
        if offset == 0 {
            if len == 0 {
                return None;
            }
            Some(self.data[0])
        } else if len < size {
            None
        } else {
            let offset_bytes = offset / 8;
            let offset_rem = offset & 7;
            if offset_rem == 0 {
                Some(self.data[offset_bytes])
            } else {
                let offset_rem_inv = 8 - offset_rem;
                Some(if size + offset_rem <= 8 {
                    ((self.data[offset_bytes] & (255 >> offset_rem)) << offset_rem)
                } else {
                    ((self.data[offset_bytes] & (255 >> offset_rem)) << offset_rem)
                        + ((self.data[(offset_bytes) + 1] & (255 << offset_rem_inv))
                            >> offset_rem_inv)
                })
            }
        }
    }

    fn byte_at_offset(&self, offset: usize) -> Option<u8> {
        self.data_at_offset(offset, 8)
    }
}

#[derive(Debug)]
pub struct BitSliceMut<'a> {
    data: &'a mut [u8],
    prefix: u8,
    len: usize,
}

pub trait BitBufMut {
    fn len(&self) -> usize;
    fn advance(&mut self, bits: usize) -> Result<(), Insufficient>;
    fn push(&mut self, item: bool) -> Result<(), Insufficient>;
    fn put(&mut self, data: &[u8], bits: usize) -> Result<(), CopyError>;
    fn put_byte(&mut self, data: u8) -> Result<(), CopyError> {
        self.put_aligned(&[data])
    }
    fn put_aligned(&mut self, data: &[u8]) -> Result<(), CopyError> {
        self.put(data, data.len() * 8)
    }
}

impl<'a> BitBufMut for BitSliceMut<'a> {
    fn len(&self) -> usize {
        self.len
    }

    fn advance(&mut self, bits: usize) -> Result<(), Insufficient> {
        self.prefix += (bits & 7) as u8;
        if self.prefix >= 8 {
            self.prefix -= 8;
            let empty = &mut [];
            let data = replace(&mut self.data, empty);
            self.data = data.get_mut((bits / 8) + 1..).ok_or(Insufficient)?;
        } else {
            let empty = &mut [];
            let data = replace(&mut self.data, empty);
            self.data = data.get_mut(bits / 8..).ok_or(Insufficient)?;
        }
        self.len += bits;
        Ok(())
    }

    fn push(&mut self, item: bool) -> Result<(), Insufficient> {
        if self.data.len() == 0 {
            return Err(Insufficient);
        }
        let byte = &mut self.data[0];
        if item {
            *byte |= 128 >> self.prefix;
        } else {
            *byte &= 255 ^ (128 >> self.prefix);
        }
        self.advance(1)?;
        Ok(())
    }

    fn put_byte(&mut self, item: u8) -> Result<(), CopyError> {
        if self.data.len() == 0 {
            return Err(Insufficient.into());
        }
        if self.prefix == 0 {
            self.data[0] = item;
        } else {
            if self.data.len() == 1 {
                return Err(Insufficient.into());
            }
            let inv_prefix = 8 - self.prefix;
            self.data[0] |= item >> self.prefix;
            self.data[0] &= (item >> self.prefix) | (255 << inv_prefix);
            self.data[1] |= item << inv_prefix;
            self.data[1] &= (item << inv_prefix) | (255 << self.prefix);
        }
        self.advance(8)?;
        Ok(())
    }

    fn put(&mut self, data: &[u8], bits: usize) -> Result<(), CopyError> {
        let bytes = bits / 8;
        if bits == 0 {
            return Ok(());
        }
        let len = data.len();
        if len == 0 {
            return Err(CopyError::Overflow);
        }
        if len < bytes {
            return Err(CopyError::Overflow);
        }
        for i in 0..bytes {
            self.put_byte(data[i])?;
        }
        let rem = bits & 7;
        if rem != 0 {
            if len < bytes + 1 {
                return Err(CopyError::Overflow);
            }
            let byte = data[bytes];
            self.write(byte, rem)?;
        }
        Ok(())
    }

    fn put_aligned(&mut self, data: &[u8]) -> Result<(), CopyError> {
        for byte in data {
            self.put_byte(*byte)?;
        }
        Ok(())
    }
}

impl<'a> BitSliceMut<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        BitSliceMut {
            data,
            prefix: 0,
            len: 0,
        }
    }

    fn write(&mut self, item: u8, bits: usize) -> Result<(), Insufficient> {
        if self.prefix == 0 {
            let inv_bits = 8 - bits;
            let litem = item & (255 << (inv_bits));
            let hitem = item | (255 >> (inv_bits));
            self.data[0] |= litem;
            self.data[0] &= hitem;
        } else {
            let inv_bits = 8 - bits;
            let inv_prefix = 8 - self.prefix;
            let litem = item & (255 << inv_bits);
            let hitem = item | (255 >> inv_bits);
            self.data[0] |= litem >> self.prefix;
            self.data[0] &= (hitem >> self.prefix) | (255 << inv_prefix);
            self.data[1] |= litem << inv_prefix;
            self.data[1] &= (hitem << inv_prefix) | (255 << self.prefix);
        }
        self.advance(bits)?;
        Ok(())
    }
}
